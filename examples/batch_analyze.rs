//! 批量分析 COMTRADE 录波文件示例：遍历目录，对每个 `.cfg` 对应的录波组
//! 输出站名、通道数、采样点数、文件大小、加载/读取耗时，写入 Tab 分隔日志。
//!
//! 运行：
//! ```bash
//! # 默认遍历 Z:\，日志写 Z:\batch_analyze.log
//! cargo run --example batch_analyze
//! # 指定目录与日志文件
//! cargo run --example batch_analyze -- Z:\2012 out.log
//! ```

use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use comtrade_io::Comtrade;

/// 单个文件的分析结果（失败时仅 status 与 load_ms 有效）
struct Record {
    path: PathBuf,
    station: String,
    channels_total: usize,
    channels_analog: usize,
    channels_status: usize,
    samples: usize,
    size_total: u64,
    size_cfg: u64,
    size_dat: u64,
    load_ms: f64,
    read_ms: f64,
    status: String,
}

fn main() {
    let mut args = std::env::args().skip(1);
    let root = args.next().unwrap_or_else(|| r"Z:\".to_string());
    let log_path = args.next().unwrap_or_else(|| {
        if root.ends_with('\\') || root.ends_with('/') {
            format!("{}batch_analyze.log", root)
        } else {
            format!("{}\\batch_analyze.log", root)
        }
    });

    let root_path = Path::new(&root);
    println!("扫描目录: {}", root_path.display());

    let started = Instant::now();
    let mut cfg_files = Vec::new();
    collect_cfg(root_path, &mut cfg_files);
    cfg_files.sort();
    println!(
        "发现 {} 个 CFG 文件，耗时 {:.1} ms",
        cfg_files.len(),
        started.elapsed().as_secs_f64() * 1000.0
    );

    let mut ok = 0usize;
    let mut failed = 0usize;
    let mut missing_dat = 0usize;

    let file = File::create(&log_path).expect("无法创建日志文件");
    let mut w = BufWriter::new(file);

    // 表头
    writeln!(
        w,
        "序号\t文件路径\t站名\t通道总数\t模拟通道数\t状态通道数\t采样点数\t文件大小(B)\tCFG大小(B)\tDAT大小(B)\t加载耗时(ms)\t读取耗时(ms)\t状态"
    )
    .expect("写表头失败");

    for (idx, cfg) in cfg_files.iter().enumerate() {
        let rec = analyze(cfg);
        match rec.status.as_str() {
            "OK" => ok += 1,
            s if s.starts_with("DAT缺失") => {
                ok += 1;
                missing_dat += 1;
            },
            _ => failed += 1,
        }
        write_record(&mut w, idx + 1, &rec);
        if (idx + 1) % 500 == 0 {
            println!("已处理 {}/{}", idx + 1, cfg_files.len());
        }
    }

    let elapsed = started.elapsed().as_secs_f64();

    // 汇总
    writeln!(
        w,
        "\n# 汇总: 总数 {} | 成功 {} | 失败 {} | DAT缺失 {} | 总耗时 {:.2} s",
        cfg_files.len(),
        ok,
        failed,
        missing_dat,
        elapsed
    )
    .expect("写汇总失败");
    w.flush().expect("flush 失败");

    println!(
        "完成: 总数 {} | 成功 {} | 失败 {} | DAT缺失 {} | 总耗时 {:.2} s",
        cfg_files.len(),
        ok,
        failed,
        missing_dat,
        elapsed
    );
    println!("日志已写入: {}", log_path);
}

/// 递归收集所有扩展名为 cfg 的文件（大小写不敏感）
fn collect_cfg(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_cfg(&path, out);
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("cfg"))
            .unwrap_or(false)
        {
            out.push(path);
        }
    }
}

/// 分析单个 CFG：计时加载两次取平均（读盘+解析整组），统计文件大小
fn analyze(cfg: &Path) -> Record {
    // 文件大小：CFG + DAT 兄弟文件
    let size_cfg = fs::metadata(cfg).map(|m| m.len()).unwrap_or(0);
    let (size_dat, dat_exists) = sibling_size(cfg, "dat");

    // 加载耗时：两次整组加载取平均（与读取耗时同口径）
    let mut load_ms = 0.0;
    let mut read_ms = 0.0;
    let mut attempts = 0usize;
    let mut station = String::new();
    let mut channels_total = 0usize;
    let mut channels_analog = 0usize;
    let mut channels_status = 0usize;
    let mut samples = 0usize;
    let mut status = String::new();

    for i in 0..2 {
        let t = Instant::now();
        match Comtrade::from_path(cfg) {
            Ok(ct) => {
                attempts += 1;
                let ms = t.elapsed().as_secs_f64() * 1000.0;
                if i == 0 {
                    station = ct.config.header.station.clone();
                    channels_total = ct.config.channels.total;
                    channels_analog = ct.config.channels.analog;
                    channels_status = ct.config.channels.status;
                    samples = ct.data.as_ref().map(|d| d.len()).unwrap_or(0);
                }
                load_ms += ms;
                read_ms += ms;
            },
            Err(e) => {
                // DAT 兄弟文件缺失 → 单独归类；其余为解析失败
                let is_missing_dat = matches!(&e, comtrade_io::Error::NotFound(p)
                    if p.extension().map(|x| x.eq_ignore_ascii_case("dat")).unwrap_or(false));
                status = if is_missing_dat {
                    "DAT缺失".to_string()
                } else {
                    format!("解析失败: {}", e)
                };
                break;
            },
        }
    }
    let n = attempts.max(1) as f64;
    load_ms /= n;
    read_ms /= n;

    // 文件大小：DAT 缺失时以文件是否存在标记状态（加载失败时由错误信息判定）
    let status = if status.is_empty() {
        if dat_exists {
            "OK".to_string()
        } else {
            "DAT缺失".to_string()
        }
    } else {
        status
    };

    Record {
        path: cfg.to_path_buf(),
        station,
        channels_total,
        channels_analog,
        channels_status,
        samples,
        size_total: size_cfg + size_dat,
        size_cfg,
        size_dat,
        load_ms,
        read_ms,
        status,
    }
}

/// 与 cfg 同目录同名（同扩展名大小写风格）的兄弟文件大小
fn sibling_size(cfg: &Path, suffix: &str) -> (u64, bool) {
    let stem = cfg.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let parent = cfg.parent().unwrap_or(Path::new("."));
    // 与 cfg 扩展名大小写保持一致（.CFG -> .DAT）
    let upper = cfg
        .extension()
        .map(|e| e.to_str().unwrap_or("").chars().all(|c| c.is_uppercase()))
        .unwrap_or(false);
    let ext = if upper {
        suffix.to_uppercase()
    } else {
        suffix.to_string()
    };
    let path = parent.join(format!("{}.{}", stem, ext));
    match fs::metadata(&path) {
        Ok(m) => (m.len(), true),
        Err(_) => (0, false),
    }
}

/// 写一行记录（Tab 分隔）
fn write_record(w: &mut impl Write, idx: usize, rec: &Record) {
    writeln!(
        w,
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.3}\t{:.3}\t{}",
        idx,
        rec.path.display(),
        rec.station,
        rec.channels_total,
        rec.channels_analog,
        rec.channels_status,
        rec.samples,
        rec.size_total,
        rec.size_cfg,
        rec.size_dat,
        rec.load_ms,
        rec.read_ms,
        rec.status
    )
    .expect("写记录失败");
}
