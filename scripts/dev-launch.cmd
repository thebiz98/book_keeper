@echo off
rem Book Keeper dev launcher: runs the app straight from source with `tauri dev`.
rem No installer and no release build. The frontend hot-reloads on save and the Rust
rem side recompiles on change. dev-launch.ps1 closes this console when the app closes;
rem closing this console window also closes the app.
title Book Keeper (dev)
cd /d "%~dp0.."
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0dev-launch.ps1"
