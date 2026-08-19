//! 传输层模块（≡ ITransport）

pub mod adapter;
pub mod transport;
pub mod websocket;
pub mod socketio;
pub mod http;
pub mod unified;
pub mod hooks;
pub mod http_routing;
pub mod ws_connection;

pub use adapter::{AdapterKind, AdapterRegistry, ProtocolAdapter};
pub use transport::Transport;
pub use hooks::TransportHooks;
