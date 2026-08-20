//! 内置压测模块（v3 P2-3）：并发连接 + 消息速率
//!
//! 在进程内启动一个**真实的** `WsServer`（复用生产传输层 `backend::transport::websocket`），
//! 以回显（echo）模式压测：N 个并发 WS 客户端各发送 M 条消息，服务端 `on_message`
//! 钩子原样回显，客户端按序等待回显并计算 RTT；同时用共享原子计数器统计服务端
//! 实际接收量。产出结构化 `BenchReport`（连接数、吞吐 msg/s、RTT p50/p95/p99、错误数）。
//!
//! 运行（可读报告）：`cargo run --bin benchmark`（见 `src/bin/benchmark.rs`）。

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Instant;

use futures_util::{SinkExt, StreamExt};
use serde::Serialize;

use tokio::time::timeout;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use crate::backend::error::BackendError;
use crate::backend::transport::hooks::TransportHooks;
use crate::backend::transport::websocket::WsServer;
use crate::backend::transport::Transport;
use crate::backend::types::*;

/// 压测配置
#[derive(Debug, Clone)]
pub struct BenchConfig {
    /// 并发客户端数
    pub clients: usize,
    /// 每客户端发送消息数
    pub messages_per_client: usize,
    /// 单条消息 payload 字节数（约；用于生成填充文本，上限 4 KiB）
    pub payload_bytes: usize,
    /// 是否回显（true：客户端等待回显并测 RTT；false：仅测发送吞吐）
    pub echo: bool,
}

impl Default for BenchConfig {
    fn default() -> Self {
        Self {
            clients: 50,
            messages_per_client: 200,
            payload_bytes: 64,
            echo: true,
        }
    }
}

/// 压测报告（结构化，可序列化输出）
#[derive(Debug, Clone, Serialize)]
pub struct BenchReport {
    /// 被压测的服务端 URL
    pub server_url: String,
    /// 并发客户端目标数
    pub clients: usize,
    /// 成功建立连接的客户端数
    pub connects: usize,
    /// 连接失败数
    pub connect_errors: usize,
    /// 目标发送消息总数（clients × messages_per_client）
    pub messages_target: usize,
    /// 客户端实际发送消息数
    pub messages_sent: usize,
    /// 服务端实际接收消息数（共享计数器统计）
    pub messages_received: usize,
    /// 总耗时（毫秒）
    pub duration_ms: u64,
    /// 吞吐：消息/秒（按已发送计）
    pub msg_per_sec: f64,
    /// RTT p50（毫秒，echo 模式）
    pub latency_ms_p50: f64,
    /// RTT p95（毫秒，echo 模式）
    pub latency_ms_p95: f64,
    /// RTT p99（毫秒，echo 模式）
    pub latency_ms_p99: f64,
    /// 错误数（连接失败 + 收发异常 + 回显超时）
    pub errors: usize,
}

/// 单客户端压测结果（内部聚合用）
struct ClientResult {
    connected: bool,
    sent: usize,
    received: usize,
    latencies: Vec<f64>,
    errors: usize,
}

/// 分配一个空闲 TCP 端口（绑定 0 端口后取系统分配值）。
/// 存在极小竞态窗口，调用方在 `start_echo_server` 失败时会重试。
fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("分配空闲端口失败");
    listener.local_addr().unwrap().port()
}

/// 在进程内启动回显 WS 服务端（复用生产 `WsServer`），返回 server 句柄与服务端接收计数。
async fn start_echo_server(port: u16) -> Result<(Arc<WsServer>, Arc<AtomicUsize>), BackendError> {
    let received = Arc::new(AtomicUsize::new(0));
    let received_for_hook = received.clone();

    // 用 weak-cell 让 on_message 钩子能回显：钩子内升级 weak 拿到 server 调 Transport::send
    let weak_cell: Arc<Mutex<Option<Weak<WsServer>>>> = Arc::new(Mutex::new(None));
    let weak_cell_for_hook = weak_cell.clone();

    let hooks = TransportHooks {
        on_connect: Arc::new(|_| {}),
        on_message: Arc::new(move |sid, event, data| {
            received_for_hook.fetch_add(1, Ordering::SeqCst);
            // `Transport::send` 是 async，钩子是同步 Fn，必须 spawn 执行（否则 future 被丢弃，回显不发）
            if let Some(s) = weak_cell_for_hook
                .lock()
                .unwrap()
                .as_ref()
                .and_then(|w| w.upgrade())
            {
                tokio::spawn(async move {
                    let _ = s.send(&sid, &event, data).await;
                });
            }
        }),
        on_disconnect: Arc::new(|_| {}),
    };

    let cfg = ServerConfig {
        id: "benchmark".into(),
        name: "benchmark".into(),
        ip: "127.0.0.1".into(),
        port: port as u32,
        protocol: ProtocolType::Websocket,
        wss_enabled: false,
        ..Default::default()
    };
    // 放宽连接上限，避免压测被 max_connections_per_server 拦截；ip_access 默认空白名单=放行全部
    let mut sys = SystemSettings::default();
    sys.max_connections_per_server = 1_000_000;

    let server: Arc<WsServer> = Arc::new_cyclic(|weak| {
        *weak_cell.lock().unwrap() = Some(weak.clone());
        WsServer::new(cfg, sys, hooks.clone(), weak.clone())
    });
    server.start().await?;
    Ok((server, received))
}

/// 单客户端：连接 → 发送 messages 条消息 →（echo 模式）等待回显并记 RTT。
async fn run_client(url: String, messages: usize, payload_bytes: usize, echo: bool) -> ClientResult {
    let (ws_stream, _) = match connect_async(&url).await {
        Ok(v) => v,
        Err(_) => {
            return ClientResult {
                connected: false,
                sent: 0,
                received: 0,
                latencies: Vec::new(),
                errors: 1,
            }
        }
    };
    let (mut write, mut read) = ws_stream.split();

    let pad = "x".repeat(payload_bytes.min(4096));
    let mut sent = 0usize;
    let mut received_count = 0usize;
    let mut errors = 0usize;
    let mut latencies = Vec::new();

    for i in 0..messages {
        let msg = serde_json::json!({
            "event": "benchmark",
            "data": { "seq": i, "pad": pad }
        })
        .to_string();
        if write.send(Message::Text(msg.into())).await.is_err() {
            errors += 1;
            break;
        }
        sent += 1;

        if echo {
            let t0 = Instant::now();
            match timeout(std::time::Duration::from_secs(5), read.next()).await {
                Ok(Some(Ok(Message::Text(_)))) => {
                    let rtt = t0.elapsed().as_secs_f64() * 1000.0;
                    latencies.push(rtt);
                    received_count += 1;
                }
                _ => {
                    errors += 1;
                    break;
                }
            }
        }
    }

    // 优雅关闭：发送 Close 帧，减少服务端 "Connection reset" 日志
    let _ = write.close().await;

    ClientResult {
        connected: true,
        sent,
        received: received_count,
        latencies,
        errors,
    }
}

/// 对延迟样本排序后取分位值
fn percentiles(latencies: &mut [f64]) -> (f64, f64, f64) {
    if latencies.is_empty() {
        return (0.0, 0.0, 0.0);
    }
    latencies.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = latencies.len();
    let at = |p: f64| latencies[((p * n as f64) as usize).min(n - 1)];
    (at(0.50), at(0.95), at(0.99))
}

/// 执行压测：启动回显服务端 → 并发客户端压测 → 聚合报告 → 停止服务端。
pub async fn run_bench(cfg: &BenchConfig) -> Result<BenchReport, BackendError> {
    // 找空闲端口并启动服务端（少量重试规避端口竞态）
    let (server, received, port) = {
        let mut result = None;
        let mut last_err = None;
        for _ in 0..5 {
            let p = free_port();
            match start_echo_server(p).await {
                Ok((s, r)) => {
                    result = Some((s, r, p));
                    break;
                }
                Err(e) => last_err = Some(e),
            }
        }
        result.ok_or_else(|| {
            last_err.unwrap_or_else(|| BackendError::Internal("压测服务端启动失败".into()))
        })?
    };
    let url = format!("ws://127.0.0.1:{}", port);

    let start = Instant::now();
    let mut handles = Vec::with_capacity(cfg.clients);
    for _ in 0..cfg.clients {
        let u = url.clone();
        handles.push(tokio::spawn(run_client(
            u,
            cfg.messages_per_client,
            cfg.payload_bytes,
            cfg.echo,
        )));
    }

    let mut connects = 0usize;
    let mut connect_errors = 0usize;
    let mut messages_sent = 0usize;
    let mut errors = 0usize;
    let mut all_latencies: Vec<f64> = Vec::new();
    for h in handles {
        let r = h.await.unwrap_or(ClientResult {
            connected: false,
            sent: 0,
            received: 0,
            latencies: Vec::new(),
            errors: 1,
        });
        if r.connected {
            connects += 1;
        } else {
            connect_errors += 1;
        }
        messages_sent += r.sent;
        errors += r.errors;
        all_latencies.extend(r.latencies);
    }

    // 非回显模式客户端发完即结束，服务端可能仍在从 socket 缓冲读取；
    // 等待服务端 ingest 追上已发送量（最多 2s），确保 messages_received 准确。
    if !cfg.echo {
        let deadline = start + std::time::Duration::from_secs(2);
        while received.load(Ordering::SeqCst) < messages_sent && Instant::now() < deadline {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }

    let duration_ms = start.elapsed().as_millis() as u64;
    let messages_received = received.load(Ordering::SeqCst);
    let msg_per_sec = if duration_ms > 0 {
        messages_sent as f64 / (duration_ms as f64 / 1000.0)
    } else {
        0.0
    };
    let (p50, p95, p99) = percentiles(&mut all_latencies);

    // 停止服务端（释放端口；客户端连接会因 out_rx 关闭而结束）
    let _ = server.stop().await;

    Ok(BenchReport {
        server_url: url,
        clients: cfg.clients,
        connects,
        connect_errors,
        messages_target: cfg.clients * cfg.messages_per_client,
        messages_sent,
        messages_received,
        duration_ms,
        msg_per_sec,
        latency_ms_p50: p50,
        latency_ms_p95: p95,
        latency_ms_p99: p99,
        errors: errors + connect_errors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn benchmark_runs_and_produces_report() {
        let cfg = BenchConfig {
            clients: 16,
            messages_per_client: 100,
            payload_bytes: 32,
            echo: true,
        };
        let report = run_bench(&cfg).await.expect("压测应成功");

        assert_eq!(report.connects, cfg.clients, "所有客户端应连上");
        assert_eq!(report.connect_errors, 0, "不应有连接失败");
        assert_eq!(
            report.messages_target,
            cfg.clients * cfg.messages_per_client,
            "目标消息数"
        );
        assert_eq!(
            report.messages_sent,
            cfg.clients * cfg.messages_per_client,
            "应全部发送"
        );
        assert_eq!(
            report.messages_received,
            report.messages_sent,
            "回显应全部收回（服务端接收 == 发送）"
        );
        assert!(report.msg_per_sec > 0.0, "吞吐应为正");
        assert!(report.latency_ms_p99 >= report.latency_ms_p50, "p99 ≥ p50");
        assert_eq!(report.errors, 0, "不应有错误");
    }

    #[tokio::test]
    async fn benchmark_non_echo_counts_server_received() {
        let cfg = BenchConfig {
            clients: 8,
            messages_per_client: 50,
            payload_bytes: 16,
            echo: false,
        };
        let report = run_bench(&cfg).await.expect("压测应成功");
        assert_eq!(report.connects, cfg.clients);
        assert_eq!(report.messages_sent, cfg.clients * cfg.messages_per_client);
        assert_eq!(
            report.messages_received,
            report.messages_sent,
            "非回显模式服务端仍应全部收到"
        );
        assert_eq!(report.latency_ms_p50, 0.0, "非回显模式无 RTT");
    }
}
