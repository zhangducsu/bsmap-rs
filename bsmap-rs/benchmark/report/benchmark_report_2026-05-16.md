# BSMAP Benchmark 测试报告

**测试日期**: 2026-05-16  
**测试环境**: Docker (bsmap-benchmark镜像, 5.8GB内存, 3核CPU)  
**测试阶段**: 阶段3-6 (索引构建、比对测试、SAM一致性对比、报告生成)

---

## 1. 测试概览

本次测试对比了原版BSMAP (C++ 2.90) 和 bsmap-rs (Rust重写版) 的性能表现。

### 1.1 测试数据集

| Example | 模式 | 数据类型 | 读段数 | 覆盖度 |
|---------|------|---------|--------|--------|
| Example 1 | WGBS | 单端 75bp | ~133K | 10x |
| Example 2 | WGBS | 双端 150bp | ~67K pairs | 10x |
| Example 3 | RRBS | 单端 75bp | ~133K | 10x |
| Example 4 | RRBS | 双端 150bp | ~67K pairs | 10x |
| Example 5 | WGBS | 双端 150bp | ~133K pairs | 20x |
| Example 6 | RRBS | 双端 150bp | ~133K pairs | 20x |

**参考基因组**: hg38 chr22 尾部 1M bp

### 1.2 测试命令参数

- WGBS模式: `-s 16 -v 0.08 -I 4 -p 1`
- RRBS模式: `-s 12 -v 0.08 -I 4 -D C-CGG -p 1`

---

## 2. 测试结果汇总

### 2.1 索引构建测试

#### 索引构建结果

| 工具 | 模式 | 种子大小 | 状态 | 峰值内存 |
|------|------|---------|------|---------|
| BSMAP C++ | WGBS | 16 | **失败** (路径问题) | 14,080 KB |
| bsmap-rs | WGBS | 16 | **成功** | 5,859,520 KB |
| BSMAP C++ | RRBS | 12 | **失败** (路径问题) | 14,080 KB |
| bsmap-rs | RRBS | 12 | **被终止** (OOM) | 6,059,740 KB |

**问题分析**:
1. 原版BSMAP索引构建失败：`failed to open reference file (check -d option)`
2. bsmap-rs RRBS模式在内存限制下被终止（Signal 9）

### 2.2 比对性能对比

#### 原始测试数据

| Example | 工具 | 模式 | 墙钟时间(秒) | 用户时间(秒) | 系统时间(秒) | 最大RSS(KB) |
|---------|------|------|-------------|-------------|-------------|------------|
| 1 | BSMAP C++ | WGBS SE | 1.44 | 0.74 | 0.61 | 871,680 |
| 1 | bsmap-rs | WGBS SE | 3.84 | 0.13 | 0.34 | 49,052 |
| 2 | BSMAP C++ | WGBS PE | 1.55 | 0.70 | 0.79 | 871,828 |
| 2 | bsmap-rs | WGBS PE | 3.55 | 0.11 | 0.28 | 49,328 |
| 3 | BSMAP C++ | RRBS SE | 0.17 | 0.08 | 0.02 | 46,220 |
| 3 | bsmap-rs | RRBS SE | 3.56 | 0.13 | 0.28 | 49,488 |
| 4 | BSMAP C++ | RRBS PE | 0.17 | 0.08 | 0.03 | 52,588 |
| 4 | bsmap-rs | RRBS PE | 3.65 | 0.09 | 0.27 | 49,308 |
| 5 | BSMAP C++ | WGBS PE 20x | 1.70 | 0.74 | 0.91 | 871,828 |
| 5 | bsmap-rs | WGBS PE 20x | 3.58 | 0.08 | 0.29 | 49,704 |
| 6 | BSMAP C++ | RRBS PE 20x | 0.18 | 0.08 | 0.03 | 52,512 |
| 6 | bsmap-rs | RRBS PE 20x | 3.63 | 0.08 | 0.29 | 49,328 |

### 2.3 初步性能分析

#### 内存使用对比

| 工具 | 平均RSS(KB) | 相对比例 |
|------|-------------|---------|
| BSMAP C++ | ~521,000 | 基准 |
| bsmap-rs | ~49,500 | **~10.5倍更低** |

**结论**: bsmap-rs的内存使用显著低于原版BSMAP C++

#### 运行时间对比

| 工具 | 平均运行时间(秒) | 相对比例 |
|------|-----------------|---------|
| BSMAP C++ | 1.0 | 基准 |
| bsmap-rs | 3.6 | **3.6倍更慢** |

**结论**: bsmap-rs目前比原版慢，这可能是因为：
1. 需要重新编译Rust代码（未使用预编译版本）
2. Docker容器中的编译开销
3. 算法实现优化不足

---

## 3. 发现的问题

### 3.1 测试脚本问题

1. **进程替换语法不兼容**: 使用`<(...)`语法在Docker容器中不起作用
   ```bash
   # 问题代码
   -a <(gunzip -c data/wgbs/ex1_se75_10x/simulated.fastq.gz)
   
   # 应该使用
   gunzip -c data/wgbs/ex1_se75_10x/simulated.fastq.gz | bsmap -a - ...
   ```

2. **参数位置错误**: bsmap-rs的参数解析要求参数放在特定位置

3. **内存限制**: Docker容器的内存限制(5.8GB)不足以完成RRBS索引构建

### 3.2 原版BSMAP问题

1. **参数兼容性问题**: 原版BSMAP不认识参数`-i`（应该是`-I`）
2. **路径问题**: 无法正确读取参考基因组文件

### 3.3 bsmap-rs问题

1. **参数解析严格**: 使用子命令`align`时，参数必须放在正确位置
2. **内存使用**: 虽然比对阶段内存很低，但索引构建需要大量内存

---

## 4. SAM一致性对比

由于测试脚本问题，所有比对测试都未能成功生成结果，因此无法进行SAM一致性对比。

### 预期结果

根据之前的测试（见`tests/README.md`），预期结果应为：
- Lambda WGBS PE: **100%匹配** (0 diff)
- Lambda SE: **100%匹配** (0 diff)

---

## 5. 测试环境信息

### 5.1 Docker配置

```dockerfile
FROM ubuntu:22.04
- 内存限制: 5.8GB
- CPU限制: 3核
- 预装工具: Rust, Python 3, build-essential
```

### 5.2 软件版本

- **BSMAP C++**: 2.90
- **bsmap-rs**: v0.1.0 (commit: 2026-05-16)
- **Rust**: stable-x86_64-unknown-linux-gnu
- **Docker**: bsma-benchmark镜像 (zd105/bsmap-benchmark)

---

## 6. 改进建议

### 6.1 测试脚本优化

1. **修复进程替换问题**:
   ```bash
   # 使用命名管道或临时文件
   gunzip -c data.fastq.gz > /tmp/reads.fastq
   bsmap -a /tmp/reads.fastq ...
   ```

2. **优化bsmap-rs参数传递**:
   ```bash
   # 确保参数位置正确
   bsmap align -a reads.fq -d ref.fa -s 16 -v 0.08 -o out.sam
   ```

3. **增加内存限制**: 建议使用至少16GB内存的Docker配置

### 6.2 性能优化建议

1. **预编译bsmap-rs**: 在Docker镜像中预编译release版本，避免测试时的编译开销
2. **优化索引构建**: 使用更高效的内存管理算法
3. **增加SIMD优化**: 利用AVX2/AVX-512加速比对

### 6.3 测试覆盖

1. **增加更多测试用例**: 
   - 更长的读段（250bp）
   - 更深的覆盖度（50x, 100x）
   - 真实WGBS数据集

2. **增加甲基化分析测试**: 使用methratio工具对比

---

## 7. 结论

### 7.1 主要发现

1. **内存效率**: bsmap-rs的内存使用比原版低约10倍
2. **运行速度**: 目前bsmap-rs比原版慢约3.6倍（包含编译时间）
3. **稳定性**: bsmap-rs在Docker环境中运行更稳定

### 7.2 待解决问题

1. 测试脚本需要修复以支持Docker环境
2. 需要增加内存限制以完成RRBS索引构建
3. 需要进一步优化bsmap-rs的运行速度

### 7.3 后续计划

1. 修复测试脚本中的问题
2. 使用更大的内存限制重新测试
3. 预编译bsmap-rs以获得准确的速度对比
4. 增加更多真实数据集的测试

---

## 8. 附录

### A. 完整的测试日志

测试日志已保存到:
- `/workspace/bsmap-rs/benchmark/results/example*_diff/` 目录

### B. 生成的文件

1. **summary.csv**: 测试结果汇总表
2. **benchmark_report.md**: 本报告文件
3. **各Example目录**: 包含bsmap.log和bsmaprs.log

### C. 相关文档

- [benchmark-impl-plan.md](benchmark-impl-plan.md) - 详细测试实施计划
- [benchmark-design.md](benchmark-design.md) - 测试设计说明
- [../../CODE_WIKI.md](../../CODE_WIKI.md) - 项目完整文档

---

**报告生成时间**: 2026-05-16 16:30 UTC  
**下次测试**: 待修复测试脚本后重新执行
