//! ASCII DAT 解析与写出

use super::DatFile;
use cbase::error::Result;
use cfg::Config;

/// 解析 ASCII DAT 文本
pub fn parse_ascii(text: &str, cfg: &Config) -> Result<DatFile> {
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return Ok(DatFile::default());
    }

    let analog_count = cfg.channels.analog;
    let status_count = cfg.channels.status;
    let analog_precision = 3; // 默认精度

    let sample_count = lines.len();
    let mut sample_index = Vec::with_capacity(sample_count);
    let mut timestamp_us = Vec::with_capacity(sample_count);
    let mut analogs: Vec<Vec<f64>> = (0..analog_count)
        .map(|_| Vec::with_capacity(sample_count))
        .collect();
    let mut statuses: Vec<Vec<u8>> = (0..status_count)
        .map(|_| Vec::with_capacity(sample_count))
        .collect();

    for line in lines.iter() {
        let parts: Vec<&str> = line.split(',').collect();

        // 序号
        let index: i32 = parts
            .first()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0);
        sample_index.push(index);

        // 时间戳
        let ts: f64 = parts
            .get(1)
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0.0);
        timestamp_us.push(ts);

        // 模拟量
        for (i, col) in analogs.iter_mut().enumerate().take(analog_count) {
            let val = parts
                .get(2 + i)
                .and_then(|s| {
                    let s = s.trim();
                    if s.is_empty()
                        || s.eq_ignore_ascii_case("NA")
                        || s.eq_ignore_ascii_case("null")
                    {
                        Some(0.0)
                    } else {
                        s.parse().ok()
                    }
                })
                .unwrap_or(0.0);
            col.push(val);
        }

        // 状态量
        let status_start = 2 + analog_count;
        for (i, col) in statuses.iter_mut().enumerate().take(status_count) {
            let val: u8 = parts
                .get(status_start + i)
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0);
            col.push(val);
        }
    }

    // 应用模拟量系数
    for (i, col) in analogs.iter_mut().enumerate().take(analog_count) {
        if i < cfg.analogs.len() {
            let ch = &cfg.analogs[i];
            let mult = ch.multiplier;
            let offset = ch.offset;
            for val in col.iter_mut() {
                *val = *val * mult + offset;
                // 应用精度
                if (1..=6).contains(&analog_precision) {
                    let factor = 10f64.powi(analog_precision);
                    *val = (*val * factor).round() / factor;
                }
            }
        }
    }

    Ok(DatFile {
        sample_index,
        timestamp_us,
        analogs,
        statuses,
    })
}

/// 写出 ASCII DAT 文本
pub fn write_ascii(dat: &DatFile, cfg: &Config) -> String {
    let mut output = String::new();
    let sample_count = dat.len();

    // 反算原始值所用的系数
    let multipliers: Vec<f64> = cfg.analogs.iter().map(|a| a.multiplier).collect();
    let offsets: Vec<f64> = cfg.analogs.iter().map(|a| a.offset).collect();

    for row in 0..sample_count {
        let mut parts = Vec::new();
        // 序号
        parts.push(format!("{}", dat.sample_index[row]));
        // 时间戳（取整）
        parts.push(format!("{:.0}", dat.timestamp_us[row]));

        // 模拟量：反算原始整数
        for (ch, col) in dat.analogs.iter().enumerate() {
            let val = col[row];
            let mult = multipliers.get(ch).copied().unwrap_or(1.0);
            let offset = offsets.get(ch).copied().unwrap_or(0.0);
            let raw = if mult.abs() > 1e-10 {
                ((val - offset) / mult).round() as i64
            } else {
                0
            };
            parts.push(format!("{}", raw));
        }

        // 状态量
        for col in &dat.statuses {
            parts.push(format!("{}", col[row]));
        }

        output.push_str(&parts.join(","));
        output.push('\n');
    }

    output
}
