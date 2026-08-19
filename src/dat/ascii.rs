//! ASCII DAT 解析与写出

use super::DatFile;
use crate::cfg::Config;
use crate::error::{Error, FileRole, Result};

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

    let expected_columns = 2 + analog_count + status_count;

    for (line_idx, line) in lines.iter().enumerate() {
        let line_no = line_idx + 1;
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() != expected_columns {
            return Err(Error::parse(
                FileRole::Dat,
                line_no,
                format!(
                    "列数与 CFG 不一致：期望 {} 列，实际 {} 列",
                    expected_columns,
                    parts.len()
                ),
            ));
        }

        // 序号
        let index = parse_i32(parts[0], line_no, 1, "采样序号")?;
        sample_index.push(index);

        // 时间戳
        let ts = parse_f64(parts[1], line_no, 2, "时间戳")?;
        timestamp_us.push(ts);

        // 模拟量
        for (i, col) in analogs.iter_mut().enumerate().take(analog_count) {
            let val = parse_f64(parts[2 + i], line_no, 3 + i, "模拟量")?;
            col.push(val);
        }

        // 状态量
        let status_start = 2 + analog_count;
        for (i, col) in statuses.iter_mut().enumerate().take(status_count) {
            let val = parse_status(parts[status_start + i], line_no, status_start + i + 1)?;
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

fn parse_i32(s: &str, line: usize, column: usize, field: &str) -> Result<i32> {
    let value = s.trim();
    if value.is_empty() {
        return Err(empty_field(line, column, field));
    }
    value.parse().map_err(|_| {
        Error::parse(
            FileRole::Dat,
            line,
            format!("第 {} 列 {} 不是有效整数: '{}'", column, field, value),
        )
    })
}

fn parse_f64(s: &str, line: usize, column: usize, field: &str) -> Result<f64> {
    let value = s.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("NA") || value.eq_ignore_ascii_case("null") {
        return Err(empty_field(line, column, field));
    }
    let parsed = value.parse::<f64>().map_err(|_| {
        Error::parse(
            FileRole::Dat,
            line,
            format!("第 {} 列 {} 不是有效浮点数: '{}'", column, field, value),
        )
    })?;
    if !parsed.is_finite() {
        return Err(Error::parse(
            FileRole::Dat,
            line,
            format!("第 {} 列 {} 不是有限数值: '{}'", column, field, value),
        ));
    }
    Ok(parsed)
}

fn parse_status(s: &str, line: usize, column: usize) -> Result<u8> {
    let value = s.trim();
    if value.is_empty() {
        return Err(empty_field(line, column, "状态量"));
    }
    let parsed = value.parse::<u8>().map_err(|_| {
        Error::parse(
            FileRole::Dat,
            line,
            format!("第 {} 列 状态量 不是有效整数: '{}'", column, value),
        )
    })?;
    if parsed > 1 {
        return Err(Error::parse(
            FileRole::Dat,
            line,
            format!("第 {} 列 状态量 只能为 0 或 1: '{}'", column, value),
        ));
    }
    Ok(parsed)
}

fn empty_field(line: usize, column: usize, field: &str) -> Error {
    Error::parse(
        FileRole::Dat,
        line,
        format!("第 {} 列 {} 为空或缺失", column, field),
    )
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
