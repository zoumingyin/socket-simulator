//! TcpListener 绑定 + 端口冲突释放重试（≡ Node `start` bind→killPort→rebind）
//!
//! 受管服务（Ws/Http/Unified/SocketIo）与 Mock 自定义端口原本各有 1 处复制的
//! `bind → AddrInUse → release_port → sleep → rebind` 逻辑，现统一收敛到
//! [`bind_with_release`]（P1-2）。
//!
//! 错误处理刻意保持与原各传输层 `start()` 内联 match 完全一致：
//! - 首次成功 → `Ok(listener)`；
//! - `AddrInUse` → 告警 + `release_port` + 等待 [`PORT_RELEASE_RETRY_DELAY_MS`] + 重试；
//! - 其余错误（含重试失败）经 `From<io::Error>` 透传为 [`BackendError`]，不改变错误码语义。

use tokio::net::TcpListener;

use crate::backend::constants::PORT_RELEASE_RETRY_DELAY_MS;
use crate::backend::error::BackendError;
use crate::backend::net::port_release::release_port;

/// 绑定 `ip:port`，若端口被占用则释放占用进程后重试一次。
///
/// 调用点传入的 `(ip, port)` 元组与原 `TcpListener::bind((ip, port))` 一致。
pub async fn bind_with_release(ip: &str, port: u16) -> Result<TcpListener, BackendError> {
    let addr = (ip, port);
    match TcpListener::bind(addr).await {
        Ok(listener) => Ok(listener),
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            eprintln!("[net::bind] 端口 {} 被占用，尝试释放后重试", port);
            release_port(port);
            tokio::time::sleep(std::time::Duration::from_millis(
                PORT_RELEASE_RETRY_DELAY_MS,
            ))
            .await;
            TcpListener::bind(addr).await.map_err(|e| e.into())
        }
        Err(e) => Err(e.into()),
    }
}
