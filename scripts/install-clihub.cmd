@echo off
rem 把 clihub 安装为终端命令：构建 release、复制到 %LOCALAPPDATA%\Programs\clihub、
rem 并加入用户 PATH。之后在任何终端输入 clihub 即可打开。
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

rem 用 PowerShell 安全地把目录加入"用户" PATH（不截断系统 PATH）
powershell -NoProfile -Command ^
  "$d = Join-Path $env:LOCALAPPDATA 'Programs\clihub';" ^
  "$up = [Environment]::GetEnvironmentVariable('Path','User');" ^
  "if ($up -notlike \"*$d*\") { [Environment]::SetEnvironmentVariable('Path', ($up.TrimEnd(';') + ';' + $d), 'User'); Write-Host '[clihub] PATH updated' } else { Write-Host '[clihub] PATH already set' }"

echo [clihub] Done. Restart your terminal, then type: clihub
endlocal
