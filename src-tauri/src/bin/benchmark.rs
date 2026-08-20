//! 内置压测 CLI（v3 P2-3）：`cargo run --bin benchmark`
//!
//! 在进程内启动真实 `WsServer` 并以回显模式压测，输出可读报告。
//! 可用环境变量覆盖默认规模：BENCH_CLIENTS / BENCH_MESSAGES / BENCH_PAYLOAD / BENCH_NO_ECHO=1。

// 工具 bin：编译整个 backend 树但仅复用其中一部分（压测），
// dead_code / unused_imports 是预期视角，非真实缺陷（主 bin socket-service-manager 不受影响）
#[path = "../backend/mod.rs"]
#[allow(dead_code, unused_imports)]
mod backend;

fn main() {
    let mut cfg = backend::benchmark::BenchConfig::default();
    if let Ok(v) = std::env::var("BENCH_CLIENTS") {
        if let Ok(n) = v.parse() {
            cfg.clients = n;
        }
    }
    if let Ok(v) = std::env::var("BENCH_MESSAGES") {
        if let Ok(n) = v.parse() {
            cfg.messages_per_client = n;
        }
    }
    if let Ok(v) = std::env::var("BENCH_PAYLOAD") {
        if let Ok(n) = v.parse() {
            cfg.payload_bytes = n;
        }
    }
    if std::env::var("BENCH_NO_ECHO").is_ok() {
        cfg.echo = false;
    }

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("构建 tokio runtime 失败");
    let report = rt
        .block_on(backend::benchmark::run_bench(&cfg))
        .expect("压测执行失败");

    println!("=== NexSocket Studio 内置压测报告 ===");
    println!("服务端:          {}", report.server_url);
    println!("并发客户端:      {}", report.clients);
    println!("连接成功/失败:   {} / {}", report.connects, report.connect_errors);
    println!("目标消息数:      {}", report.messages_target);
    println!("已发送:          {}", report.messages_sent);
    println!("服务端接收:      {}", report.messages_received);
    println!("耗时:            {} ms", report.duration_ms);
    println!("吞吐:            {:.1} msg/s", report.msg_per_sec);
    if cfg.echo {
        println!(
            "RTT p50/p95/p99:  {:.2} / {:.2} / {:.2} ms",
            report.latency_ms_p50, report.latency_ms_p95, report.latency_ms_p99
        );
    }
    println!("错误:            {}", report.errors);
}
