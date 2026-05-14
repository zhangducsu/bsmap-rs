# Phase 6 Spec: methratio 子 crate 开发与端到端测试

> 日期: 2026-05-14
> 状态: Draft
> 范围: 仅 methratio（methdiff、bsp2sam 不在本次范围内）

---

## 1. 目标

用 Rust 重写 C++ BSMAP 附带的 `methratio.py`（257 行 Python），实现甲基化率计算功能。

**核心收益**:
- 内存优化：人类基因组从 ~26GB 降至 <1GB
- 性能提升：Rust 原生速度 + rayon 并行
- 管道集成：`bsmap align ... | methratio -d ref.fa -o out.txt`

## 2. 输入/输出规格

### 2.1 输入

| 输入源 | 格式 | 解析方式 |
|--------|------|----------|
| SAM 文件 | 文本 | noodles `sam::Reader` |
| BAM 文件 | BGZF 二进制 | noodles `bam::Reader` |
| BSP 文件 | 文本（C++ BSMAP 私有格式） | 自定义解析器 |
| STDIN | 自动检测 SAM/BAM（BAM magic） | noodles |

**BSP 格式说明**（tab 分隔，无 header，来自 README.md Section 4.1）:

| 列号 | 字段名 | 说明 |
|------|--------|------|
| 1 | id | read ID |
| 2 | seq | mapped read sequence |
| 3 | qual | quality scores |
| 4 | map_flag | UM=唯一比对, MA=多重比对, OF=超出MAXHITS, NM=无命中, QC=低质量 |
| 5 | ref | 参考序列名（染色体名） |
| 6 | ref_loc | 比对位置（1-based, Watson链 5'端坐标） |
| 7 | strand | ++=BSW, +-=BSWC, -+=BSC, --=BSCC |
| 8 | ins_size | insert size（配对末端总长度），0=单端或未配对 |
| 9 | refseq | Watson链参考序列（含两端各2个小写侧翼碱基） |
| 10 | mismatch | 无gap: #mismatches; 有gap: #mismatches:#gap_size:gap_position (gap_size>0=读段插入, <0=读段缺失) |
| 11 | mismatch_info | 0 mismatch到max mismatch的命中数，用':'分隔 |

**BSP 解析要点**（来自 methratio.py 第 69-80 行）:
- `col[3][:2]` 为 hit type（UM/MA/NM/QC），NM 和 QC 直接跳过
- `col[1]` = 序列, `col[4]` = 染色体, `col[5]` = 位置(转0-based), `col[6]` = strand, `col[7]` = insert size
- `col[9]` = mismatch 信息，含 gap 时格式为 `#mm:#gap_size:gap_pos`
  - `gap_size < 0`: 读段插入（删除读段碱基）: `seq = seq[:gap_pos] + seq[gap_pos-gap_size:]`
  - `gap_size > 0`: 参考缺失（插入'-'占位符）: `seq = seq[:gap_pos] + '-' * gap_size + seq[gap_pos:]`
- BSP 格式**不包含** mate 比对信息，因此**不支持** paired overlap 处理（README 第 298-300 行明确说明）
- BSP 格式**不支持** `--pair` 过滤的完整语义（仅检查 `col[7] == '0'` 判断是否配对）

**BSP vs SAM 差异**:

| 特性 | SAM | BSP |
|------|-----|-----|
| paired overlap 处理 | ✅ 支持（通过 TLEN） | ❌ 不支持（无 mate 信息） |
| `--pair` 过滤 | FLAG 0x2 + 0x8 | 仅检查 insert > 0 |
| CIGAR 解析 | 标准 CIGAR 字符串 | mismatch 字段中的 gap 信息 |
| 链信息 | ZS:Z: tag | col[7] 直接读取 |
| 唯一性判断 | FLAG 0x100 | col[3] == 'UM' |
| 去重 | 基于 (frag_end, direction) | 同 SAM（基于 frag_end, direction） |

### 2.2 输出

**TXT 输出**（默认，与原版一致）:
```
chr	pos	strand	context	ratio	eff_CT_count	C_count	CT_count	rev_G_count	rev_GA_count	CI_lower	CI_upper
```

**WIG 输出**（可选，`--wig` 参数）:
```
variableStep chrom=XXX span=25
pos1	ratio1
pos2	ratio2
```

## 3. CLI 参数

```
methratio [OPTIONS] -d <reference> <input>...

Options:
  -d, --ref <FASTA>          参考基因组 FASTA 文件（必需）
  -o, --out <FILE>           输出文件（默认 STDOUT）
  -O, --alignment-copy <FILE> 保存输入比对的 BAM 副本（用于管道输入）
  -w, --wig <FILE>           输出 WIG 文件
  -b, --wig-bin <INT>        WIG bin 大小（默认 25）
  -c, --chr <CHR>            仅处理指定染色体（逗号分隔）
  -s, --sam-path <PATH>      samtools 路径（默认自动检测）
      --unique               仅唯一比对读段
      --pair                 仅配对比对读段
      --remove-duplicate     去除 PCR 重复
  -t, --trim-fillin <INT>    修剪 fill-in 碱基数（默认 0）
      --combine-cpg          合并双链 CpG 位点
  -m, --min-depth <INT>      最小覆盖深度（默认 1）
  -n, --no-header            不输出 header 行
  -i, --ct-snp <STR>         CT_SNP 处理模式：no-action/correct/skip（默认 correct）
  -x, --context <STR>        上下文过滤：CG/CHG/CHH（逗号分隔，默认全部）
      --zero-meth            报告零甲基化位点（默认启用，已废弃选项始终启用）
      --quiet                不输出进度信息
  -p, --threads <INT>        并行线程数（默认 1）
  -h, --help                 帮助信息

Args:
  <input>...                 输入文件（SAM/BAM/BSP），支持多个文件，省略则从 STDIN 读取
                            格式自动检测：*.sam → SAM, *.bam → BAM, 其他 → BSP
                            STDIN 默认为 SAM（通过 samtools 管道）
```

**与原版 methratio.py 参数对照**:

| 原版参数 | Rust 参数 | 变更说明 |
|---------|----------|----------|
| `-o/--out` | `-o/--out` | 一致 |
| `-O/--alignment-copy` | `-O/--alignment-copy` | 一致 |
| `-w/--wig` | `-w/--wig` | 一致 |
| `-b/--wig-bin` | `-b/--wig-bin` | 一致 |
| `-d/--ref` | `-d/--ref` | 一致 |
| `-c/--chr` | `-c/--chr` | 一致 |
| `-s/--sam-path` | `-s/--sam-path` | Rust 版使用 noodles 原生解析，此参数仅用于 alignment-copy |
| `-u/--unique` | `--unique` | 一致 |
| `-p/--pair` | `--pair` | 一致 |
| `-z/--zero-meth` | `--zero-meth` | 一致（始终启用） |
| `-q/--quiet` | `--quiet` | 一致 |
| `-r/--remove-duplicate` | `--remove-duplicate` | 一致 |
| `-t/--trim-fillin` | `-t/--trim-fillin` | 一致 |
| `-g/--combine-CpG` | `--combine-cpg` | 一致（kebab-case） |
| `-m/--min-depth` | `-m/--min-depth` | 一致 |
| `-n/--no-header` | `-n/--no-header` | 一致 |
| `-i/--ct-snp` | `-i/--ct-snp` | 一致（值：no-action/correct/skip） |
| `-x/--context` | `-x/--context` | 一致 |
| 无 | `-p/--threads` | 新增：并行线程数 |

## 4. 架构设计

### 4.1 模块划分

```
methratio/
├── Cargo.toml
└── src/
    ├── main.rs          # CLI 入口 + 管道编排（~150 行）
    ├── input.rs         # SAM/BAM/BSP 输入解析（~200 行）
    ├── counter.rs       # 甲基化计数核心逻辑（~400 行）
    ├── output.rs        # TXT + WIG 输出（~200 行）
    ├── cpg_index.rs     # 预计算 CpG/CHG/CHH 位点索引（~150 行）
    └── snp.rs           # CT_SNP 处理（~100 行）
```

### 4.2 核心数据结构

```rust
/// 统一的比对记录抽象（屏蔽 SAM/BAM/BSP 差异）
struct AlignmentRecord {
    qname:      String,
    flag:       u16,
    chrom:      String,
    pos:        u32,         // 0-based
    cigar:      String,
    tlen:       i32,
    seq:        Vec<u8>,
    strand:     (char, char), // (ref_chain, read_chain) from ZS tag
    is_unique:  bool,
    is_paired:  bool,
    is_duplicate: bool,
    hit_type:   Option<String>, // BSP only: UM/MA/NM/QC
}

/// 每条染色体的甲基化计数（稀疏 HashMap）
struct ChromosomeCounts {
    meth:   HashMap<u32, u16>,  // pos -> 甲基化计数
    depth:  HashMap<u32, u16>,  // pos -> 覆盖深度
    meth1:  HashMap<u32, u16>,  // CT_SNP 反向链甲基化（可选）
    depth1: HashMap<u32, u16>,  // CT_SNP 反向链深度（可选）
}
```

### 4.3 内存优化策略

**原版 Python（密集数组）**:
```
人类基因组 ~3Gb × 4 数组 × 2 bytes = ~24GB
```

**Rust 版（稀疏 HashMap）**:
```
仅存储有读段覆盖的 C/G 位置
典型 WGBS ~5-10% 覆盖率 → ~150-300MB
HashMap 开销（~50%）→ 总计 ~300-450MB < 1GB ✅
```

### 4.4 数据流

```
输入 (SAM/BAM/BSP)
    │
    ▼
input.rs ── 解析为 AlignmentRecord
    │
    ▼
counter.rs ── 甲基化计数
    │  1. 根据 strand 确定查找目标（+链找C，-链找G）
    │  2. 遍历参考序列中对应位置的 C/G 碱基
    │  3. 比较读段碱基：C/G=甲基化，T/A=未甲基化
    │  4. 更新 HashMap 计数
    │
    ▼
snp.rs ── CT_SNP 处理（可选）
    │
    ▼
counter.rs ── combine CpG 双链合并（可选）
    │
    ▼
output.rs ── TXT/WIG 输出
```

## 5. 核心算法

### 5.1 比对记录解析（get_alignment）

与原版 `methratio.py` 第 44-91 行逻辑一一对应：

**SAM 格式解析**（第 46-68 行）:
- 跳过 `@` 开头的 header 行
- FLAG 字段：检查是否包含字符 `'u'`（未比对）→ 跳过
- `--unique`：检查 FLAG 是否包含字符 `'s'`（二次比对）→ 跳过
- `--pair`：检查 FLAG 是否包含字符 `'P'`（配对）→ 不满足则跳过
- 提取：`col[2]`=染色体, `col[3]`=位置(转0-based), `col[5]`=CIGAR, `col[8]`=insert size, `col[9]`=序列
- ZS tag：`line.find('ZS:Z:')` → `strand = line[index+5:index+7]`
- CIGAR gap 处理（第 58-68 行）：
  - `I`（insertion）：从序列中删除对应碱基 `seq = seq[:gap_pos] + seq[gap_pos+gap_size:]`
  - `D`（deletion）：在序列中插入 `-` 占位符 `seq = seq[:gap_pos] + '-' * gap_size + seq[gap_pos:]`
- paired overlap（仅 SAM）：`if insert > 0: seq = seq[:col[7]-1-pos]`

**BSP 格式解析**（第 69-80 行）:
- `col[3][:2]` 为 hit type：NM/QC → 跳过
- `--unique`：`col[3][:2] != 'UM'` → 跳过
- `--pair`：`col[7] == '0'` → 跳过（insert size 为 0 视为未配对）
- 提取：`col[1]`=序列, `col[4]`=染色体, `col[5]`=位置(转0-based), `col[6]`=strand, `col[7]`=insert size
- Gap 处理（第 76-80 行）：
  - `col[9]` 含 `:` 时解析 gap：`#mm:#gap_size:gap_pos`
  - `gap_size < 0`（读段插入）: `seq = seq[:gap_pos] + seq[gap_pos-gap_size:]`
  - `gap_size > 0`（参考缺失）: `seq = seq[:gap_pos] + '-' * gap_size + seq[gap_pos:]`
- **无 paired overlap 处理**（BSP 不含 mate 信息）

**公共处理**（第 81-91 行）:
- 边界检查：`pos + len(seq) >= len(ref[cr])` → 跳过
- 去重：基于 `(frag_end, direction)` 标记
  - `strand == '+-' or '-+'`: `frag_end = pos + len(seq)`, `direction = 2`
  - `strand == '++' or '--'`: `frag_end = pos`, `direction = 1`
  - 如果 `coverage[cr][frag_end] & direction` 已设置 → 跳过（重复）
- trim fillin：
  - `'+-' or '-+'`: `seq = seq[:-trim_fillin]`
  - `'++' or '--'`: `seq = seq[trim_fillin:]`, `pos += trim_fillin`

### 5.2 甲基化计数

与原版 `methratio.py` 第 127-164 行逻辑一一对应：

```python
BS_conversion = {'+': ('C','T','G','A'), '-': ('G','A','C','T')}
# (match_base, convert_base, methyl_base, rc_match_base)
```

对每条比对记录：
1. 根据 `strand[0]` 确定查找目标：
   - `+` 链（`++` 或 `+-`）：在参考序列中找 `C`，读段中对应位置是 `T`=未甲基化，是 `C`=甲基化
   - `-` 链（`-+` 或 `--`）：在参考序列中找 `G`，读段中对应位置是 `A`=未甲基化，是 `G`=甲基化
2. 使用 `refseq.find(match, pos, pos2)` 逐个查找 C/G 碱基位置
3. 对每个位置比较 `seq[index-pos]` 与 `convert`/`match`
4. **深度计数规则**：无论读段碱基是什么，只要参考位置有 C/G 就计数 depth
   - `seq[index-pos] == convert`（T/A）: `depth += 1`（未甲基化）
   - `seq[index-pos] == match`（C/G）: `depth += 1, meth += 1`（甲基化）
5. **溢出保护**：`if depth >= 65535: depth = 65535`（u16 上限）

**CT_SNP 处理**（第 153-164 行，可选）:
- 反向检查：`+` 链同时检查参考上的 `G`（预期 `A`），`-` 链同时检查参考上的 `C`（预期 `T`）
- 用于检测和校正 CT/GA SNP 导致的假阳性甲基化调用

### 5.3 上下文判定

```
参考碱基为 C（正链）：
  ref[i+1..=i+2] == "CG"  → context = CG
  ref[i+2] == 'G'          → context = CHG
  其他                      → context = CHH

参考碱基为 G（负链）：
  ref[i-2..=i-1] == "CG"  → context = CG  (即 ref[i-1]=='C')
  ref[i-2] == 'C'          → context = CHG
  其他                      → context = CHH
```

### 5.4 Wilson 置信区间

与原版 `methratio.py` 第 248-251 行逻辑完全一致：

```rust
fn wilson_ci(meth: u16, depth: u16) -> (f64, f64) {
    if depth == 0 { return (0.0, 0.0); }
    let z95 = 1.96;
    let z95sq = z95 * z95;  // 3.8416
    let ratio = meth as f64 / depth as f64;
    let d = depth as f64;
    let pmid = ratio + z95sq / (2.0 * d);
    let sd = z95 * ((ratio * (1.0 - ratio) / d + z95sq / (4.0 * d * d)).sqrt());
    let denom = 1.0 + z95sq / d;
    ((pmid - sd) / denom, (pmid + sd) / denom)
}
```

**注意**：原版 Python 中 `ratio = min(m, d) / d`（第 237 行），即 ratio 上限为 1.0。

### 5.5 CT_SNP 处理

```
模式 0（关闭）：不做任何 CT_SNP 处理
模式 1（correct）：按比例校正深度 d = dd * m1/d1
模式 2（skip）：跳过存在 CT_SNP 的位点
```

### 5.6 Combine CpG

将 CpG 二核苷酸的上下游两个 C/G 位置的计数合并到第一个位置：
```
对每个 CG 位点 pos:
  depth[pos] += depth[pos+1]
  meth[pos] += meth[pos+1]
  depth[pos+1] = 0
  meth[pos+1] = 0
```

## 6. 依赖

### 6.1 新增依赖

| 依赖 | 版本 | 用途 |
|------|------|------|
| `statrs` | 0.18 | Wilson 置信区间（备选：手写公式） |

### 6.2 复用 workspace 依赖

| 依赖 | 用途 |
|------|------|
| `clap` | CLI 参数解析 |
| `anyhow` | 错误处理 |
| `noodles` | SAM/BAM 原生解析 |
| `needletail` | FASTA 参考基因组解析 |
| `rayon` | 按染色体并行处理 |
| `log` + `env_logger` | 日志 |
| `indicatif` | 进度条 |

## 7. 端到端测试计划

### 7.1 测试数据

| 数据集 | 用途 | 预期结果 |
|--------|------|----------|
| `ex1_small/` | 单元测试 | 基本功能验证 |
| `lambda_wgbs/` | 端到端对比 | Rust vs Python >= 99.9% 一致 |
| `rrbs_random_v2/` | RRBS 场景 | 验证 RRBS 数据兼容性 |

### 7.2 测试流程

```
步骤 1: 生成比对结果
  bsmap align -a R1.fq -b R2.fq -d ref.fa -o align.bam

步骤 2: Rust methratio
  methratio -d ref.fa -o rust_out.txt align.bam

步骤 3: Python methratio.py
  python methratio.py -d ref.fa -o python_out.txt align.sam

步骤 4: 对比
  diff rust_out.txt python_out.txt
  # 预期：>= 99.9% 行一致（浮点精度差异除外）
```

### 7.3 验证标准

| 标准 | 方法 | 通过条件 |
|------|------|----------|
| 输出一致性 | diff Rust vs Python | >= 99.9% 行一致 |
| 内存 | `/usr/bin/time -v` | 人类基因组 < 1GB |
| 输入格式 | SAM/BAM/BSP 分别测试 | 全部通过 |
| 管道模式 | `bsmap ... \| methratio ...` | 正常工作 |
| 边界条件 | 空输入、无覆盖位点 | 不崩溃 |

## 8. 实施步骤

### Step 1: 项目脚手架
- 创建 `methratio/` 目录和 `Cargo.toml`
- 启用 workspace member
- 添加 `statrs` 依赖

### Step 2: 输入解析（input.rs）
- 实现 `AlignmentRecord` 结构体
- 实现 SAM 解析（noodles）
- 实现 BAM 解析（noodles）
- 实现 BSP 解析（自定义）
- 实现 STDIN 自动检测

### Step 3: 参考基因组加载
- 使用 needletail 解析 FASTA
- 按染色体存储为 `HashMap<String, Vec<u8>>`

### Step 4: 甲基化计数（counter.rs）
- 实现核心计数逻辑
- 实现 CIGAR 处理（insertion/deletion）
- 实现去重、unique/paired 过滤
- 实现 trim-fillin

### Step 5: CT_SNP 处理（snp.rs）
- 实现 CT_SNP 检测
- 实现 correct/skip 模式

### Step 6: Combine CpG
- 实现 CpG 双链合并

### Step 7: 输出（output.rs）
- 实现 TXT 输出
- 实现 WIG 输出
- 实现 Wilson 置信区间

### Step 8: CLI（main.rs）
- 实现 clap CLI
- 实现管道编排
- 实现进度条

### Step 9: 端到端测试
- Lambda WGBS 数据集对比
- 内存测试
- 管道模式测试

### Step 10: 文档与提交
- 更新 README
- 提交 GitHub
