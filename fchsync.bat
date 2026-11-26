@echo off
rem Double-click to start syncing. Put your settings in fchsync.env next to this file:
rem   TRACKER_SERVER=http://host:8088
rem   TRACKER_TOKEN=secret
rem   FCH_DIR=optional characters folder
if exist "%~dp0fchsync.env" (
  for /f "usebackq eol=# tokens=1,* delims==" %%a in ("%~dp0fchsync.env") do set "%%a=%%b"
)
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0fchsync.ps1" %*
pause
