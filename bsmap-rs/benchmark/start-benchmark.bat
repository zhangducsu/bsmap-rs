@echo off
REM Windows批处理脚本 - 启动Docker benchmark测试

cd /d "%~dp0"

echo ========================================
echo 启动 bsmap-benchmark Docker 容器
echo ========================================
echo.

REM 检查Docker是否可用
docker --version >nul 2>&1
if %errorlevel% neq 0 (
    echo 错误: 未检测到Docker，请确保Docker已安装并运行
    pause
    exit /b 1
)

REM 设置工作目录
set PROJECT_ROOT=%~dp0..\..

echo 项目根目录: %PROJECT_ROOT%
echo.

REM 给Linux脚本赋予执行权限（通过Docker容器）
echo 赋予执行脚本权限...
docker run --rm -v "%PROJECT_ROOT%:/workspace" ubuntu chmod +x /workspace/bsmap-rs/benchmark/run-phases-3-6.sh

echo.
echo ========================================
echo 开始执行 benchmark 测试
echo ========================================
echo.

REM 运行测试
docker run --rm -v "%PROJECT_ROOT%:/workspace" -w /workspace/bsmap-rs/benchmark bsmap-benchmark /workspace/bsmap-rs/benchmark/run-phases-3-6.sh

echo.
echo ========================================
echo 测试执行完成！
echo ========================================
echo 结果文件保存在:
echo   - %~dp0results\summary.csv
echo   - %~dp0report\benchmark_report.md
echo.
pause
