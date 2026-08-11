@echo off
REM Ensure Rust + MinGW (dlltool) are on PATH for Tauri builds in fresh/stale terminals.
set "PATH=%USERPROFILE%\.cargo\bin;C:\mingw64\bin;%PATH%"
%*
