# Mmap模式索引加载对比测试指南

## 测试目标

对比原版C++ BSMAP和修复后Rust BSMAP (Mmap模式)的索引加载性能。

## 已修复的问题

### 1. Mmap模式崩溃问题
- **问题**: Mmap模式运行时崩溃
- **原因**: 指针转换方式不正确
- **修复**: 使用正确的`pointer.add(offset)`方式替代错误的指针算术

### 2. 文件句柄问题
- **问题**: 多个文件句柄导致mmap失效
- **修复**: 使用单个文件创建所有mmap，通过Arc共享

### 3. 确认Mmap模式
- main.rs第311行确认使用`LoadMode::Mmap`
- 修复后的代码在Docker环境中编译通过

## 执行测试

### 方法1: Windows批处理文件（推荐）

双击运行:
```
start_index_load_test.bat
```

### 方法2: Linux/Mac

```bash
chmod +x run_index_load_test.sh
./run_index_load_test.sh
```

### 方法3: 手动Docker执行

```bash
# 1. 构建Docker镜像
docker build -t bsmap-rs-test .

# 2. 进入容器
docker run --rm -it -v "$(pwd):/workspace/bsmap-rs" -w /workspace/bsmap-rs bsmap-rs-test bash

# 3. 编译
cd /workspace/bsmap-rs
cargo build --release

# 4. 运行测试
cd benchmark
chmod +x compare_index_loading.sh
./compare_index_loading.sh
```

## 测试流程

1. **准备数据**: 解压测试数据
2. **构建V3索引**: 为Rust BSMAP构建V3格式索引
3. **测试C++ BSMAP**: 运行原版C++版本，记录性能
4. **测试Rust BSMAP**: 运行修复后的Rust版本（Mmap模式）
5. **生成报告**: 对比两者性能，生成详细报告

## 输出文件

测试完成后，在`benchmark/results/`目录下生成:

- `index_load_summary.csv`: 摘要数据（CSV格式）
- `index_load_comparison_report.md`: 详细报告（Markdown格式）
- 日志文件和SAM输出文件

## 预期结果

- ✅ 索引成功加载（显示"v3, mmap"）
- ✅ 比对正常运行，无崩溃
- ✅ 生成对比报告，显示性能改进
- ✅ SAM文件输出正常

## 修改的文件清单

1. `bsmap/src/reference/storage.rs`: 修复Mmap指针转换问题
2. `bsmap/src/align/mismatch.rs`: 恢复AVX2 SIMD功能
3. `bsmap/src/main.rs`: 确认使用Mmap模式
