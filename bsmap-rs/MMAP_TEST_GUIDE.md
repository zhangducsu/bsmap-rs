# Mmap 模式修复测试指南

## 已修复的问题

**问题**: Mmap 模式运行时崩溃，错误为 `timeout: the monitored command dumped core`

**原因**: 指针转换方式不正确，使用了 `(mmap.as_ptr() as usize + offset) as *const T` 的方式，导致对齐问题。

**修复**: 改用正确的 `pointer.add(offset)` 方式：

```rust
let base_ptr = mmap.as_ptr() as *const u8;
let offset_ptr = base_ptr.add(offset);
std::slice::from_raw_parts(offset_ptr as *const T, len)
```

## 修改的文件

1. `bsmap/src/reference/storage.rs` - 修复了 `MmapStorage` 和 `MmapKmerIndexStorage` 的指针转换
2. `bsmap/src/align/mismatch.rs` - 恢复了 AVX2 SIMD 功能（之前临时禁用用于调试）

## 如何测试

### Windows 方式

双击运行 `start_mmap_test.bat`，或在命令行执行：

```cmd
start_mmap_test.bat
```

### Linux/Mac 方式

```bash
./run_mmap_test_docker.sh
```

### 手动 Docker 测试

```bash
# 构建镜像
docker build -t bsmap-rs-test .

# 运行测试
docker run --rm -it -v "$(pwd):/workspace/bsmap-rs" -w /workspace/bsmap-rs bsmap-rs-test bash

# 在容器内执行
cd /workspace/bsmap-rs
cargo build --release
cd benchmark
chmod +x test_mmap_fix.sh
./test_mmap_fix.sh
```

## 预期结果

- 索引成功以 V3 格式和 Mmap 模式加载
- 比对成功运行，没有 core dump
- 生成 SAM 输出文件
- 测试完成后在 `benchmark/results/` 目录有详细报告

## 验证清单

- [ ] Docker 镜像构建成功
- [ ] 编译成功
- [ ] V3 索引构建成功
- [ ] Mmap 模式索引加载成功
- [ ] 比对正常运行，无崩溃
- [ ] SAM 输出文件生成
- [ ] 报告文件 `mmap_fix_report.md` 生成
