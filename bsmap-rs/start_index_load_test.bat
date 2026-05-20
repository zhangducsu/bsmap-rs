@echo off
cd /d "%~dp0"
echo Building Docker image...
docker build -t bsmap-rs-test .
echo.
echo Running Mmap index loading comparison test in Docker...
docker run --rm -it -v "%cd%:/workspace/bsmap-rs" -w /workspace/bsmap-rs/benchmark bsmap-rs-test bash -c "cd /workspace/bsmap-rs && cargo build --release && cd benchmark && chmod +x compare_index_loading.sh && ./compare_index_loading.sh"
echo.
echo Done! Check benchmark/results/ for report.
pause
