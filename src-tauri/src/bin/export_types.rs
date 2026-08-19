//! 类型生成管道（F-4）：从 `src/backend/types.rs` 用 specta 导出 TS 类型到 `src/types/generated.ts`
//!
//! 运行：`cargo run --bin export_types`
//! 目的：消灭前端手工双份 `src/types`（Rust 为权威源，1:1 导出）
//!
//! 注意：
//! - specta 默认禁止导出 BigInt（u64/i64/i128/i128/usize），因为 TS 无原生对应。
//!   本项目前端统一用 `number` 表示这些字段，故配置 `BigIntExportBehavior::Number`。
//! - specta 1.0.5 的 `ExportConfiguration` 不支持全局 `Option<T>` → `T | undefined` 切换，
//!   `Option<T>` 恒导出为 `T | null`（见 `src/lang/ts/export_config.rs`）。前端类型里
//!   既有 `?` 也有 `| null` 的用法，生成产物以 `| null` 为准；前端消费时按此契约适配。

#[path = "../backend/mod.rs"]
mod backend;

use specta::ts::{export, BigIntExportBehavior, ExportConfiguration};
use crate::backend::types::*;

fn main() {
    let conf = ExportConfiguration::default().bigint(BigIntExportBehavior::Number);
    let mut out = String::new();
    out.push_str("// 本文件由 `cargo run --bin export_types` 自动生成，请勿手工修改。\n");
    out.push_str("// 权威源：src-tauri/src/backend/types.rs（specta 1:1 导出）\n\n");

    macro_rules! exp {
        ($t:ty) => {
            match export::<$t>(&conf) {
                Ok(s) => out.push_str(&s),
                Err(e) => eprintln!("[specta] 导出 {} 失败: {}", stringify!($t), e),
            }
        };
    }

    // 枚举
    exp!(ProtocolType);
    exp!(HttpMethod);
    exp!(HttpRouteType);
    exp!(LogLevel);
    exp!(ServerStatus);
    exp!(EventStatus);
    exp!(ClientStatus);
    exp!(ClientGroupType);
    // 结构体
    exp!(HttpRouteConfig);
    exp!(MockMatchCondition);
    exp!(MockRule);
    exp!(MockServiceConfig);
    exp!(SceneConfig);
    exp!(SceneServerResult);
    exp!(ServerConfig);
    exp!(ServerRuntime);
    exp!(EventConfig);
    exp!(ClientInfo);
    exp!(LogEntry);
    exp!(HeartbeatConfig);
    exp!(IpAccessList);
    exp!(WssConfig);
    exp!(SystemSettings);
    exp!(WindowConfig);
    exp!(WsFrame);
    exp!(PersistedConfig);

    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/types");
    let _ = std::fs::create_dir_all(dir);
    let path = format!("{}/generated.ts", dir);
    if let Err(e) = std::fs::write(&path, out) {
        eprintln!("写入 {} 失败: {}", path, e);
        std::process::exit(1);
    }
    println!("[specta] 已生成 {}", path);
}
