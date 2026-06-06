@echo off
REM 启动后端（在新窗口中运行）
start "Backend" cmd /c "cd backend && npm run dev"

REM 等待后端启动（可选）
timeout /t 2 /nobreak > nul

REM 启动前端（在当前进程运行，这样 Tauri 可以接管）
npm run dev
