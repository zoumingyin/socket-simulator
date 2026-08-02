@echo off
REM Socket Service Manager - dev launcher (Tauri)
REM Backend is now Rust, integrated into src-tauri and auto-started by Tauri.
REM No separate Node backend needed.
npm run tauri -- dev
