//! frontend.rs —— 前端静态资源嵌入与服务
//!
//! 使用 `rust-embed` 将 Vite 构建产物 `../dist/` 编入二进制，
//! 由 axum 统一路由提供，实现「前端也是一个路由」。
//!
//! ## 路由优先级（在 app.rs fallback 中链式调用）
//!
//! 1. `/api/*`        → REST API（显式路由）
//! 2. `/admin/ws`     → 管理 WebSocket（显式路由）
//! 3. Mock basePath   → Mock 引擎分发（dispatch_main_port 返回 Some 时）
//! 4. 静态文件         → `dist/assets/*.js|css|...`（本模块 serve_static）
//! 5. SPA fallback    → `index.html`（React Router 客户端路由）
//!
//! ## 开发 vs 生产
//!
//! - **生产**：`dist/` 在 `tauri build` 前由 `tsc + vite build` 生成，嵌入二进制
//! - **开发**：`dist/` 可能不存在或过期；Tauri webview 仍从 Vite dev server 加载，
//!   但通过浏览器直接访问 `http://localhost:3080/` 会得到嵌入的（可能过期的）页面

use axum::body::Body;
use axum::http::{header, HeaderMap, HeaderValue, Response, StatusCode};
use axum::response::IntoResponse;
use rust_embed::RustEmbed;

/// 嵌入前端构建产物（路径相对于 src-tauri/ crate root）
#[derive(RustEmbed)]
#[folder = "../dist/"]
struct FrontendAsset;

/// 根据文件扩展名推断 Content-Type
fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "application/javascript; charset=utf-8",
        Some("mjs") => "application/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("eot") => "application/vnd.ms-fontobject",
        Some("map") => "application/json; charset=utf-8",
        Some("webp") => "image/webp",
        Some("avif") => "image/avif",
        Some("wasm") => "application/wasm",
        _ => "application/octet-stream",
    }
}

/// 构造文件响应（含 Content-Type + 缓存头）
fn file_response(path: &str, data: &[u8]) -> Response<Body> {
    let ct = content_type(path);
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(ct).unwrap_or(HeaderValue::from_static("application/octet-stream")),
    );

    // Vite 构建的 assets/ 下文件名带 hash，可永久缓存；
    // index.html 不缓存（确保用户拿到最新版本入口）
    if path.starts_with("assets/") {
        headers.insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=31536000, immutable"),
        );
    } else {
        headers.insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-cache"),
        );
    }

    (StatusCode::OK, headers, Body::from(data.to_vec())).into_response()
}

/// 尝试为给定路径提供静态文件
///
/// - `path` 是 URL path（如 `/assets/index-abc.js`、`/`、`/mock`）
/// - 优先查找对应的嵌入文件
/// - 找不到时回退到 `index.html`（SPA 模式，React Router 处理客户端路由）
/// - `index.html` 也不存在时返回 JSON 404
pub fn serve(path: &str) -> Response<Body> {
    // 规范化：去掉前导 /
    let clean = path.trim_start_matches('/');

    // 1. 尝试精确匹配文件（如 assets/index-abc.js、favicon.ico）
    if !clean.is_empty() {
        if let Some(file) = FrontendAsset::get(clean) {
            return file_response(clean, file.data.as_ref());
        }
    }

    // 2. SPA 回退：返回 index.html，由 React Router 处理路由
    if let Some(file) = FrontendAsset::get("index.html") {
        return file_response("index.html", file.data.as_ref());
    }

    // 3. 前端未构建（dist/ 为空）
    let body = serde_json::json!({
        "error": "Frontend Not Built",
        "message": "前端未构建，请先运行 npm run build；开发模式请通过 Tauri 窗口访问",
        "status": 404,
    });
    let body = serde_json::to_string_pretty(&body).unwrap_or_else(|_| body.to_string());
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    (StatusCode::NOT_FOUND, headers, Body::from(body)).into_response()
}

/// 检查前端是否已嵌入（dist/ 非空）
pub fn is_embedded() -> bool {
    FrontendAsset::get("index.html").is_some()
}
