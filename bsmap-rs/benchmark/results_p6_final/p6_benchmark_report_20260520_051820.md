# BSMAP-rs P6 优化基准测试报告

**测试时间**: Wed May 20 05:18:20 CST 2026
**测试环境**: WSL2 (Ubuntu)
**测试版本**: P6 最终优化版本

## 测试数据集

| 数据集 | 类型 | 读长 | 覆盖度 |
|--------|------|------|--------|
| Ex1 SE | 单端 | 75bp | 10x |
| Ex2 PE | 双端 | 150bp | 10x |

## 性能测试结果

### Ex1 SE (单端75bp)
- 单线程: 13.246039350s
- 4线程:  12.499136822s
- 加速比:  1.05x

### Ex2 PE (双端150bp)
- 单线程: 14.238943325s
- 4线程:  12.446731223s
- 加速比:  1.14x

## 比对结果统计

### Ex1 SE
- SAM行数: 3

### Ex2 PE
- SAM行数: 3

## 测试文件位置

- 单线程结果: /mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs/benchmark/results_p6_final/single/
- 4线程结果: /mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs/benchmark/results_p6_final/4threads/
- 详细日志: *.log 文件

