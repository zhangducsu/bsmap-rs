# BSMAP vs BSMAP-rs 基准测试说明

本目录包含用于对比测试原版BSMAP (C++版本)和BSMAP-rs (Rust版本)的所有脚本和数据。

## 文件说明

### 主要脚本

- **`run-phases-3-6.sh`** - Linux shell脚本，执行阶段3-6的完整测试流程
  - 阶段3: 索引构建测试
  - 阶段4: 比对测试 (6个Examples)
  - 阶段5: SAM一致性对比
  - 阶段6: 报告生成

- **`start-benchmark.bat`** - Windows批处理脚本，通过Docker容器运行测试

- **`Dockerfile`** - Docker镜像定义文件 (在bsmap-rs根目录)

### 数据目录

- **`data/`** - 测试数据目录
  - `data/wgbs/` - WGBS模式测试数据
  - `data/rrbs/rrbssim/` - RRBS模式测试数据
  - `data/ref/` - 参考基因组

## 如何运行

### 方法1: Windows环境 (推荐)

1. 确保已安装Docker Desktop并运行
2. 双击运行 `start-benchmark.bat`
3. 等待测试完成，结果将保存在 `results/` 和 `report/` 目录

### 方法2: Linux环境

```bash
cd bsmap-rs/benchmark
chmod +x run-phases-3-6.sh
./run-phases-3-6.sh
```

### 方法3: Docker手动运行

```bash
# 1. 构建Docker镜像
cd bsmap-rs
docker build -t bsmap-benchmark .

# 2. 运行测试
docker run --rm -v /path/to/BSMAP:/workspace -w /workspace/bsmap-rs/benchmark bsmap-benchmark /workspace/bsmap-rs/benchmark/run-phases-3-6.sh
```

## 测试内容

### Example 1: WGBS 单端 75bp 10x
- 模式: WGBS
- 类型: 单端 (SE)
- 读长: 75bp
- 覆盖度: 10x

### Example 2: WGBS 双端 150bp 10x
- 模式: WGBS
- 类型: 双端 (PE)
- 读长: 150bp
- 覆盖度: 10x

### Example 3: RRBS 单端 75bp 10x
- 模式: RRBS
- 类型: 单端 (SE)
- 读长: 75bp
- 覆盖度: 10x

### Example 4: RRBS 双端 150bp 10x
- 模式: RRBS
- 类型: 双端 (PE)
- 读长: 150bp
- 覆盖度: 10x

### Example 5: WGBS 双端 150bp 20x
- 模式: WGBS
- 类型: 双端 (PE)
- 读长: 150bp
- 覆盖度: 20x

### Example 6: RRBS 双端 150bp 20x
- 模式: RRBS
- 类型: 双端 (PE)
- 读长: 150bp
- 覆盖度: 20x

## 输出结果

### 运行过程输出

所有测试的日志会实时输出到控制台，同时保存到各个Example目录下的 `.log` 文件中。

### 最终输出文件

1. **`results/summary.csv`** - 汇总测试结果
   - 运行时间
   - 内存使用
   - 比对统计

2. **`report/benchmark_report.md`** - Markdown格式的详细报告
   - 索引构建对比
   - 比对性能对比
   - SAM一致性分析
   - 结论和建议

3. **`results/example*_diff/`** - 各Example的SAM对比结果
   - `diff_report.txt` - 详细差异报告
   - `sam1_filtered.sam` - 过滤后的BSMAP C++ SAM
   - `sam2_filtered.sam` - 过滤后的BSMAP-rs SAM

4. **`results/example*/`** - 各Example的比对结果
   - `bsmap.sam` - BSMAP C++的输出
   - `bsmaprs.sam` - BSMAP-rs的输出
   - `bsmap.log` - BSMAP C++的日志
   - `bsmaprs.log` - BSMAP-rs的日志

5. **`index/*.bsi`** - 构建的索引文件
   - `bsmap_wgbs.bsi` - BSMAP C++ WGBS索引
   - `bsmaprs_wgbs.bsi` - BSMAP-rs WGBS索引
   - `bsmap_rrbs.bsi` - BSMAP C++ RRBS索引
   - `bsmaprs_rrbs.bsi` - BSMAP-rs RRBS索引

## 注意事项

1. **Docker要求**: 需要至少4GB内存和2个CPU核心
2. **运行时间**: 完整测试可能需要1-3小时，取决于硬件配置
3. **存储空间**: 请确保有至少5GB的可用空间
4. **权限问题**: Windows环境下确保Docker有文件系统访问权限

## 故障排除

### Docker镜像不存在

如果提示镜像 `bsmap-benchmark` 不存在，需要先构建镜像：

```bash
cd bsmap-rs
docker build -t bsmap-benchmark .
```

### 权限被拒绝

Linux环境下可能需要给脚本执行权限：

```bash
chmod +x run-phases-3-6.sh
```

### 测试数据问题

如果测试数据损坏或缺失，请参考 `benchmark/benchmark-impl-plan.md` 中的数据生成步骤重新生成。

## 相关文档

- `benchmark-impl-plan.md` - 详细的测试执行计划
- `benchmark-design.md` - 测试设计说明
- `../CODE_WIKI.md` - 完整的项目文档
