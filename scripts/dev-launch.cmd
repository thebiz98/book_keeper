@echo off
rem Book Keeper dev launcher: runs the app straight from source with `tauri dev`.
rem No installer and no release build. The frontend hot-reloads on save and the Rust
rem side recompiles on change. Closing this console window also closes the app.
title Book Keeper (dev)
cd /d "%~dp0.."

if not exist node_modules (
  echo Installing npm dependencies...
  call npm install || goto :fail
)

call npx tauri dev
if errorlevel 1 goto :fail
exit /b 0

:fail
echo.
echo Book Keeper dev launcher failed. See the output above.
pause
exit /b 1
