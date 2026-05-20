@echo off
cd /d "%~dp0"
echo Building Docker image...
docker build -t bsmap-rs-test .
echo.
echo Running Mmap test in Docker...
docker run --rm -it -v "%cd%:/workspace/bsmap-rs" -w /workspace/bsmap-rs bsmap-rs-test bash -c "cd /workspace/bsmap-rs && cargo build --release && cd benchmark && ./test_mmap_fix.sh"
echo.
echo Done!
pause
