@echo off
REM =============================================================================
REM 快速验证脚本 - 验证P系列优化代码修改
REM =============================================================================
cd /d "%~dp0"

echo.
echo ========================================
echo P系列优化代码快速验证
echo ========================================
echo.

REM 检查Git状态
echo [1/3] 检查Git状态...
if exist ".git" (
    git status
) else (
    echo Git未初始化，跳过
)
echo [1/3] 完成
echo.

REM 列出关键修改文件
echo [2/3] 检查修改的文件...
echo.
echo 关键修改文件:
echo.

if exist "bsmap\src\alphabet.rs" (
    echo   [MODIFIED] bsmap\src\alphabet.rs ^(P0-1 SIMD, P0-3 unchecked^)
)
if exist "bsmap\src\align\seed.rs" (
    echo   [MODIFIED] bsmap\src\align\seed.rs ^(P0-3集成^)
)
if exist "bsmap\src\param.rs" (
    echo   [MODIFIED] bsmap\src\param.rs ^(P0-2结构^)
)
if exist "bsmap\src\reference\prefetch.rs" (
    echo   [NEW] bsmap\src\reference\prefetch.rs ^(P1预热^)
)
if exist "bsmap\src\main.rs" (
    echo   [MODIFIED] bsmap\src\main.rs ^(P1集成^)
)
if exist "bsmap\src\cli.rs" (
    echo   [MODIFIED] bsmap\src\cli.rs ^(选项^)
)
echo.
echo [2/3] 完成
echo.

REM 检查docs目录
echo [3/3] 检查文档...
if exist "docs" (
    echo 文档目录:
    dir /b "docs\*.md"
)
echo [3/3] 完成
echo.

echo ========================================
echo 快速验证完成！
echo ========================================
echo.
echo 下一步:
echo   1. 运行 start_p_series_test.bat 进行完整测试
echo   2. 或参考 TEST_GUIDE.md 了解详细步骤
echo.
echo 主要修改:
echo   - P0-1: SIMD优化 (alphabet.rs)
echo   - P0-2: 索引结构优化 (param.rs)
echo   - P0-3: 热点路径边界检查 (seed.rs)
echo   - P1: 索引预热 (prefetch.rs)
echo.
pause
