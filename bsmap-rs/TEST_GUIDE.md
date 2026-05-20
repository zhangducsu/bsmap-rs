# P系列优化测试与对比指南

## 快速开始

### 1. 确保Docker Desktop已启动
打开 Docker Desktop，确保它正在运行。

### 2. 运行测试脚本
在 Windows 上，双击运行：
```
start_p_series_test.bat
```

或者在命令行中：
```cmd
cd c:\Users\zhang_i5edc0\OneDrive\Documents\TraeSOLO\BSMAP\bsmap-rs
start_p_series_test.bat
```

## 测试说明

### 测试内容
- **编译**: bsmap-rs (release模式)
- **单元测试**: 验证代码正确性
- **基准测试1**: Ex1 (WGBS SE 75bp 10x)
- **基准测试2**: Ex2 (WGBS PE 150bp 10x)
- **SAM对比**: 与 C++ BSMAP 的结果一致性验证
- **报告生成**: 性能对比和一致性分析

### 预计耗时
- 首次构建Docker镜像: 5-10分钟
- 编译 bsmap-rs: 3-5分钟
- 完整测试运行: 15-30分钟
- **总耗时**: 25-45分钟

## 结果查看

### 测试完成后的结果文件位置
```
c:\Users\zhang_i5edc0\OneDrive\Documents\TraeSOLO\BSMAP\bsmap-rs\benchmark\results\
├── summary.csv                     # 性能数据汇总 (时间/内存)
├── final_report.md                 # 最终测试报告
├── tests.log                       # 单元测试日志
├── P_SERIES_FINAL_REPORT.md        # P系列优化完整报告
├── example1_wgbs_se_bsmap/         # C++ BSMAP Ex1结果
├── example1_wgbs_se_bsmaprs/       # bsmap-rs Ex1结果
├── example2_wgbs_pe_bsmap/         # C++ BSMAP Ex2结果
├── example2_wgbs_pe_bsmaprs/       # bsmap-rs Ex2结果
├── comparison_example1_wgbs_se/    # Ex1 SAM一致性对比
└── comparison_example2_wgbs_pe/    # Ex2 SAM一致性对比
```

### 主要报告内容
1. **summary.csv** - 性能数据 (运行时间/内存使用)
2. **final_report.md** - 完整的基准测试报告
3. **comparison_*/detailed_report.txt** - SAM一致性详细分析

### P系列优化进度文档
```
c:\Users\zhang_i5edc0\OneDrive\Documents\TraeSOLO\BSMAP\bsmap-rs\docs\
├── P0-1_SIMD_optimization_final_report.md
├── P0-2_index_optimization_final_report.md
├── P0-3_hotpath_optimization_report.md
├── P1_index_loading_optimization_report.md
└── P_series_optimization_final_report.md
```

## 手动测试步骤

如果需要手动运行测试，也可以分步骤执行：

### 1. 构建Docker镜像
```cmd
cd c:\Users\zhang_i5edc0\OneDrive\Documents\TraeSOLO\BSMAP\bsmap-rs
docker build -t bsmap-rs-test .
```

### 2. 运行测试容器
```cmd
docker run --rm -it -v "%cd%:/workspace/bsmap-rs" -v "%cd%/../bsmap-original:/workspace/bsmap-original" -w /workspace/bsmap-rs --memory=20g --cpus=4 bsmap-rs-test bash
```

### 3. 在容器内编译
```bash
cargo build --release
```

### 4. 运行单元测试
```bash
cargo test --package bsmap
```

### 5. 运行基准测试
```bash
cd benchmark
./run_ex1_ex2.sh
```

## 测试数据

### Example 1 (WGBS Single-End)
- 读长: 75bp
- 覆盖度: 10x
- 模式: 单端
- 文件: `tmp/ex1_se75_10x.fastq`

### Example 2 (WGBS Paired-End)
- 读长: 150bp
- 覆盖度: 10x
- 模式: 双端
- 文件: `tmp/ex2_pe150_10x_1.fastq` 和 `tmp/ex2_pe150_10x_2.fastq`

### 参考基因组
- 长度: 1Mbp (chr22 tail)
- 文件: `data/chr22_tail_1M.fa`

## SAM一致性指标

### 主要验证内容
1. **位置一致性**: ≥98%
2. **链方向一致性**: ≥99%
3. **比对统计相似性**: 总比对数/唯一比对数/多重比对数
4. **MAPQ分布**: 合理的质量分数分配

### 检查方法
在 `benchmark/results/comparison_*/detailed_report.txt` 查看完整对比。

## 常见问题

### Docker未找到
确保已安装Docker Desktop并正在运行。

### 权限错误
Windows上确保有足够权限，或右键"以管理员身份运行"。

### 测试时间过长
- 第一次运行需要构建Docker镜像和编译Rust，之后会快一些
- 如果只修改了代码，可以跳过构建步骤
- 使用 `--no-cache` 的Docker构建会更快

### 内存不足
确保Docker设置中分配了足够内存 (建议≥20GB)。

## 下一步

测试完成后，请查看：
1. `benchmark/results/summary.csv` - 性能数据
2. `benchmark/results/P_SERIES_FINAL_REPORT.md` - 最终报告
3. `docs/P_series_optimization_final_report.md` - P系列优化总结文档
