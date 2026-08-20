@echo off
chcp 65001 >nul 2>&1
title Work-Review 一键启动

echo ============================================
echo    Work-Review 一键启动脚本
echo ============================================
echo.

cd /d "%~dp0"

echo [1/2] 启动 Vite 前端开发服务器 (http://localhost:5173) ...
start "Vite Dev Server" cmd /k "npx vite --host"

timeout /t 3 /nobreak >nul

echo [2/2] 启动 Tauri 桌面端 (含后端 API 服务器) ...
start "Tauri Desktop" cmd /k "npx tauri dev"

echo.
echo ============================================
echo    所有服务已启动！
echo ============================================
echo.
echo    网页端:  http://localhost:5173
echo    API:    http://127.0.0.1:47831
echo    桌面端:  Tauri 窗口
echo.
echo    关闭此窗口不会影响已启动的服务
echo    如需停止，请关闭对应的命令行窗口
echo ============================================
echo.

pause