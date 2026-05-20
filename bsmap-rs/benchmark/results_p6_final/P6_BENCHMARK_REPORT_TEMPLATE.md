# BSMAP-rs P6 优化基准测试报告

**测试日期**: 2026-05-18
**测试环境**: WSL2 Ubuntu
**测试版本**: P6 (P0-P6完整优化)
**报告生成时间**: $(date '+%Y-%m-%d %H:%M:%S')

---

## 一、测试概述

### 1.1 测试目标

- 验证P6版本在WSL2环境下的性能表现
- 对比BSMAP-rs与C++ BSMAP的性能差异
- 分析SAM比对结果一致性
- 生成完整的性能剖析数据

### 1.2 测试数据集

| 数据集 | 描述 | 读段数 | 读段长度 |
|--------|------|--------|----------|
| **Ex1 SE** | WGBS单端测试 | ~66,000 | 75bp |
| **Ex2 PE** | WGBS双端测试 | ~33,000 pairs | 150bp |
| **参考基因组** | chr22_tail_1M | 1M bp | - |

### 1.3 测试环境

- **操作系统**: WSL2 Ubuntu
- **CPU**: Intel/AMD (支持AVX2)
- **内存**: >= 8GB
- **编译器**: Rust 1.70+
- **编译标志**: `--release -C target-cpu=native`

---

## 二、测试命令与参数

### 2.1 编译命令

```bash
cd bsmap-rs
RUSTFLAGS='-C target-cpu=native' cargo build --release
```

### 2.2 BSMAP-rs 测试命令

#### 单线程测试

```bash
# Ex1 SE 单线程
./target/release/bsmap \
    -a data/wgbs/ex1_se75_10x/simulated.fastq.gz \
    -d data/chr22_tail_1M.fa \
    -p 1 \
    -o results_p6_final/single/ex1_se_rust.sam

# Ex2 PE 单线程
./target/release/bsmap \
    -a data/wgbs/ex2_pe150_10x/simulated_1.fastq.gz \
    -b data/wgbs/ex2_pe150_10x/simulated_2.fastq.gz \
    -d data/chr22_tail_1M.fa \
    -p 1 \
    -o results_p6_final/single/ex2_pe_rust.sam
```

#### 多线程测试

```bash
# Ex1 SE 4线程
./target/release/bsmap \
    -a data/wgbs/ex1_se75_10x/simulated.fastq.gz \
    -d data/chr22_tail_1M.fa \
    -p 4 \
    -o results_p6_final/multi/ex1_se_rust_4t.sam

# Ex2 PE 4线程
./target/release/bsmap \
    -a data/wgbs/ex2_pe150_10x/simulated_1.fastq.gz \
    -b data/wgbs/ex2_pe150_10x/simulated_2.fastq.gz \
    -d data/chr22_tail_1M.fa \
    -p 4 \
    -o results_p6_final/multi/ex2_pe_rust_4t.sam
```

### 2.3 C++ BSMAP 测试命令

```bash
# Ex1 SE
./bsmap \
    -a data/wgbs/ex1_se75_10x/simulated.fastq.gz \
    -d data/chr22_tail_1M.fa \
    -p 1 \
    -o results_p6_final/single/ex1_se_cpp.sam

# Ex2 PE
./bsmap \
    -a data/wgbs/ex2_pe150_10x/simulated_1.fastq.gz \
    -b data/wgbs/ex2_pe150_10x/simulated_2.fastq.gz \
    -d data/chr22_tail_1M.fa \
    -p 1 \
    -o results_p6_final/single/ex2_pe_cpp.sam
```

---

## 三、性能测试结果

### 3.1 执行时间对比

| 测试用例 | 线程数 | C++ BSMAP | BSMAP-rs | 性能提升 | 加速比 |
|---------|--------|-----------|----------|----------|--------|
| Ex1 SE 75bp | 1 | TBD | TBD | TBD | TBD |
| Ex2 PE 150bp | 1 | TBD | TBD | TBD | TBD |
| Ex1 SE 75bp | 4 | N/A | TBD | - | TBD |
| Ex2 PE 150bp | 4 | N/A | TBD | - | TBD |

### 3.2 内存占用对比

| 测试用例 | 线程数 | C++ BSMAP | BSMAP-rs | 内存节省 |
|---------|--------|-----------|----------|----------|
| Ex1 SE 75bp | 1 | TBD | TBD | TBD |
| Ex2 PE 150bp | 1 | TBD | TBD | TBD |

### 3.3 多线程加速比

| 测试用例 | 单线程时间 | 4线程时间 | 加速比 | 效率 |
|---------|-----------|-----------|--------|------|
| Ex1 SE 75bp | TBD | TBD | TBD | TBD% |
| Ex2 PE 150bp | TBD | TBD | TBD | TBD% |

---

## 四、SAM比对结果对比

### 4.1 比对统计

#### Ex1 SE 75bp

| 指标 | C++ BSMAP | BSMAP-rs | 差异 |
|------|-----------|----------|------|
| 总读段数 | TBD | TBD | - |
| 唯一比对数 | TBD | TBD | TBD |
| 多重比对数 | TBD | TBD | TBD |
| 未比对数 | TBD | TBD | TBD |
| 唯一比对率 | TBD% | TBD% | TBD% |

#### Ex2 PE 150bp

| 指标 | C++ BSMAP | BSMAP-rs | 差异 |
|------|-----------|----------|------|
| 总读段对数 | TBD | TBD | - |
| 唯一比对数 | TBD | TBD | TBD |
| 多重比对数 | TBD | TBD | TBD |
| 未比对数 | TBD | TBD | TBD |
| 唯一比对率 | TBD% | TBD% | TBD% |

### 4.2 比对一致性分析

#### 位置一致率

| 测试用例 | 比对一致率 | 说明 |
|---------|-----------|------|
| Ex1 SE | TBD% | TBD |
| Ex2 PE | TBD% | TBD |

#### 链方向一致率

| 测试用例 | 正链一致率 | 负链一致率 |
|---------|-----------|-----------|
| Ex1 SE | TBD% | TBD% |
| Ex2 PE | TBD% | TBD% |

### 4.3 CIGAR一致性

| 测试用例 | CIGAR一致率 | 说明 |
|---------|------------|------|
| Ex1 SE | TBD% | TBD |
| Ex2 PE | TBD% | TBD |

---

## 五、性能剖析数据

### 5.1 各模块耗时分布

（使用 `perf` 或 `cargo flamegraph` 生成）

| 模块 | 耗时 | 占比 | 说明 |
|------|------|------|------|
| 索引构建 | TBD | TBD% | - |
| 种子提取 | TBD | TBD% | - |
| Mismatch检测 | TBD | TBD% | P5-1优化 |
| Gap比对 | TBD | TBD% | P5-2优化 |
| 命中收集 | TBD | TBD% | P5-3优化 |
| 读段处理 | TBD | TBD% | P5-4优化 |
| 输出写入 | TBD | TBD% | - |

### 5.2 CPU使用率

| 测试用例 | 线程数 | 平均CPU | 峰值CPU |
|---------|--------|---------|---------|
| Ex1 SE | 1 | TBD% | TBD% |
| Ex1 SE | 4 | TBD% | TBD% |
| Ex2 PE | 1 | TBD% | TBD% |
| Ex2 PE | 4 | TBD% | TBD% |

### 5.3 内存使用曲线

（记录峰值内存和内存增长趋势）

---

## 六、优化效果总结

### 6.1 P0-P6优化清单

| 优化阶段 | 优化内容 | 文件 | 效果 |
|---------|---------|------|------|
| P0-1 | SIMD批量哈希 | alphabet.rs | 10-15% |
| P0-2 | KmerLoc2优化 | reference/index.rs | 内存节省 |
| P0-3 | 无边界检查 | alphabet.rs | 5-10% |
| P1 | 索引预热 | reference/index.rs | PF减少36% |
| P2 | 4线程并行 | align/mod.rs | 3-4x |
| P3 | 提前终止+对象池 | align/*.rs | 1.4-4.6% |
| P4-1 | SIMD种子预取 | align/seed.rs | 预取优化 |
| P4-2 | 索引预取 | reference/index.rs | 10-20% |
| P4-3 | 批量Mismatch | align/mismatch.rs | 预取优化 |
| P4-4 | 配对哈希索引 | pairs/pair_index.rs | 2-5x |
| P4-5 | 线程本地对象池 | align/pool.rs | 15-25% |
| P5-1 | AVX2向量化Mismatch | align/mismatch.rs | 2-3x |
| P5-2 | Gap算法优化 | align/gap.rs | 1.5-2x |
| P5-3 | 命中收集优化 | align/extend.rs | 20-30% |
| P5-4 | 批量读段并行化 | reads/batch.rs | 2-3x |
| P6-1 | 编译优化 | Cargo.toml | 10-15% |
| P6-2 | 内存布局优化 | util/cache.rs | 缓存友好 |

### 6.2 总体性能提升

| 指标 | C++ BSMAP | BSMAP-rs P0 | BSMAP-rs P6 | 提升 |
|------|-----------|------------|-------------|------|
| 单线程速度 | 基准 | TBD | **TBD** | **TBDx** |
| 4线程速度 | N/A | TBD | **TBD** | **TBDx** |
| 内存占用 | 基准 | TBD | **TBD** | **TBD%↓** |

### 6.3 验收标准达成

| 标准 | 目标 | 实际 | 状态 |
|------|------|------|------|
| 单线程速度 | 2x+ | TBD | TBD |
| 4线程速度 | 4x+ | TBD | TBD |
| 内存优化 | 40%+ | TBD | TBD |
| SAM一致性 | ≥99% | TBD | TBD |

---

## 七、结论与建议

### 7.1 结论

1. **性能表现**: TBD
2. **功能正确性**: TBD
3. **优化效果**: TBD

### 7.2 建议

1. TBD
2. TBD
3. TBD

---

## 八、附录

### A. 测试脚本

- `run_p6_full_benchmark.sh` - 完整基准测试脚本
- `compare_sam.py` - SAM对比脚本

### B. 性能剖析工具

```bash
# perf分析
perf record -g ./target/release/bsmap [参数]
perf report

# flamegraph
cargo flamegraph --bin bsmap -- [参数]
```

### C. 测试数据来源

- 测试数据由之前的模拟生成
- 参考基因组: chr22_tail_1M.fa (1M bp)

---

**报告生成工具**: BSMAP-rs Benchmark Suite
**报告版本**: P6-v1.0
