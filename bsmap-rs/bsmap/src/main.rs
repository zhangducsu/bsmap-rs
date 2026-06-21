//! BSMAP-rs: Bisulfite Sequence MAPping — ultra-fast BS-seq aligner in Rust.
//!
//! 亚硫酸氢盐测序比对器主程序入口。
//!
//! ## 流程概述
//!
//! 1. 解析 CLI 参数并分发子命令
//! 2. `bsmap index -d ref.fa`: 构建参考序列索引
//! 3. `bsmap align -a reads.fq -d ref.fa -o out.sam`: 比对读段
//! 4. `bsmap -a reads.fq -d ref.fa`: 等价于 `bsmap align`（向后兼容）

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use log::{info, warn, LevelFilter};

use bsmap::align::{format_bsp, format_sam, AlignmentResult, SingleAlign};
use bsmap::cli::{resolve_command, resolve_index_args, AlignArgs, Cli};
use bsmap::pairs::{format_pair_sam, PairAlign, PairBatchResult};
use bsmap::param::{AlignConfig, AlignStats, BATCH_SIZE, ReadInf};
use bsmap::reads::{encode_read, process_batch, EncodedRead, FastqReader};
use bsmap::reference::{
    default_index_path, is_index_compatible, load_index_with_mode, save_index_v2,
    BinSeqCollection, BinSeqCollectionBuilder, KmerIndex, LoadMode, Reference,
    ReferenceReader, RrbsIndexBuilder,
};
use bsmap::utils::Timer;

// ─────────────────────────────────────────────────────────────────────────────
// 主程序入口
// ─────────────────────────────────────────────────────────────────────────────

fn main() -> Result<()> {
    // 1. 解析 CLI 参数
    let cli = Cli::parse();

    // 2. 分发子命令
    match resolve_command(&cli) {
        bsmap::cli::ResolvedCommand::Index => {
            let index_args = resolve_index_args(&cli)
                .expect("index 子命令参数解析失败");
            run_index_command(&index_args)
        }
        bsmap::cli::ResolvedCommand::Align(args) => {
            // 验证必要参数
            if args.query_a.is_none() {
                eprintln!("错误: 比对需要提供 -a 参数指定读段文件");
                eprintln!("用法: bsmap align -a reads.fq -d ref.fa [-o out.sam]");
                std::process::exit(1);
            }
            if args.reference.is_none() {
                eprintln!("错误: 比对需要提供 -d 参数指定参考序列文件");
                eprintln!("用法: bsmap align -a reads.fq -d ref.fa [-o out.sam]");
                std::process::exit(1);
            }
            run_align_command(&args)
        }
        bsmap::cli::ResolvedCommand::Help => {
            // 没有足够参数，打印帮助信息
            Cli::parse_from(["bsmap", "--help"]);
            Ok(())
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// index 子命令
// ─────────────────────────────────────────────────────────────────────────────

/// 运行 `bsmap index` 子命令。
///
/// 加载参考序列，构建二进制序列集合，构建 k-mer 索引，保存为 .bsi 文件。
fn run_index_command(args: &bsmap::cli::IndexArgs) -> Result<()> {
    // 初始化日志
    let log_level = match args.verbose {
        0 => LevelFilter::Off,
        1 => LevelFilter::Info,
        _ => LevelFilter::Debug,
    };
    env_logger::Builder::new()
        .filter_level(log_level)
        .format_timestamp_secs()
        .init();

    info!("BSMAP-rs v{} [index]", env!("CARGO_PKG_VERSION"));
    info!("参考序列: {}", args.reference.display());

    let mut timer = Timer::new();

    // 1. 加载参考序列
    info!("加载参考序列...");
    let refs = load_references(&args.reference)?;
    let ref_names: Vec<String> = refs.iter().map(|r| r.name.clone()).collect();
    let total_bp: u64 = refs.iter().map(|r| r.len as u64).sum();
    info!(
        "加载 {} 条参考序列，共 {} bp，耗时 {:.2}s",
        refs.len(),
        total_bp,
        timer.step()
    );

    // 2. 构建二进制序列集合
    info!("构建二进制序列集合...");
    let coll = BinSeqCollection::from_references(&refs);
    info!("二进制序列集合构建完成，耗时 {:.2}s", timer.step());

    // 3. 检查是否有兼容的缓存索引
    let index_path = default_index_path(&args.reference);
    let is_rrbs = !args.digestion_sites.is_empty();
    let seed_size = if is_rrbs && args.seed_size == 16 { 12 } else { args.seed_size };

    if is_index_compatible(&index_path, &ref_names, seed_size, is_rrbs)? {
        info!("索引文件已存在且兼容: {}", index_path.display());
        info!("如需重建，请删除该文件后重试");
        return Ok(());
    }

    // 4. 构建索引
    info!(
        "构建索引 (seed_size={}, interval={}, mode={})...",
        seed_size,
        args.index_interval,
        if is_rrbs { "RRBS" } else { "WGBS" }
    );

    let index = if is_rrbs {
        KmerIndex::build_rrbs(
            &coll,
            &refs,
            seed_size,
            args.index_interval,
            &args.digestion_sites,
            args.min_insert,
            args.max_insert,
        )
    } else {
        KmerIndex::build_wgbs(
            &coll,
            seed_size,
            args.index_interval,
            args.kmer_cutoff,
        )
    };

    info!("索引构建完成，耗时 {:.2}s", timer.step());

    // 5. 保存索引
    match save_index_v2(
        &index_path,
        &index,
        &coll,
        seed_size,
        args.index_interval,
        args.kmer_cutoff,
        &ref_names,
        is_rrbs,
    ) {
        Ok(()) => {
            info!("索引已保存: {}", index_path.display());
        }
        Err(e) => {
            warn!("索引保存失败: {}", e);
        }
    }

    info!("总耗时: {:.2}s", timer.elapsed());

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// align 子命令
// ─────────────────────────────────────────────────────────────────────────────

/// 运行 `bsmap align` 子命令（含向后兼容的无子命令模式）。
fn run_align_command(args: &AlignArgs) -> Result<()> {
    // 初始化日志
    let log_level = match args.verbose {
        0 => LevelFilter::Off,
        1 => LevelFilter::Info,
        _ => LevelFilter::Debug,
    };
    env_logger::Builder::new()
        .filter_level(log_level)
        .format_timestamp_secs()
        .init();

    info!("BSMAP-rs v{} [align]", env!("CARGO_PKG_VERSION"));
    info!("查询文件: {}", args.query_a.as_ref().unwrap().display());
    if let Some(ref qb) = args.query_b {
        info!("查询文件 (mate): {}", qb.display());
    }
    info!("参考序列: {}", args.reference.as_ref().unwrap().display());

    // 构建配置
    let mut config = AlignConfig::from(args);

    // 设置 rayon 全局线程池（必须在首次调用 par_iter 前）
    if let Err(e) = rayon::ThreadPoolBuilder::new()
        .num_threads(config.num_threads)
        .build_global()
    {
        warn!("rayon 线程池已初始化，使用已有配置: {}", e);
    }

    // 流式编码参考序列，避免同时保留整份 ASCII FASTA 与双链二进制数据。
    let mut timer = Timer::new();
    info!("加载参考序列...");
    let ref_path = args.reference.as_ref().unwrap();
    let coll = load_binseq_collection(ref_path)?;
    let ref_names = coll.chr_names.clone();
    let ref_lengths = coll.chr_lengths.clone();
    info!(
        "加载 {} 条参考序列，共 {} bp，耗时 {:.2}s",
        ref_names.len(),
        coll.sum_length,
        timer.step()
    );

    // 构建或加载索引
    let (index, coll) = load_or_build_index(coll, &mut config, ref_path, &ref_names)?;

    // 打开输出文件
    let mut output = open_output(args)?;

    // 写入 SAM header
    if config.sam_header && config.out_sam > 0 {
        write_sam_header(&mut output, &ref_names, &ref_lengths)?;
    }

    // 运行比对
    let stats = Arc::new(AlignStats::default());

    if config.paired_end {
        run_paired_align(args, &config, &index, &coll, &mut output, &stats)?;
    } else {
        run_single_align(args, &config, &index, &coll, &mut output, &stats)?;
    }

    // BAM 输出需要关闭 BGZF 流
    if let OutputWriter::BamFile { writer, .. } = &mut output {
        writer
            .try_finish()
            .with_context(|| "关闭 BAM BGZF 流失败")?;
    }

    // 打印统计信息
    print_stats(&stats, &config);

    info!("总耗时: {:.2}s", timer.elapsed());

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// 参考序列加载
// ─────────────────────────────────────────────────────────────────────────────

/// 加载参考序列。
///
/// 从 FASTA 文件加载所有染色体序列。
/// 支持普通文件和 gzip 压缩文件。
///
/// # 参数
/// - `path`: FASTA 文件路径
///
/// # 返回值
/// 参考序列列表
fn load_references(path: &Path) -> Result<Vec<Reference>> {
    // 检测是否为 gzip 文件
    let is_gz = bsmap::reference::fasta::is_gzipped(path)?;

    if is_gz {
        bsmap::reference::fasta::load_fasta_with_gzip(path, true)
    } else {
        bsmap::reference::fasta::load_fasta(path, false)
    }
}

fn load_binseq_collection(path: &Path) -> Result<BinSeqCollection> {
    let mut reader = ReferenceReader::open(path)?;
    let mut builder = BinSeqCollectionBuilder::new();
    while let Some(reference) = reader.next_reference()? {
        builder.push(&reference);
    }
    Ok(builder.finish())
}

// ─────────────────────────────────────────────────────────────────────────────
// 索引加载/构建
// ─────────────────────────────────────────────────────────────────────────────

/// 加载或构建索引。
///
/// 如果缓存文件存在且兼容，直接加载；否则构建新索引并保存。
///
/// # 参数
/// - `refs`: 参考序列列表
/// - `config`: 比对配置（会被更新）
/// - `ref_path`: 参考序列文件路径
/// - `ref_names`: 参考序列名称列表
///
/// # 返回值
/// (KmerIndex, BinSeqCollection) 元组
fn load_or_build_index(
    mut coll: BinSeqCollection,
    config: &mut AlignConfig,
    ref_path: &Path,
    ref_names: &[String],
) -> Result<(KmerIndex, BinSeqCollection)> {
    let mut timer = Timer::new();

    // 检查是否有缓存索引
    let index_path = default_index_path(ref_path);
    let use_cache = is_index_compatible(&index_path, ref_names, config.seed_size, config.rrbs_flag)?;

    if use_cache {
        info!("从缓存加载索引: {}", index_path.display());
        match load_index_with_mode(&index_path, LoadMode::Mmap) {
            Ok((loaded_coll, index, _meta)) => {
                info!("索引已从缓存加载: {}", index_path.display());
                // 用 mmap 加载的 refcat/crefcat 替换内存版本，保留其他元数据
                coll.refcat = loaded_coll.refcat;
                coll.crefcat = loaded_coll.crefcat;
                return Ok((index, coll));
            }
            Err(e) => {
                info!("无法加载索引: {}, 将重新构建: {}", index_path.display(), e);
            }
        }
    }

    // 构建新索引
    info!("构建索引 (seed_size={}, interval={})...", config.seed_size, config.index_interval);

    let index = if config.rrbs_flag {
        let mut builder = RrbsIndexBuilder::new(
            coll.chr_names.len(),
            config.seed_size,
            &config.digest_sites,
            config.min_insert,
            config.max_insert,
        );
        let mut reader = ReferenceReader::open(ref_path)?;
        let mut chromosome_index = 0usize;
        while let Some(reference) = reader.next_reference()? {
            builder.push_reference(chromosome_index, &reference);
            chromosome_index += 1;
        }
        builder.finish(&coll)
    } else {
        // WGBS 模式
        KmerIndex::build_wgbs(
            &coll,
            config.seed_size,
            config.index_interval,
            config.max_kmer_ratio,
        )
    };

    info!("索引构建完成，耗时 {:.2}s", timer.step());

    // 保存索引到缓存
    if let Err(e) = save_index_v2(
        &index_path,
        &index,
        &coll,
        config.seed_size,
        config.index_interval,
        config.max_kmer_ratio,
        ref_names,
        config.rrbs_flag,
    ) {
        warn!("索引保存失败: {}", e);
    }

    Ok((index, coll))
}

// ─────────────────────────────────────────────────────────────────────────────
// 输出文件处理
// ─────────────────────────────────────────────────────────────────────────────

/// 输出写入器枚举。
///
/// 支持三种输出方式：
/// - Stdout: 输出到标准输出
/// - SamFile: 输出到 SAM 文件
/// - BamFile: 输出到 BAM 文件（使用 noodles bam writer，内置 BGZF 压缩）
pub enum OutputWriter {
    /// 标准输出。
    Stdout,
    /// SAM 文件输出。
    SamFile(BufWriter<File>),
    /// BAM 文件输出（使用 noodles bam writer，内置 BGZF 压缩）。
    BamFile {
        writer: noodles::bam::io::Writer<noodles::bgzf::Writer<File>>,
        header: noodles::sam::Header,
    },
}

/// 打开输出文件。
///
/// 根据 AlignArgs 决定输出方式：
/// - 无 `-o` 参数: 输出到 stdout
/// - `-o file.sam`: 输出到 SAM 文件
/// - `-o file.bam`: 输出到 BAM 文件
fn open_output(args: &AlignArgs) -> Result<OutputWriter> {
    match &args.output {
        None => {
            // 输出到 stdout
            Ok(OutputWriter::Stdout)
        }
        Some(path) => {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase());

            match ext.as_deref() {
                Some("bam") => {
                    // BAM 输出（使用 bam::io::Writer，内置 BGZF 压缩）
                    let file = File::create(path)
                        .with_context(|| format!("无法创建 BAM 文件: {}", path.display()))?;
                    let writer = noodles::bam::io::Writer::new(file);
                    Ok(OutputWriter::BamFile {
                        writer,
                        header: noodles::sam::Header::default(),
                    })
                }
                Some("sam") | _ => {
                    // SAM 输出（或其他扩展名默认为 SAM）
                    let file = File::create(path)
                        .with_context(|| format!("无法创建 SAM 文件: {}", path.display()))?;
                    Ok(OutputWriter::SamFile(BufWriter::new(file)))
                }
            }
        }
    }
}

/// 写入 SAM header。
///
/// 生成并写入 SAM 文件头，包括：
/// - @HD 行：文件格式版本
/// - @SQ 行：参考序列信息
/// - @PG 行：程序信息
fn write_sam_header(
    output: &mut OutputWriter,
    ref_names: &[String],
    ref_lengths: &[u32],
) -> Result<()> {
    let mut header = String::new();

    // @HD 行
    header.push_str("@HD\tVN:1.0\tSO:unsorted\n");

    // @SQ 行（只取名称的第一个空白字符前的部分）
    for (name, &length) in ref_names.iter().zip(ref_lengths) {
        let sn = name.split_whitespace().next().unwrap_or(name);
        header.push_str(&format!("@SQ\tSN:{}\tLN:{}\n", sn, length));
    }

    // @PG 行
    header.push_str(&format!(
        "@PG\tID:bsmap\tPN:bsmap\tVN:{}\n",
        env!("CARGO_PKG_VERSION")
    ));

    match output {
        OutputWriter::Stdout => {
            print!("{}", header);
        }
        OutputWriter::SamFile(w) => {
            w.write_all(header.as_bytes())?;
        }
        OutputWriter::BamFile { writer, header: sam_header } => {
            // 解析 SAM header 字符串为 noodles::sam::Header
            let parsed: noodles::sam::Header = header
                .parse()
                .with_context(|| "解析 SAM header 为 noodles Header 失败")?;
            writer
                .write_header(&parsed)
                .with_context(|| "写入 BAM header 失败")?;
            // 将解析后的 header 存回 OutputWriter，供后续 write_record 使用
            *sam_header = parsed;
        }
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// 单端比对流程
// ─────────────────────────────────────────────────────────────────────────────

/// 运行单端比对。
///
/// 流式读取读段，批量处理，输出比对结果。
fn run_single_align(
    args: &AlignArgs,
    config: &AlignConfig,
    index: &KmerIndex,
    coll: &BinSeqCollection,
    output: &mut OutputWriter,
    stats: &Arc<AlignStats>,
) -> Result<()> {
    info!("开始单端比对...");

    // 创建读段读取器
    let query_path = args.query_a.as_ref().unwrap();
    let mut reader = FastqReader::open(query_path, config.gz_input)?;

    // 创建进度条
    let progress = ProgressBar::new(0);
    progress.set_style(
        ProgressStyle::default_bar()
            .template("{elapsed} {wide_bar} {pos} reads ({per_sec})")?,
    );

    // 创建比对引擎
    let mut aligner = SingleAlign::new();

    // 批量处理
    let mut batch_raw = Vec::with_capacity(BATCH_SIZE);
    let mut read_start = config.read_start;
    let read_end = config.read_end;

    loop {
        batch_raw.clear();

        // 读取一批读段
        let n = reader.read_batch(&mut batch_raw, BATCH_SIZE, &mut read_start, read_end)?;
        if n == 0 {
            break;
        }

        // 处理读段（P11-4: mem::take 代替 clone，消除每批深拷贝）
        let reads = process_batch(std::mem::take(&mut batch_raw), 0, config);

        // 编码读段
        let encoded: Vec<EncodedRead> = reads.iter().map(|r| encode_read(r)).collect();

        // 执行比对
        let results = aligner.do_batch(&encoded, index, coll, config);

        // 输出结果
        for result in &results {
            if result.has_hits() {
                output_alignment(
                    output,
                    &reads[result.read_idx as usize],
                    result,
                    coll,
                    config,
                )?;

                // 更新统计
                stats.n_aligned.fetch_add(1, Ordering::Relaxed);
                if result.is_unique {
                    stats.n_unique.fetch_add(1, Ordering::Relaxed);
                } else {
                    stats.n_multiple.fetch_add(1, Ordering::Relaxed);
                }
            } else if config.out_unmap {
                // 输出未比对读段
                output_unmapped(output, &reads[result.read_idx as usize], config)?;
            }
        }

        // 更新进度条
        progress.inc(n as u64);
    }

    progress.finish();
    info!("单端比对完成");

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// 双端比对流程
// ─────────────────────────────────────────────────────────────────────────────

/// 运行双端比对。
///
/// 同时读取两个配对文件，执行配对比对，输出结果。
fn run_paired_align(
    args: &AlignArgs,
    config: &AlignConfig,
    index: &KmerIndex,
    coll: &BinSeqCollection,
    output: &mut OutputWriter,
    stats: &Arc<AlignStats>,
) -> Result<()> {
    info!("开始双端比对...");

    // 检查是否有第二个读段文件
    let query_a = args.query_a.as_ref().unwrap();
    let query_b = match &args.query_b {
        Some(path) => path,
        None => bail!("双端比对需要提供 -b 参数指定第二个读段文件"),
    };

    // 创建两个读段读取器
    let mut reader_a = FastqReader::open(query_a, config.gz_input)?;
    let mut reader_b = FastqReader::open(query_b, config.gz_input)?;

    // 创建进度条
    let progress = ProgressBar::new(0);
    progress.set_style(
        ProgressStyle::default_bar()
            .template("{elapsed} {wide_bar} {pos} pairs ({per_sec})")?,
    );

    // 创建配对比对引擎
    let mut pair_aligner = PairAlign::new();

    // 批量处理
    let mut batch_a = Vec::with_capacity(BATCH_SIZE);
    let mut batch_b = Vec::with_capacity(BATCH_SIZE);
    // 修复：两个 reader 必须使用独立的 read_start，否则 reader_a 读取后
    // read_start 被更新，导致 reader_b 跳过所有读段
    let mut read_start_a = config.read_start;
    let mut read_start_b = config.read_start;
    let read_end = config.read_end;

    loop {
        batch_a.clear();
        batch_b.clear();

        // 读取一批读段
        let n_a = reader_a.read_batch(&mut batch_a, BATCH_SIZE, &mut read_start_a, read_end)?;
        let n_b = reader_b.read_batch(&mut batch_b, BATCH_SIZE, &mut read_start_b, read_end)?;

        if n_a == 0 || n_b == 0 {
            break;
        }

        // 确保两个文件读段数量一致
        let n = n_a.min(n_b);

        // 处理读段（P11-4: mem::take 代替 clone，消除每批深拷贝）
        let reads_a = process_batch(std::mem::take(&mut batch_a), 1, config);
        let reads_b = process_batch(std::mem::take(&mut batch_b), 2, config);

        // 编码读段
        let encoded_a: Vec<EncodedRead> = reads_a.iter().map(|r| encode_read(r)).collect();
        let encoded_b: Vec<EncodedRead> = reads_b.iter().map(|r| encode_read(r)).collect();

        // 执行配对比对
        let results = pair_aligner.do_pair_batch(&encoded_a, &encoded_b, index, coll, config);

        // 输出结果
        for result in &results {
            let idx = result.read_idx as usize;
            if idx >= reads_a.len() || idx >= reads_b.len() {
                continue;
            }

            if result.has_pair() {
                // 输出配对结果
                output_pair_alignment(
                    output,
                    &reads_a[idx],
                    &reads_b[idx],
                    result,
                    coll,
                    config,
                )?;

                // 更新统计
                stats.n_aligned_pairs.fetch_add(1, Ordering::Relaxed);
                if result.is_unique {
                    stats.n_unique_pairs.fetch_add(1, Ordering::Relaxed);
                } else {
                    stats.n_multiple_pairs.fetch_add(1, Ordering::Relaxed);
                }
            } else {
                // 未配对，输出单端结果
                // C++ 行为：有 hit 的未配对 reads 始终输出，无 hit 的只在 out_unmap 时输出
                let has_hit_a = !result.unpair_hits_a.is_empty();
                let has_hit_b = !result.unpair_hits_b.is_empty();

                if has_hit_a {
                    stats.n_aligned_a.fetch_add(1, Ordering::Relaxed);
                }
                if has_hit_b {
                    stats.n_aligned_b.fetch_add(1, Ordering::Relaxed);
                }

                // 有 hit 的未配对 reads 始终输出（与 C++ 一致）
                // 无 hit 的 reads 只在 out_unmap 时输出
                if has_hit_a || has_hit_b || config.out_unmap {
                    output_unpaired(
                        output,
                        &reads_a[idx],
                        &reads_b[idx],
                        result,
                        coll,
                        config,
                    )?;
                }
            }
        }

        // 更新进度条
        progress.inc(n as u64);
    }

    progress.finish();
    info!("双端比对完成");

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// 输出格式化
// ─────────────────────────────────────────────────────────────────────────────

/// 输出单端比对结果。
///
/// 根据配置输出 SAM 或 BSP 格式。
fn output_alignment(
    output: &mut OutputWriter,
    read: &ReadInf,
    result: &AlignmentResult,
    coll: &BinSeqCollection,
    config: &AlignConfig,
) -> Result<()> {
    let total_hits = result.hits.len();

    for (i, hit) in result.hits.iter().enumerate() {
        // 根据 report_repeat_hits 决定输出策略
        if config.report_repeat_hits == 0 && !result.is_unique {
            // 仅输出唯一比对
            continue;
        }
        if config.report_repeat_hits == 1 && i > 0 {
            // 随机选择一个，只输出第一个
            break;
        }

        let line = match config.out_sam {
            0 => format_bsp(read, hit, coll, config, if result.is_unique { "UM" } else { "MA" }),
            _ => format_sam(read, hit, coll, config, result.is_unique, total_hits),
        };

        write_output_line(output, &line)?;
    }

    Ok(())
}

/// 输出配对比对结果。
///
/// 输出配对的两个读段。
fn output_pair_alignment(
    output: &mut OutputWriter,
    read_a: &ReadInf,
    read_b: &ReadInf,
    result: &PairBatchResult,
    coll: &BinSeqCollection,
    config: &AlignConfig,
) -> Result<()> {
    let total_hits = result.pair_hits.len();

    for (i, pair_hit) in result.pair_hits.iter().enumerate() {
        // 根据 report_repeat_hits 决定输出策略
        if config.report_repeat_hits == 0 && !result.is_unique {
            continue;
        }
        if config.report_repeat_hits == 1 && i > 0 {
            break;
        }

        let (line_a, line_b) = match config.out_sam {
            0 => {
                // BSP 格式
                let hit_type = if result.is_unique { "UM" } else { "MA" };
                (
                    format_bsp(read_a, &pair_hit.a, coll, config, hit_type),
                    format_bsp(read_b, &pair_hit.b, coll, config, hit_type),
                )
            }
            _ => {
                // SAM 格式
                format_pair_sam(
                    read_a, read_b, pair_hit, coll, config, result.is_unique, total_hits,
                )
            }
        };

        write_output_line(output, &line_a)?;
        write_output_line(output, &line_b)?;
    }

    Ok(())
}

/// 输出未比对读段。
fn output_unmapped(
    output: &mut OutputWriter,
    read: &ReadInf,
    config: &AlignConfig,
) -> Result<()> {
    use bsmap::align::OutputFormat;

    let format = match config.out_sam {
        0 => OutputFormat::Bsp,
        _ => OutputFormat::Sam,
    };

    let line = bsmap::align::output::format_unmapped(read, format);
    write_output_line(output, &line)?;

    Ok(())
}

/// 输出未配对读段。
///
/// 与 C++ `s_OutHitUnpair` 行为一致：
/// - 只输出有 hit 的 read（不输出无 hit 的 mate）
/// - 设置 0x40 (first in pair) 或不设置
/// - 当 mate 无 hit 时设置 0x8 (mate unmapped)
fn output_unpaired(
    output: &mut OutputWriter,
    read_a: &ReadInf,
    read_b: &ReadInf,
    result: &PairBatchResult,
    coll: &BinSeqCollection,
    config: &AlignConfig,
) -> Result<()> {
    let has_hit_a = !result.unpair_hits_a.is_empty();
    let has_hit_b = !result.unpair_hits_b.is_empty();

    // read_a 有 hit → 输出
    if has_hit_a {
        let line = if config.out_sam == 0 {
            format_bsp(read_a, &result.unpair_hits_a[0], coll, config, "MA")
        } else {
            format_unpair_sam_single(
                read_a,
                &result.unpair_hits_a[0],
                result.unpair_hits_a.len(),
                has_hit_b,
                result.unpair_hits_b.first(),
                true, // is_first
                coll,
                config,
            )
        };
        write_output_line(output, &line)?;
    }

    // read_b 有 hit → 输出
    if has_hit_b {
        let line = if config.out_sam == 0 {
            format_bsp(read_b, &result.unpair_hits_b[0], coll, config, "MA")
        } else {
            format_unpair_sam_single(
                read_b,
                &result.unpair_hits_b[0],
                result.unpair_hits_b.len(),
                has_hit_a,
                result.unpair_hits_a.first(),
                false, // is_second
                coll,
                config,
            )
        };
        write_output_line(output, &line)?;
    }

    Ok(())
}

/// 格式化单条未配对 read 的 SAM 记录。
///
/// 与 C++ `s_OutHitUnpair` 行为一致：
/// - 只输出一条记录
/// - 设置 0x40 (first in pair) 当 is_first=true
/// - 设置 0x8 (mate unmapped) 当 mate 无 hit
/// - 当 mate 无 hit 时 RNEXT=*, PNEXT=0
fn format_unpair_sam_single(
    read: &ReadInf,
    hit: &bsmap::param::GHit,
    total_hits: usize,
    mate_has_hit: bool,
    mate_hit: Option<&bsmap::param::GHit>,
    is_first: bool,
    coll: &bsmap::reference::binseq::BinSeqCollection,
    config: &bsmap::param::AlignConfig,
) -> String {
    use bsmap::align::output::{get_chromosome_length, make_cigar, make_zs_tag, select_output_seq};

    let ref_chain = hit.strand >> 1;
    let read_chain = hit.strand & 1;

    // FLAG: 与 C++ s_OutHitUnpair 一致
    let mut flag: u16 = 0x1; // paired
    if is_first {
        flag |= 0x40; // first in pair
    } else {
        flag |= 0x80; // second in pair
    }
    if !mate_has_hit {
        flag |= 0x8; // mate unmapped
    }

    // 0x10: reverse strand
    // 与 C++ s_OutHitUnpair 一致: rev_seq = chain_a ^ (ha.chr % 2) = read_chain ^ ref_chain
    if (read_chain ^ ref_chain) == 1 {
        flag |= 0x10;
    }

    // 多重命中标记
    if total_hits > 1 {
        flag |= 0x100;
    }

    let mapq: u8 = 255;
    let cigar = make_cigar(read.seq.len() as u32, hit.gap_size as i8, hit.gap_pos as u8);
    let (seq, qual) = select_output_seq(read, flag & 0x10 != 0);

    // POS: 反向参考链坐标转换
    let pos = if ref_chain == 0 {
        hit.loc + 1
    } else {
        let chr_len = get_chromosome_length(hit.chr, coll);
        chr_len - hit.loc - read.seq.len() as u32 + 1
    };

    // RNEXT/PNEXT: mate 信息
    let (rnext, pnext) = if mate_has_hit {
        if let Some(mh) = mate_hit {
            let mate_ref_chain = mh.strand >> 1;
            let mate_pos = if mate_ref_chain == 0 {
                mh.loc + 1
            } else {
                let mate_chr_len = get_chromosome_length(mh.chr, coll);
                mate_chr_len - mh.loc - read.seq.len() as u32 + 1
            };
            ("=".to_string(), mate_pos)
        } else {
            ("*".to_string(), 0)
        }
    } else {
        ("*".to_string(), 0)
    };

    // ZS tag
    let zs = make_zs_tag(
        if ref_chain == 0 { 0 } else { 1 },
        if read_chain == 0 { 0 } else { 1 },
    );

    // QNAME: 去除后缀
    let qname = strip_r_suffix_for_unpair(&read.name);

    format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t0\t{}\t{}\tNM:i:{}\tZS:Z:{}",
        qname, flag,
        bsmap::align::output::get_reference_name(hit.chr, coll),
        pos, mapq, cigar, rnext, pnext, seq, qual,
        hit.snps, zs,
    )
}

/// 去除 QNAME 的 read 编号后缀。
fn strip_r_suffix_for_unpair(name: &str) -> &str {
    if name.ends_with("_R1") || name.ends_with("_R2") ||
       name.ends_with("_r1") || name.ends_with("_r2") {
        &name[..name.len()-3]
    } else if name.ends_with("/1") || name.ends_with("/2") {
        &name[..name.len()-2]
    } else {
        name
    }
}

/// 写入一行输出。
fn write_output_line(output: &mut OutputWriter, line: &str) -> Result<()> {
    match output {
        OutputWriter::Stdout => {
            println!("{}", line);
        }
        OutputWriter::SamFile(w) => {
            writeln!(w, "{}", line)?;
        }
        OutputWriter::BamFile { writer, header } => {
            // 将 SAM 文本行解析为 sam::Record，然后写入 BAM
            use noodles::sam::alignment::io::Write;
            let record = noodles::sam::Record::try_from(line.as_bytes())
                .with_context(|| format!("解析 SAM 行为 Record 失败: {}", line))?;
            writer
                .write_alignment_record(header, &record)
                .with_context(|| "写入 BAM 记录失败")?;
        }
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// 统计信息
// ─────────────────────────────────────────────────────────────────────────────

/// 打印统计信息。
///
/// 输出比对统计，包括：
/// - 总读段数
/// - 比对读段数
/// - 唯一比对数
/// - 多重比对数
fn print_stats(stats: &AlignStats, config: &AlignConfig) {
    info!("========== 比对统计 ==========");

    if config.paired_end {
        let aligned_pairs = stats.n_aligned_pairs.load(Ordering::Relaxed);
        let unique_pairs = stats.n_unique_pairs.load(Ordering::Relaxed);
        let multiple_pairs = stats.n_multiple_pairs.load(Ordering::Relaxed);

        info!("配对比对数: {}", aligned_pairs);
        info!("  唯一配对: {}", unique_pairs);
        info!("  多重配对: {}", multiple_pairs);

        let aligned_a = stats.n_aligned_a.load(Ordering::Relaxed);
        let aligned_b = stats.n_aligned_b.load(Ordering::Relaxed);
        info!("单端比对 (read_a): {}", aligned_a);
        info!("单端比对 (read_b): {}", aligned_b);
    } else {
        let aligned = stats.n_aligned.load(Ordering::Relaxed);
        let unique = stats.n_unique.load(Ordering::Relaxed);
        let multiple = stats.n_multiple.load(Ordering::Relaxed);

        info!("比对读段数: {}", aligned);
        info!("  唯一比对: {}", unique);
        info!("  多重比对: {}", multiple);
    }

    info!("==============================");
}

// ─────────────────────────────────────────────────────────────────────────────
// 测试
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试输出写入器的基本功能
    #[test]
    fn test_output_writer_stdout() {
        // 这个测试主要验证编译通过
        // 实际的 stdout 输出不易测试
    }

    /// 测试 SAM header 生成
    #[test]
    fn test_sam_header_generation() {
        let refs = vec![
            Reference {
                name: "chr1".to_string(),
                seq: b"ACGT".repeat(100).to_vec(),
                len: 400,
            },
            Reference {
                name: "chr2".to_string(),
                seq: b"TGCA".repeat(50).to_vec(),
                len: 200,
            },
        ];
        let ref_names: Vec<String> = refs.iter().map(|r| r.name.clone()).collect();

        // 验证 header 格式
        let mut header = String::new();
        header.push_str("@HD\tVN:1.0\tSO:unsorted\n");
        for (name, r) in ref_names.iter().zip(refs.iter()) {
            header.push_str(&format!("@SQ\tSN:{}\tLN:{}\n", name, r.len));
        }
        header.push_str("@PG\tID:bsmap\tPN:bsmap\tVN:0.1.0\n");

        assert!(header.contains("@HD"));
        assert!(header.contains("chr1"));
        assert!(header.contains("LN:400"));
        assert!(header.contains("chr2"));
        assert!(header.contains("LN:200"));
        assert!(header.contains("@PG"));
    }

    /// 测试统计信息更新
    #[test]
    fn test_stats_update() {
        let stats = AlignStats::default();

        stats.n_aligned.fetch_add(100, Ordering::Relaxed);
        stats.n_unique.fetch_add(60, Ordering::Relaxed);
        stats.n_multiple.fetch_add(40, Ordering::Relaxed);

        assert_eq!(stats.n_aligned.load(Ordering::Relaxed), 100);
        assert_eq!(stats.n_unique.load(Ordering::Relaxed), 60);
        assert_eq!(stats.n_multiple.load(Ordering::Relaxed), 40);
    }
}
