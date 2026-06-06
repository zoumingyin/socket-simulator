// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use serde_json::Value;
use tauri::{
    App, AppHandle, Emitter, Manager, Runtime,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    image::Image,
    WindowEvent,
};

const MENU_SHOW: &str = "show";
const MENU_START_ALL: &str = "start_all";
const MENU_STOP_ALL: &str = "stop_all";
const MENU_RESTART_ALL: &str = "restart_all";
const MENU_QUIT: &str = "quit";

/// 从配置文件读取 startMinimized 配置
fn should_start_minimized() -> bool {
    let paths = vec![
        PathBuf::from("../config/config.json"),
        PathBuf::from("../../config/config.json"),
        PathBuf::from("config/config.json"),
    ];

    for path in &paths {
        if let Ok(mut file) = File::open(path) {
            let mut contents = String::new();
            if file.read_to_string(&mut contents).is_ok() {
                if let Ok(json) = serde_json::from_str::<Value>(&contents) {
                    if let Some(system) = json.get("systemSettings") {
                        if let Some(minimized) = system.get("startMinimized") {
                            return minimized.as_bool().unwrap_or(false);
                        }
                    }
                }
            }
        }
    }

    false
}

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
    let show_item = MenuItem::with_id(
        app,
        MENU_SHOW,
        "显示主界面",
        true,
        None::<&str>,
    )?;
    let start_item = MenuItem::with_id(
        app,
        MENU_START_ALL,
        "启动全部服务",
        true,
        None::<&str>,
    )?;
    let stop_item = MenuItem::with_id(
        app,
        MENU_STOP_ALL,
        "停止全部服务",
        true,
        None::<&str>,
    )?;
    let restart_item = MenuItem::with_id(
        app,
        MENU_RESTART_ALL,
        "重启全部服务",
        true,
        None::<&str>,
    )?;
    let quit_item = MenuItem::with_id(
        app,
        MENU_QUIT,
        "退出",
        true,
        None::<&str>,
    )?;
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
fn setup_tray(app: &App) -> tauri::Result<()> {
    let app_handle = app.handle().clone();
    let tray_menu = create_tray_menu(&app_handle)?;

    // 加载图标 - 尝试多个路径
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
                    // 通过事件通知前端启动全部服务
                    let _ = app.emit("tray-start-all", "start-all");
                }
                MENU_STOP_ALL => {
                    let _ = app.emit("tray-stop-all", "stop-all");
                }
                MENU_RESTART_ALL => {
                    let _ = app.emit("tray-restart-all", "restart-all");
                }
                MENU_QUIT => {
                    // 真正退出应用
                    app.exit(0);
                }
                _ => {}
            }
        });

    // 设置图标
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

fn main() {
    // 检查是否应该启动时最小化
    let start_minimized = should_start_minimized();
    println!("[main] startMinimized = {}", start_minimized);

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![])
        .setup(move |app| {
            // 获取 Tauri 自动创建的窗口（根据 tauri.conf.json 的 app.windows 配置）
            let window = app.get_webview_window("main").unwrap();

            // 根据配置决定是否显示窗口
            if start_minimized {
                println!("[main] 启动时最小化到托盘");
                let _ = window.hide();
                let _ = window.set_skip_taskbar(true);
            } else {
                let _ = window.show();
                let _ = window.set_skip_taskbar(false);
            }

            // 设置托盘
            setup_tray(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            match event {
                WindowEvent::CloseRequested { api, .. } => {
                    // 拦截关闭请求，改为隐藏到托盘
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
