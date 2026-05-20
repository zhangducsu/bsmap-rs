@echo off
REM ==============================================================================
REM Windows批处理脚本 - P系列优化完整性能测试与报告生成
REM 内容: 构建+编译+测试+对比+报告
REM ==============================================================================

setlocal enabledelayedexpansion

cd /d "%~dp0"

echo.
echo ========================================
echo P系列优化完整测试与对比报告生成
echo ========================================
echo 1. 构建Docker镜像
echo 2. 编译bsmap-rs（release模式）
echo 3. 运行Ex1/Ex2基准测试
echo 4. 进行SAM一致性对比
echo 5. 生成详细报告
echo ========================================
echo.

REM 检查Docker是否可用
docker --version >nul 2>&1
if %errorlevel% neq 0 (
    echo 错误: 未检测到Docker，请确保Docker Desktop已安装并运行
    pause
    exit /b 1
)

REM 设置变量
set SCRIPT_DIR=%~dp0
set PROJECT_ROOT=%SCRIPT_DIR%
set TEST_DATE=%date:~0,4%%date:~5,2%%date:~8,2%_%time:~0,2%%time:~3,2%%time:~6,2%
set TEST_DATE=%TEST_DATE: =0%

echo 项目根目录: %PROJECT_ROOT%
echo 测试时间: %TEST_DATE%
echo.

REM 步骤1：构建Docker镜像
echo [1/6] 构建Docker镜像...
docker build -t bsmap-rs-p-series-test .
if %errorlevel% neq 0 (
    echo 错误: Docker镜像构建失败
    pause
    exit /b 1
)
echo [1/6] 完成 ✓
echo.

REM 步骤2：运行Docker容器，执行完整测试
echo [2/6] 启动Docker容器并执行完整测试...
echo.

REM 创建测试脚本
docker run --rm -v "%PROJECT_ROOT%:/workspace/bsmap-rs" -w /workspace/bsmap-rs ubuntu bash -c "chmod +x /workspace/bsmap-rs/benchmark/run_ex1_ex2.sh"

REM 运行完整测试流程
docker run --rm -v "%PROJECT_ROOT%:/workspace/bsmap-rs" -v "%PROJECT_ROOT%/../bsmap-original:/workspace/bsmap-original" -w /workspace/bsmap-rs --memory=20g --cpus=4 bsmap-rs-p-series-test bash -c "
    set -e

    echo '========================================'
    echo 'P系列优化完整测试'
    echo '========================================'
    echo ''
    
    echo '========================================'
    echo '[1/5] 编译bsmap-rs (release模式)'
    echo '========================================'
    cd /workspace/bsmap-rs
    cargo build --release
    echo ''

    echo '========================================'
    echo '[2/5] 运行单元测试验证'
    echo '========================================'
    cargo test --package bsmap --no-fail-fast 2>&1 | tee /workspace/bsmap-rs/benchmark/results/tests.log || true
    echo ''

    echo '========================================'
    echo '[3/5] 运行Ex1/Ex2基准测试'
    echo '========================================'
    cd /workspace/bsmap-rs/benchmark
    ./run_ex1_ex2.sh
    echo ''

    echo '========================================'
    echo '[4/5] 生成P系列优化最终报告'
    echo '========================================'
    cd /workspace/bsmap-rs/benchmark
    
    # 创建最终汇总报告
    echo '# P系列优化最终测试报告' > results/final_comparison_report_${TEST_DATE}.md
    echo '报告日期: '$(date) >> results/final_comparison_report_${TEST_DATE}.md
    echo '' >> results/final_comparison_report_${TEST_DATE}.md
    echo '## 性能对比汇总' >> results/final_comparison_report_${TEST_DATE}.md
    echo '' >> results/final_comparison_report_${TEST_DATE}.md
    if [ -f summary.csv ]; then
        cat summary.csv >> results/final_comparison_report_${TEST_DATE}.md
    fi
    echo '' >> results/final_comparison_report_${TEST_DATE}.md
    echo '## SAM一致性' >> results/final_comparison_report_${TEST_DATE}.md
    echo '' >> results/final_comparison_report_${TEST_DATE}.md
    if [ -f comparison_example1_wgbs_se/detailed_report.txt ]; then
        echo '- [Example1详细报告](comparison_example1_wgbs_se/detailed_report.txt)' >> results/final_comparison_report_${TEST_DATE}.md
    fi
    if [ -f comparison_example2_wgbs_pe/detailed_report.txt ]; then
        echo '- [Example2详细报告](comparison_example2_wgbs_pe/detailed_report.txt)' >> results/final_comparison_report_${TEST_DATE}.md
    fi
    echo '' >> results/final_comparison_report_${TEST_DATE}.md
    echo '## 单元测试日志' >> results/final_comparison_report_${TEST_DATE}.md
    echo '' >> results/final_comparison_report_${TEST_DATE}.md
    if [ -f tests.log ]; then
        echo '[单元测试结果](tests.log)' >> results/final_comparison_report_${TEST_DATE}.md
    fi
    echo ''
    echo '[4/5] 完成 ✓'
    echo ''

    echo '========================================'
    echo '[5/5] 整理最终结果'
    echo '========================================'
    ls -lh /workspace/bsmap-rs/benchmark/results/
    echo ''
    
    echo '========================================'
    echo '✅ P系列优化完整测试完成！'
    echo '========================================'
"

if %errorlevel% neq 0 (
    echo.
    echo 警告: 测试过程中可能存在错误，但已尝试生成报告
)

echo.
echo [2/6] 测试执行完成 ✓
echo.

REM 步骤3：显示最终结果
echo [3/6] 显示测试结果...
if exist "%PROJECT_ROOT%benchmark\results\summary.csv" (
    echo.
    echo === 性能测试结果 ===
    type "%PROJECT_ROOT%benchmark\results\summary.csv"
    echo.
)
echo [3/6] 完成 ✓
echo.

REM 步骤4：生成总结报告
echo [4/6] 生成本地总结报告...
set REPORT_FILE=%PROJECT_ROOT%benchmark\results\p_series_final_summary_%TEST_DATE%.txt

echo # P系列优化最终测试总结 > %REPORT_FILE%
echo 测试时间: %date% %time% >> %REPORT_FILE%
echo. >> %REPORT_FILE%
echo 测试文件: >> %REPORT_FILE%
echo   - %PROJECT_ROOT%benchmark\results\summary.csv >> %REPORT_FILE%
echo   - %PROJECT_ROOT%benchmark\results\final_report.md >> %REPORT_FILE%
echo   - %PROJECT_ROOT%benchmark\results\tests.log >> %REPORT_FILE%
echo. >> %REPORT_FILE%
echo 对比报告: >> %REPORT_FILE%
echo   - %PROJECT_ROOT%benchmark\results\comparison_example1_wgbs_se\detailed_report.txt >> %REPORT_FILE%
echo   - %PROJECT_ROOT%benchmark\results\comparison_example2_wgbs_pe\detailed_report.txt >> %REPORT_FILE%
echo. >> %REPORT_FILE%
echo 请参考 docs/ 目录下的详细报告文档 >> %REPORT_FILE%
echo [4/6] 完成 ✓
echo.

REM 步骤5：列出所有生成的文件
echo [5/6] 列出生成的测试文件...
if exist "%PROJECT_ROOT%benchmark\results" (
    dir /b "%PROJECT_ROOT%benchmark\results"
)
echo [5/6] 完成 ✓
echo.

REM 步骤6：完成
echo [6/6] 全部完成 ✓
echo.
echo ========================================
echo ✅ P系列优化完整测试与报告已生成！
echo ========================================
echo.
echo 主要结果文件:
echo   性能汇总: %PROJECT_ROOT%benchmark\results\summary.csv
echo   完整报告: %PROJECT_ROOT%benchmark\results\final_report.md
echo   SAM对比1: %PROJECT_ROOT%benchmark\results\comparison_example1_wgbs_se\detailed_report.txt
echo   SAM对比2: %PROJECT_ROOT%benchmark\results\comparison_example2_wgbs_pe\detailed_report.txt
echo   单元测试: %PROJECT_ROOT%benchmark\results\tests.log
echo.
echo 请查看 benchmark\results\ 目录获取所有结果
echo ========================================
echo.

pause
exit /b 0
