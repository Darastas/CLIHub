@echo off
rem Install 'clihub' as a terminal command:
rem build release, copy to %LOCALAPPDATA%\Programs\clihub, add to User PATH.
setlocal
set DEST=%LOCALAPPDATA%\Programs\clihub
set SCRIPT_DIR=%~dp0

echo [clihub] Building release...
call cargo build --release --manifest-path "%SCRIPT_DIR%..\Cargo.toml"
if errorlevel 1 (
    echo [clihub] build failed.
    exit /b 1
)

if not exist "%DEST%" mkdir "%DEST%"
copy /y "%SCRIPT_DIR%..\target\release\clihub.exe" "%DEST%\clihub.exe" >nul
echo [clihub] installed: %DEST%\clihub.exe

rem Add dir to User PATH safely (does not truncate System PATH).
powershell -NoProfile -Command ^
  "$d = Join-Path $env:LOCALAPPDATA 'Programs\clihub';" ^
  "$up = [Environment]::GetEnvironmentVariable('Path','User');" ^
  "if ($up -notlike \"*$d*\") { [Environment]::SetEnvironmentVariable('Path', ($up.TrimEnd(';') + ';' + $d), 'User'); Write-Host '[clihub] PATH updated' } else { Write-Host '[clihub] PATH already set' }"

echo [clihub] Done. Restart your terminal, then type: clihub
endlocal
