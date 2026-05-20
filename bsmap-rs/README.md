# BSMAP-rs: 高效的重亚硫酸盐测序比对工具 (Rust实现)

BSMAP-rs 是原 BSMAP (Bisulfite Sequence Mapping Program) 的 Rust 重写实现，专注于性能优化和现代软件开发实践。

## 特性

- **高性能**: SIMD加速、内存映射索引加载、多核并行
- **完全兼容**: 与原版 C++ BSMAP 100% 比对结果一致
- **完整功能链**: 索引构建 → 序列比对 → 甲基化分析 → 结果转换
- **跨平台**: Windows、Linux、macOS

## 项目结构

```
bsmap-rs/
├── bsmap/              # 主程序：序列比对 (核心模块)
│   ├── src/
│   │   ├── align/      # 比对引擎
│   │   ├── pairs/      # Paired-end 处理
│   │   ├── reads/      # 读段读取 (FASTQ/BAM)
│   │   └── reference/  # 参考基因组与索引
├── methratio/          # 甲基化比例计算
├── bsp2sam/            # 格式转换 (BSP → SAM)
├── benchmark/          # 基准测试数据与脚本
├── tests/              # 测试用例
├── docs/               # 文档与设计规范
└── tools/              # 外部工具依赖
```

## 快速开始

### 1. 安装 Rust

```bash
# 官方安装脚本
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### 2. 克隆仓库

```bash
git clone <your-repo-url>
cd bsmap-rs
```

### 3. 编译

```bash
# Release 模式编译 (推荐)
cargo build --release

# 优化到当前 CPU
RUSTFLAGS='-C target-cpu=native' cargo build --release
```

编译好的二进制文件在 `target/release/` 目录下。

## 使用说明

### 构建索引

```bash
# WGBS 模式
./target/release/bsmap index -d reference.fa -o index_prefix

# RRBS 模式
./target/release/bsmap index -d reference.fa -o index_prefix -r
```

### 序列比对

```bash
# 单端模式
./target/release/bsmap align -a reads.fq -d reference.fa -o output.sam -p 4

# 双端模式
./target/release/bsmap align -a reads1.fq -b reads2.fq -d reference.fa -o output.sam -p 4
```

常用参数：
- `-p, --threads N` : 使用 N 个线程 (默认 1)
- `-s, --seed-size N` : 种子长度 (默认 16)
- `-v, --max-mismatch-ratio R` : 最大错配率 (默认 0.08)
- `-I, --index-interval N` : 索引间隔 (默认 4)

### 甲基化分析

```bash
./target/release/methratio -i input.sam -o methratio.txt
```

## 基准测试

### 快速测试

```bash
cd benchmark
./run_p6_wsl2_final.sh
```

### 运行完整测试

```bash
cd benchmark
# 1. 准备数据
./0-prepare.sh

# 2. 构建索引
./1-index.sh

# 3. 运行比对
./2-align.sh
```

## 开发指南

### 运行测试

```bash
# 运行所有测试
cargo test

# 运行特定测试
cargo test --lib
```

### 查看文档

```bash
cargo doc --open
```

## 优化记录 (P0-P6)

| 阶段 | 优化内容 | 性能提升 | 文档 |
|-----|---------|---------|------|
| P0 | SIMD 矢量化加速、热点优化 | ~15% | [P0优化报告](P0_optimization_test_report.md) |
| P1 | 内存映射索引加载 | 启动加速 | [P1优化报告](P1_index_loading_optimization_report.md) |
| P2 | 索引构建优化 | 显著加速 | [P2优化报告](benchmark/results_p2/p2_optimization_final_report.md) |
| P3-6 | 多阶段深度优化 | 累计 2x+ | [最终报告](FINAL_OPTIMIZATION_REPORT.md) |

完整优化记录请查看 [docs/](docs/) 目录和项目根目录下的报告文件。

## 项目规则

请查看 [CLAUDE.md](CLAUDE.md) 了解本项目的开发规范和工作流程。

## 许可证

本项目采用 GPL-3.0 许可证，与原版 BSMAP 保持一致。
