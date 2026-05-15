@echo off
chcp 65001 > nul
set SCRIPT_PATH=%~dp0..\scripts\TokenJ_mcp_server.py

:: 尝试查找 Python：先查 PATH，再查常见安装路径
where python >nul 2>nul
if %ERRORLEVEL% EQU 0 (
    python "%SCRIPT_PATH%"
    exit /b %ERRORLEVEL%
)

:: Python 不在 PATH，尝试常见安装路径
for %%p in (
    "%LOCALAPPDATA%\Programs\Python\Python312\python.exe"
    "%LOCALAPPDATA%\Programs\Python\Python311\python.exe"
    "%LOCALAPPDATA%\Programs\Python\Python310\python.exe"
    "C:\Python312\python.exe"
    "C:\Python311\python.exe"
    "C:\Python310\python.exe"
    "C:\Program Files\Python312\python.exe"
    "C:\Program Files\Python311\python.exe"
) do (
    if exist %%p (
        %%p "%SCRIPT_PATH%"
        exit /b %ERRORLEVEL%
    )
)

:: 都没找到，报错提示
echo ================================================
echo   TokenJ MCP Server 启动失败
echo ================================================
echo.
echo   未找到 Python。请确保 Python 3.10+ 已安装并加入 PATH。
echo.
echo   安装: https://www.python.org/downloads/
echo.
echo   安装时请勾选 "Add Python to PATH"
echo.
echo   或者手动指定 Python 路径:
echo     将上面的 %%LOCALAPPDATA%%\Programs\Python\Python312\python.exe
echo     替换为你的实际 Python 路径。
echo.
echo ================================================
pause
exit /b 1
