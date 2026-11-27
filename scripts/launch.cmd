@echo off
rem Book Keeper launcher: runs the optimized release exe (target\release\book-keeper.exe).
rem launch.ps1 pulls updates from GitHub, rebuilds the exe only when the code changed,
rem starts the app and closes this console. No installer is used.
title Book Keeper
cd /d "%~dp0.."
rem Kept on one line with exit: cmd reads .cmd files line by line, and a git pull may rewrite this file.
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0launch.ps1" & exit /b
