use std::collections::HashMap;
use std::fs::File;
use std::io;

use clap::Parser;
use needletail::parse_fastx_file;

use methratio::{counter, input, output, Config};

#[derive(Parser, Debug)]
#[command(name = "methratio", version, about = "Methylation ratio calculator for BS-seq data")]
struct Cli {
    /// Reference genome FASTA file (required)
    #[arg(short, long)]
    ref_file: String,

    /// Output file (default: STDOUT)
    #[arg(short, long)]
    out: Option<String>,

    /// Save a copy of input alignment in BAM format
    #[arg(short = 'O', long)]
    alignment_copy: Option<String>,

    /// Output WIG file
    #[arg(short, long)]
    wig: Option<String>,

    /// WIG bin size
    #[arg(short = 'b', long, default_value = "25")]
    wig_bin: usize,

    /// Process only specified chromosomes (comma-separated)
    #[arg(short, long)]
    chr: Option<String>,

    /// Path to samtools
    #[arg(short = 's', long)]
    sam_path: Option<String>,

    /// Process only unique mappings
    #[arg(long)]
    unique: bool,

    /// Process only properly paired mappings
    #[arg(long)]
    pair: bool,

    /// Remove duplicated reads
    #[arg(long)]
    remove_duplicate: bool,

    /// Trim N end-repairing fill-in nucleotides
    #[arg(short = 't', long, default_value = "0")]
    trim_fillin: usize,

    /// Combine CpG methylation on both strands
    #[arg(long)]
    combine_cpg: bool,

    /// Minimum coverage depth
    #[arg(short = 'm', long, default_value = "1")]
    min_depth: usize,

    /// Don't print header line
    #[arg(short = 'n', long)]
    no_header: bool,

    /// CT_SNP handling: no-action, correct, skip
    #[arg(short = 'i', long, default_value = "correct")]
    ct_snp: String,

    /// Methylation context filter: CG, CHG, CHH (comma-separated)
    #[arg(short = 'x', long)]
    context: Option<String>,

    /// Don't print progress
    #[arg(long)]
    quiet: bool,

    /// Input files (SAM/BAM/BSP)
    #[arg(default_value = "-")]
    input: Vec<String>,
}

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let cli = Cli::parse();

    // 解析 CT_SNP 模式
    let ct_snp_val = match cli.ct_snp.to_lowercase().as_str() {
        "no-action" => 0,
        "correct" => 1,
        "skip" => 2,
        _ => anyhow::bail!("Invalid -i value, select \"no-action\", \"correct\" or \"skip\""),
    };

    // 解析染色体列表
    let chroms: Vec<String> = cli.chr.map(|s| s.split(',').map(String::from).collect()).unwrap_or_default();

    // 解析上下文列表
    let context: Vec<String> = cli.context.map(|s| s.split(',').map(String::from).collect()).unwrap_or_default();

    let config = Config {
        unique: cli.unique,
        pair: cli.pair,
        remove_duplicate: cli.remove_duplicate,
        trim_fillin: cli.trim_fillin,
        combine_cpg: cli.combine_cpg,
        min_depth: cli.min_depth,
        no_header: cli.no_header,
        ct_snp: ct_snp_val,
        context,
        chroms: chroms.clone(),
        quiet: cli.quiet,
        wig_bin: cli.wig_bin,
    };

    if !config.quiet {
        eprintln!("[methratio] loading reference file: {} ...", cli.ref_file);
    }

    // 加载参考基因组 (methratio.py 第 103-114 行)
    let mut ref_seqs: HashMap<String, Vec<u8>> = HashMap::new();
    let mut reader = parse_fastx_file(&cli.ref_file)?;
    while let Some(record) = reader.next() {
        let rec = record?;
        let chrom = String::from_utf8_lossy(rec.id()).to_string();
        // 只取第一个空白前的部分作为染色体名
        let chrom_name = chrom.split_whitespace().next().unwrap_or(&chrom).to_string();
        if !config.chroms.is_empty() && !config.chroms.contains(&chrom_name) {
            continue;
        }
        let seq: Vec<u8> = rec.seq().to_vec();
        ref_seqs.insert(chrom_name, seq);
    }

    if !config.quiet {
        eprintln!("[methratio] loaded {} chromosomes", ref_seqs.len());
    }

    // 确定输入文件列表
    let input_files: Vec<String> = if cli.input.len() == 1 && cli.input[0] == "-" {
        vec![]
    } else {
        cli.input.clone()
    };

    // 读取比对记录（传入 ref_seqs）
    let records = if input_files.is_empty() {
        // 从 STDIN 读取
        input::AlignmentReader::from_stdin(config.clone(), ref_seqs.clone())?
    } else {
        input::AlignmentReader::from_files(&input_files, config.clone(), ref_seqs.clone())?
    };

    // 甲基化计数
    if !config.quiet {
        eprintln!("[methratio] counting methylation ...");
    }

    let counts = counter::count_methylation(records, &ref_seqs, &config);

    if !config.quiet {
        eprintln!("[methratio] writing output ...");
    }

    // TXT 输出
    if let Some(out_path) = &cli.out {
        let mut fout = File::create(out_path)?;
        output::write_txt(&counts, &ref_seqs, &config, &mut fout)?;
    } else {
        let mut stdout = io::stdout().lock();
        output::write_txt(&counts, &ref_seqs, &config, &mut stdout)?;
    }

    // WIG 输出
    if let Some(wig_path) = &cli.wig {
        output::write_wig(&counts, &ref_seqs, &config, wig_path)?;
    }

    Ok(())
}
