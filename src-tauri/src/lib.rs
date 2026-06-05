#![cfg_attr(mobile, mobile_app)]
#![allow(dead_code)]

use tauri::{
    Builder, App, AppHandle, Manager, Runtime,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    image::Image,
};

const MENU_SHOW: &str = "show";
const MENU_START_ALL: &str = "start_all";
const MENU_STOP_ALL: &str = "stop_all";
const MENU_RESTART_ALL: &str = "restart_all";
const MENU_QUIT: &str = "quit";

fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

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
        "启动全部",
        true,
        None::<&str>,
    )?;
    let stop_item = MenuItem::with_id(
        app,
        MENU_STOP_ALL,
        "停止全部",
        true,
        None::<&str>,
    )?;
    let restart_item = MenuItem::with_id(
        app,
        MENU_RESTART_ALL,
        "重启全部",
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

fn setup_tray<R: Runtime>(app: &App<R>) -> tauri::Result<()> {
    let app_handle = app.handle().clone();
    let tray_menu = create_tray_menu(&app_handle)?;

    // 加载图标
    let icon_result = Image::from_path("icons/icon.png");

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
                    println!("[tray] start_all clicked");
                }
                MENU_STOP_ALL => {
                    println!("[tray] stop_all clicked");
                }
                MENU_RESTART_ALL => {
                    println!("[tray] restart_all clicked");
                }
                MENU_QUIT => {
                    app.exit(0);
                }
                _ => {}
            }
        });

    // 设置图标
    match icon_result {
        Ok(img) => {
            builder = builder.icon(img);
        }
        Err(e) => {
            eprintln!("加载托盘图标失败: {}", e);
        }
    }

    builder.build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    Builder::default()
        .setup(|app| {
            setup_tray(app)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("运行 Tauri 应用失败");
}
