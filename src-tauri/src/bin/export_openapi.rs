//! OpenAPI 3.1 导出管道（v3 P0-4）：把后端 REST 契约导出为 `openapi.json`。
//!
//! 运行：`cargo run --bin export_openapi`
//! 输出：仓库根 `openapi.json`（供 `openapi-typescript` 生成前端类型）。
//!
//! 权威源（决策门 2，2026-08-19）：OpenAPI 3.1 为契约权威；
//! F-4 的 specta `generated.ts` 在 v3 迁移期保留作过渡，完成后退役。

// 工具 bin：编译整个 backend 树但仅复用其中一部分（OpenAPI 导出），
// dead_code / unused_imports 是预期视角，非真实缺陷（主 bin socket-service-manager 不受影响）
#[path = "../backend/mod.rs"]
#[allow(dead_code, unused_imports)]
mod backend;

fn main() {
    let spec = backend::openapi::openapi_json().0;
    let pretty = serde_json::to_string_pretty(&spec).expect("OpenAPI spec 应可序列化");
    let out_path = std::path::Path::new("openapi.json");
    std::fs::write(out_path, pretty).expect("写入 openapi.json 失败");
    println!(
        "[export_openapi] 已导出 {} 到 {}",
        spec["openapi"].as_str().unwrap_or("?"),
        out_path.display()
    );
    let paths = spec["paths"]
        .as_object()
        .map(|p| p.len())
        .unwrap_or(0);
    let schemas = spec["components"]["schemas"]
        .as_object()
        .map(|s| s.len())
        .unwrap_or(0);
    println!("[export_openapi] paths: {}，schemas: {}", paths, schemas);
}
