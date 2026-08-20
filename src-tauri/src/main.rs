// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod backend;

use tauri::{
    AppHandle, Emitter, Manager, Runtime,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    image::Image,
    WindowEvent,
};

use crate::backend::app::Backend;
use crate::backend::run as run_backend;

const MENU_SHOW: &str = "show";
const MENU_START_ALL: &str = "start_all";
const MENU_STOP_ALL: &str = "stop_all";
const MENU_RESTART_ALL: &str = "restart_all";
const MENU_QUIT: &str = "quit";

/// 显示主窗口
fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        let _ = window.set_skip_taskbar(false);
    }
}

/// 创建托盘菜单
fn create_tray_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let show_item = MenuItem::with_id(app, MENU_SHOW, "显示主界面", true, None::<&str>)?;
    let start_item = MenuItem::with_id(app, MENU_START_ALL, "启动全部服务", true, None::<&str>)?;
    let stop_item = MenuItem::with_id(app, MENU_STOP_ALL, "停止全部服务", true, None::<&str>)?;
    let restart_item =
        MenuItem::with_id(app, MENU_RESTART_ALL, "重启全部服务", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, MENU_QUIT, "退出", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;

    Menu::with_items(app, &[
        &show_item,
        &start_item,
        &stop_item,
        &restart_item,
        &separator,
        &quit_item,
    ])
}

/// 设置托盘
fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let app_handle = app.handle().clone();
    let tray_menu = create_tray_menu(&app_handle)?;

    let icon_paths = vec![
        "icons/icon.png",
        "../icons/icon.png",
        "../../src-tauri/icons/icon.png",
    ];

    let mut builder = TrayIconBuilder::new()
        .menu(&tray_menu)
        .show_menu_on_left_click(true)
        .tooltip("Socket 服务管理平台")
        .on_menu_event(move |app, event| {
            match event.id().as_ref() {
                MENU_SHOW => {
                    show_main_window(app);
                }
                MENU_START_ALL => {
                    let _ = app.emit("tray-start-all", "start-all");
                }
                MENU_STOP_ALL => {
                    let _ = app.emit("tray-stop-all", "stop-all");
                }
                MENU_RESTART_ALL => {
                    let _ = app.emit("tray-restart-all", "restart-all");
                }
                MENU_QUIT => {
                    // 退出前优雅关闭后端（停止全部服务，事件轮询随进程退出结束）
                    if let Some(backend) = app.try_state::<Backend>() {
                        let b = backend.inner().clone();
                        tauri::async_runtime::spawn(async move { b.shutdown().await; });
                    }
                    app.exit(0);
                }
                _ => {}
            }
        });

    let mut icon_loaded = false;
    for icon_path in &icon_paths {
        match Image::from_path(icon_path) {
            Ok(img) => {
                builder = builder.icon(img);
                icon_loaded = true;
                break;
            }
            Err(_) => continue,
        }
    }

    if !icon_loaded {
        eprintln!("警告: 无法加载托盘图标");
    }

    builder.build(app)?;
    Ok(())
}

/// 打开开发者工具（F12）
#[tauri::command]
fn open_devtools(app: AppHandle) {
    #[cfg(feature = "devtools")]
    {
        if let Some(window) = app.get_webview_window("main") {
            window.open_devtools();
        }
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![open_devtools])
        .setup(|app| {
            // 获取 Tauri 自动创建的窗口（根据 tauri.conf.json 的 app.windows 配置）
            let window = app.get_webview_window("main").unwrap();

            // 显式设置窗口图标，确保 Windows 任务栏显示应用图标
            // （Tauri 2 的 generate_context 默认图标不一定会应用到窗口任务栏条目）
            if let Some(icon) = app.default_window_icon() {
                let _ = window.set_icon(icon.clone());
            }

            // ===== 启动 Rust 后端（先初始化，确保配置已从 SQLite 主读） =====
            let backend = Backend::new(app.handle().clone());
            app.manage(backend.clone());

            // 启动时总是显示主窗口（v3.0.0：移除「启动最小化到托盘」功能）
            let _ = window.show();
            let _ = window.set_skip_taskbar(false);

            // 设置托盘
            setup_tray(app)?;

            // 启动后端异步任务
            tauri::async_runtime::spawn(run_backend(backend));

            Ok(())
        })
        .on_window_event(|window, event| {
            match event {
                WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    println!("[main] 窗口关闭请求被拦截，隐藏到托盘");
                    let _ = window.hide();
                    let _ = window.set_skip_taskbar(true);
                }
                WindowEvent::Focused(focused) => {
                    if *focused {
                        let _ = window.set_skip_taskbar(false);
                    }
                }
                _ => {}
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
