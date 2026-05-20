# BSMAP-rs P6 基准测试最终报告

**测试日期**: 2026-05-20
**测试环境**: WSL2 Ubuntu
**测试版本**: P6 (完整优化链 - SIMD + 索引优化 + 并行优化)

---

## 一、性能测试结果

### 1.1 执行时间

| 测试用例 | 线程数 | 执行时间 |
|---------|--------|---------|
| Ex1 SE 75bp | 1 | 20.09s |
| Ex2 PE 150bp | 1 | 22.59s |
| Ex1 SE 75bp | 4 | 18.22s |
| Ex2 PE 150bp | 4 | 19.60s |

---

## 二、比对统计

### 2.1 Ex1 SE 75bp (单端)

| 指标 | 数值 |
|------|------|
| 比对读段数 | 66118 |
| 唯一比对数 | 55948 |
| 多重比对数 | 10170 |

### 2.2 Ex2 PE 150bp (双端)

| 指标 | 数值 |
|------|------|
| 配对比对数 | 33478 |
| 唯一配对比数 | 31821 |
| 多重配对比数 | 1657 |
| 单端比对 (read_a) | 0 |
| 单端比对 (read_b) | 1 |

---

## 三、结果文件位置

- 单线程结果: [results_p6_final/single/](file:///c:/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs/benchmark/results_p6_final/single/)
- 4线程结果: [results_p6_final/4threads/](file:///c:/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs/benchmark/results_p6_final/4threads/)
