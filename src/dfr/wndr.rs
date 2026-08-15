//! WNDR 文本头解析

use crate::error::{Error, FileRole, Result};

/// WNDR 模拟通道
#[derive(Debug, Clone, Default)]
pub struct WndrAnalogChannel {
    /// 通道序号
    pub idx: usize,
    /// 通道名称
    pub name: String,
    /// 相别代码
    pub phase_code: String,
    /// 监测回路
    pub monitor_loop: String,
    /// 满刻度值
    pub full_scale: f64,
    /// 转换系数
    pub conversion_factor: f64,
    /// 物理单位
    pub unit: String,
    /// 相别
    pub phase: String,
    /// 通道类型
    pub ch_type: u32,
    /// 一次侧额定值
    pub rated_primary: f64,
}

/// WNDR 状态通道
#[derive(Debug, Clone, Default)]
pub struct WndrStatusChannel {
    /// 通道序号
    pub idx: usize,
    /// 通道名称
    pub name: String,
}

/// WNDR 文本头
#[derive(Debug, Clone, Default)]
pub struct WndrSection {
    /// 站名
    pub station_name: String,
    /// 模拟通道列表
    pub analog_channels: Vec<WndrAnalogChannel>,
    /// 状态通道列表
    pub status_channels: Vec<WndrStatusChannel>,
    /// 满刻度值（缺省 32768）
    pub full_scale: u32,
    /// 每周波采样点数（缺省 24）
    pub samples_per_cycle: u32,
    /// 总采样点数
    pub total_samples: usize,
    /// 电网频率（Hz，缺省 50）
    pub grid_freq: f64,
}

impl WndrSection {
    /// 解析 WNDR 文本
    pub fn from_text(text: &str) -> Result<WndrSection> {
        let lines: Vec<&str> = text.lines().collect();
        if lines.is_empty() {
            return Err(Error::parse(FileRole::Dfr, 0, "WNDR 文本为空"));
        }

        let mut cursor = 0;

        // 行 1: 魔数 [WNDR]（缺失告警不中断）
        let first_line = lines[cursor].trim();
        if !first_line.eq_ignore_ascii_case("[WNDR]") {
            // 告警但不中断
        }
        cursor += 1;

        // 行 2: 站名
        let station_name = if cursor < lines.len() {
            lines[cursor].trim().to_string()
        } else {
            String::new()
        };
        cursor += 1;

        // 行 3: 通道计数 "total,xxA,xxD"
        let (analog_count, status_count) = if cursor < lines.len() {
            parse_channel_count(lines[cursor])
        } else {
            (0, 0)
        };
        cursor += 1;

        // 模拟通道行
        let mut analog_channels = Vec::new();
        for _ in 0..analog_count {
            if cursor < lines.len() {
                if let Some(ch) = parse_analog_line(lines[cursor]) {
                    analog_channels.push(ch);
                }
                cursor += 1;
            }
        }

        // 状态通道行
        let mut status_channels = Vec::new();
        for _ in 0..status_count {
            if cursor < lines.len() {
                let parts: Vec<&str> = lines[cursor].split(',').collect();
                let idx = parts
                    .first()
                    .and_then(|s| s.trim().parse().ok())
                    .unwrap_or(0);
                let name = parts.get(1).unwrap_or(&"").trim().to_string();
                status_channels.push(WndrStatusChannel { idx, name });
                cursor += 1;
            }
        }

        // 尾部三行
        let full_scale = if cursor < lines.len() {
            lines[cursor].trim().parse().unwrap_or(DEFAULT_FULL_SCALE)
        } else {
            DEFAULT_FULL_SCALE
        };
        cursor += 1;

        let samples_per_cycle = if cursor < lines.len() {
            lines[cursor].trim().parse().unwrap_or(DEFAULT_SPC)
        } else {
            DEFAULT_SPC
        };
        cursor += 1;

        let total_samples = if cursor < lines.len() {
            lines[cursor].trim().parse().unwrap_or(0)
        } else {
            0
        };

        Ok(WndrSection {
            station_name,
            analog_channels,
            status_channels,
            full_scale,
            samples_per_cycle,
            total_samples,
            grid_freq: DEFAULT_GRID_FREQ,
        })
    }
}

use super::{DEFAULT_FULL_SCALE, DEFAULT_GRID_FREQ, DEFAULT_SAMPLES_PER_CYCLE as DEFAULT_SPC};

fn parse_channel_count(line: &str) -> (usize, usize) {
    let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
    let analog = parts.get(1).and_then(|s| extract_digits(s)).unwrap_or(0);
    let status = parts.get(2).and_then(|s| extract_digits(s)).unwrap_or(0);
    (analog, status)
}

fn parse_analog_line(line: &str) -> Option<WndrAnalogChannel> {
    let parts: Vec<&str> = line.split(',').collect();
    if parts.len() < 8 {
        return None;
    }
    let idx = parts[0].trim().parse().unwrap_or(0);
    let name = parts[1].trim().to_string();
    let phase_code = parts.get(2).unwrap_or(&"").trim().to_string();
    let monitor_loop = parts.get(3).unwrap_or(&"").trim().to_string();
    let full_scale = parts
        .get(4)
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0.0);
    let conversion_factor = parts
        .get(5)
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(1.0);
    let unit = parts.get(6).unwrap_or(&"").trim().to_string();
    let phase = parts.get(7).unwrap_or(&"").trim().to_string();
    let ch_type = parts
        .get(12)
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);
    let rated_primary = parts
        .get(13)
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0.0);

    Some(WndrAnalogChannel {
        idx,
        name,
        phase_code,
        monitor_loop,
        full_scale,
        conversion_factor,
        unit,
        phase,
        ch_type,
        rated_primary,
    })
}

fn extract_digits(s: &str) -> Option<usize> {
    let digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}
