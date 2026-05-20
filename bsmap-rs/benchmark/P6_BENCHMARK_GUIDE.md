# BSMAP-rs P6 基准测试指南

## 测试环境准备

### 1. WSL2环境检查

```bash
# 检查WSL2是否已安装
wsl --list -v

# 如果未安装，安装Ubuntu
wsl --install -d Ubuntu
```

### 2. 依赖安装

```bash
# 更新包列表
sudo apt update

# 安装编译工具
sudo apt install -y build-essential curl git

# 安装Rust (如果尚未安装)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# 安装perf (性能剖析工具)
sudo apt install -y linux-tools-generic
```

### 3. 克隆或更新代码

```bash
cd /mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs
git pull origin main  # 如果已有仓库
```

## 编译BSMAP-rs

### 标准编译

```bash
cd bsmap-rs
cargo build --release
```

### 优化编译（推荐）

```bash
# 针对本机CPU优化编译
RUSTFLAGS='-C target-cpu=native' cargo build --release
```

### 验证编译

```bash
# 检查二进制文件
ls -lh target/release/bsmap

# 查看版本信息
./target/release/bsmap --help
```

## 运行基准测试

### 方法1: 使用完整测试脚本（推荐）

```bash
cd bsmap-rs/benchmark

# 添加执行权限
chmod +x run_p6_full_benchmark.sh

# 运行完整测试
./run_p6_full_benchmark.sh
```

### 方法2: 手动运行测试

#### 单线程测试

```bash
cd bsmap-rs

# Ex1 SE 单线程
./target/release/bsmap \
    -a benchmark/data/wgbs/ex1_se75_10x/simulated.fastq.gz \
    -d benchmark/data/chr22_tail_1M.fa \
    -p 1 \
    -o benchmark/results_p6_final/single/ex1_se_rust.sam

# Ex2 PE 单线程
./target/release/bsmap \
    -a benchmark/data/wgbs/ex2_pe150_10x/simulated_1.fastq.gz \
    -b benchmark/data/wgbs/ex2_pe150_10x/simulated_2.fastq.gz \
    -d benchmark/data/chr22_tail_1M.fa \
    -p 1 \
    -o benchmark/results_p6_final/single/ex2_pe_rust.sam
```

#### 多线程测试

```bash
cd bsmap-rs

# Ex1 SE 4线程
./target/release/bsmap \
    -a benchmark/data/wgbs/ex1_se75_10x/simulated.fastq.gz \
    -d benchmark/data/chr22_tail_1M.fa \
    -p 4 \
    -o benchmark/results_p6_final/multi/ex1_se_rust_4t.sam

# Ex2 PE 4线程
./target/release/bsmap \
    -a benchmark/data/wgbs/ex2_pe150_10x/simulated_1.fastq.gz \
    -b benchmark/data/wgbs/ex2_pe150_10x/simulated_2.fastq.gz \
    -d benchmark/data/chr22_tail_1M.fa \
    -p 4 \
    -o benchmark/results_p6_final/multi/ex2_pe_rust_4t.sam
```

## SAM对比分析

### 运行SAM对比

```bash
cd bsmap-rs/benchmark

# Ex1 SE SAM对比
python3 compare_sam.py \
    results_p6_final/single/ex1_se_cpp.sam \
    results_p6_final/single/ex1_se_rust.sam \
    results_p6_final/sam_compare/ex1_se_compare.txt

# Ex2 PE SAM对比
python3 compare_sam.py \
    results_p6_final/single/ex2_pe_cpp.sam \
    results_p6_final/single/ex2_pe_rust.sam \
    results_p6_final/sam_compare/ex2_pe_compare.txt
```

## 性能剖析

### 使用perf

```bash
cd bsmap-rs

# 记录性能数据
sudo perf record -g ./target/release/bsmap \
    -a benchmark/data/wgbs/ex1_se75_10x/simulated.fastq.gz \
    -d benchmark/data/chr22_tail_1M.fa \
    -p 1 \
    -o benchmark/results_p6_final/profile/ex1_se.perf.sam

# 生成报告
sudo perf report

# 生成火焰图
perf script | stackcollapse-perf.pl | flamegraph.pl > benchmark/results_p6_final/profile/flamegraph.svg
```

### 使用time

```bash
cd bsmap-rs

# 测量执行时间和内存
/usr/bin/time -v ./target/release/bsmap \
    -a benchmark/data/wgbs/ex1_se75_10x/simulated.fastq.gz \
    -d benchmark/data/chr22_tail_1M.fa \
    -p 1 \
    -o benchmark/results_p6_final/single/ex1_se_rust.sam
```

## 查看测试结果

### 结果目录结构

```
benchmark/results_p6_final/
├── single/
│   ├── ex1_se_rust.sam          # Ex1 SE Rust结果
│   ├── ex1_se_rust.log          # Ex1 SE Rust日志
│   ├── ex1_se_cpp.sam           # Ex1 SE C++结果
│   ├── ex1_se_cpp.log           # Ex1 SE C++日志
│   ├── ex2_pe_rust.sam          # Ex2 PE Rust结果
│   ├── ex2_pe_rust.log          # Ex2 PE Rust日志
│   ├── ex2_pe_cpp.sam           # Ex2 PE C++结果
│   └── ex2_pe_cpp.log           # Ex2 PE C++日志
├── multi/
│   ├── ex1_se_rust_4t.sam       # Ex1 SE 4线程结果
│   ├── ex1_se_rust_4t.log       # Ex1 SE 4线程日志
│   ├── ex2_pe_rust_4t.sam      # Ex2 PE 4线程结果
│   └── ex2_pe_rust_4t.log       # Ex2 PE 4线程日志
├── sam_compare/
│   ├── ex1_se_compare.txt      # Ex1 SE对比结果
│   └── ex2_pe_compare.txt      # Ex2 PE对比结果
├── profile/
│   ├── ex1_se.perf.data        # perf数据
│   └── flamegraph.svg          # 火焰图
└── P6_BENCHMARK_REPORT.md       # 测试报告（需手动填写）
```

### 查看SAM文件

```bash
# 查看前20行
head -20 benchmark/results_p6_final/single/ex1_se_rust.sam

# 统计比对结果
grep -v "^@" benchmark/results_p6_final/single/ex1_se_rust.sam | wc -l

# 统计唯一比对
grep -v "^@" benchmark/results_p6_final/single/ex1_se_rust.sam | grep -c "XS:i:0"

# 统计多重比对
grep -v "^@" benchmark/results_p6_final/single/ex1_se_rust.sam | grep -c "XS:i:[1-9]"
```

## 生成最终报告

测试完成后，编辑 `benchmark/results_p6_final/P6_BENCHMARK_REPORT.md`，将测试数据填入报告模板中的 "TBD" 部分。

## 常见问题

### 1. 编译错误

```bash
# 清理并重新编译
cargo clean
RUSTFLAGS='-C target-cpu=native' cargo build --release
```

### 2. 测试数据不存在

```bash
# 检查数据目录
ls -la benchmark/data/

# 如果数据不存在，检查路径
find /mnt/c -name "simulated.fastq.gz" 2>/dev/null
```

### 3. 权限问题

```bash
# 添加执行权限
chmod +x run_p6_full_benchmark.sh
chmod +x target/release/bsmap
```

## 联系方式

如有问题，请查看：
- 项目README: `bsmap-rs/README.md`
- 优化文档: `bsmap-rs/docs/`
