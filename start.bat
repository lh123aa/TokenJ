@echo off
chcp 65001 > nul
echo ========================================
echo   TokenJ - 快速启动
echo ========================================
echo.

cd /d "%~dp0"

:: 1. 注入演示数据
echo [1/3] 注入演示数据...
python scripts/seed_demo_data.py
if %ERRORLEVEL% NEQ 0 ( echo 数据注入失败 & pause & exit /b 1 )

:: 2. 测试 MCP 连接
echo [2/3] 测试 MCP Server...
python scripts/test_mcp.py
if %ERRORLEVEL% NEQ 0 ( echo MCP 测试失败 & pause & exit /b 1 )

:: 3. 运行单元测试
echo [3/3] 验证源码...
cd t: 2>nul || subst t: "%CD%" 2>nul
echo.
echo ========================================
echo   全部就绪！使用方法：
echo ========================================
echo.
echo   TokenJ 终端        - 任意位置输入 TokenJ --help
echo   TokenJ demo        - 启动演示模式
echo   TokenJ dashboard   - 实时仪表盘
echo.
echo   MCP 已配置到 .trae/mcp.json
echo   重启 Trae 后自动生效
echo.
echo ========================================
pause
