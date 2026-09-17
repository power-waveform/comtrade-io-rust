//! CFG 配置模型与解析/序列化

use std::path::Path;

use super::channel::{parse_f64_or, AnalogChannel, AnalogExt, StatusChannel, StatusExt};
use super::data_type::DataType;
use super::header::{ChannelCount, Header};
use super::reference::{is_iec61850_reference, is_reference_like_ccbm};
use super::sampling::{Sampling, Segment};
use super::version::Version;
use cbase::encoding::{self, Encoding};
use cbase::error::{Error, FileRole, Result};
use cbase::time::{self, Timestamp};

/// 时间信息（1999 版可选）
#[derive(Debug, Clone)]
pub struct TimeInfo {
    /// 时区代码
    pub time_code: String,
    /// 本地时间码
    pub local_code: String,
}

/// 采样时间品质（1999 版可选）
#[derive(Debug, Clone)]
pub struct SamplingTimeQuality {
    /// TMQ 代码（时间质量）
    pub tmq_code: String,
}

/// CFG 配置模型
#[derive(Debug, Clone)]
pub struct Config {
    /// 文件头（第 1 行）
    pub header: Header,
    /// 通道计数（第 2 行）
    pub channels: ChannelCount,
    /// 模拟量通道列表
    pub analogs: Vec<AnalogChannel>,
    /// 状态量通道列表
    pub statuses: Vec<StatusChannel>,
    /// 采样信息（频率与采样段）
    pub sampling: Sampling,
    /// 录波开始时间
    pub start_time: Timestamp,
    /// 触发时间
    pub trigger_time: Timestamp,
    /// DAT 数据文件格式
    pub data_type: DataType,
    /// 时间倍率（时间戳乘以该系数）
    pub timemult: f64,
    /// 时间信息（1999 版及以上可选）
    pub time_info: Option<TimeInfo>,
    /// 采样时间品质（1999 版及以上可选）
    pub sampling_time_quality: Option<SamplingTimeQuality>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            header: Header {
                station: String::new(),
                recorder: String::new(),
                version: Version::V1991,
            },
            channels: ChannelCount {
                total: 0,
                analog: 0,
                status: 0,
            },
            analogs: Vec::new(),
            statuses: Vec::new(),
            sampling: Sampling::default(),
            start_time: Timestamp {
                year: 2000,
                month: 1,
                day: 1,
                hour: 0,
                minute: 0,
                second: 0,
                micro: 0,
            },
            trigger_time: Timestamp {
                year: 2000,
                month: 1,
                day: 1,
                hour: 0,
                minute: 0,
                second: 0,
                micro: 0,
            },
            data_type: DataType::default(),
            timemult: 1.0,
            time_info: None,
            sampling_time_quality: None,
        }
    }
}

impl Config {
    /// 从字符串解析 CFG 配置
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(text: &str) -> Result<Config> {
        // 保留空行以维持行号不变——Python 基线按 `\n` 直接 split 并保序。
        // 若过滤空行，设备在首行输出空行（如部分南瑞文件）时，
        // 通道数行会被误判为文件头，导致整体错位。
        let lines: Vec<&str> = text
            .lines()
            .map(|line| line.trim_end_matches('\r'))
            .collect();

        if lines.len() < 10 {
            return Err(Error::parse(
                FileRole::Cfg,
                0,
                format!("行数不足，至少需要10行，实际{}行", lines.len()),
            ));
        }

        let cursor = 0;

        // 第1行: 文件头
        let header = Header::from_line(lines[cursor]);

        // 第2行: 通道数量
        let channels = ChannelCount::from_line(lines[cursor + 1])?;

        // 跳过通道信息行，处理采样信息
        let sampling_start = channels.total + 2;
        let mut cursor = sampling_start;
        if cursor >= lines.len() {
            return Err(Error::parse(FileRole::Cfg, 0, "行数不足"));
        }

        // 采样频率
        let freq: f64 = lines[cursor].parse().unwrap_or(50.0);
        cursor += 1;

        // 采样段
        let segment_count: usize = lines[cursor].parse().unwrap_or(0);
        cursor += 1;

        let mut segments = Vec::new();
        for _ in 0..segment_count {
            if cursor < lines.len() && !lines[cursor].is_empty() {
                let parts: Vec<&str> = lines[cursor].split(',').collect();
                if parts.len() >= 2 {
                    let samp_rate = parts[0].trim().parse().unwrap_or(0.0);
                    let end_point = parts[1].trim().parse().unwrap_or(0);
                    segments.push(Segment::new(samp_rate, end_point));
                }
            }
            cursor += 1;
        }

        let sampling = Sampling { freq, segments };

        // 开始时间和故障时间
        if cursor + 2 >= lines.len() {
            return Err(Error::parse(FileRole::Cfg, 0, "时间行不足"));
        }
        let start_time = time::parse(lines[cursor])?;
        cursor += 1;
        let trigger_time = time::parse(lines[cursor])?;
        cursor += 1;

        // 数据格式
        let data_type_text = lines[cursor].trim_end_matches(',').trim();
        let data_type = DataType::parse(data_type_text).ok_or_else(|| {
            Error::parse(
                FileRole::Cfg,
                cursor + 1,
                format!("未知数据格式: {}", data_type_text),
            )
        })?;
        cursor += 1;

        // 可选字段
        let mut timemult = 1.0;
        let mut time_info = None;
        let mut sampling_time_quality = None;

        if cursor < lines.len() {
            timemult = parse_f64_or(lines[cursor], 1.0);
            cursor += 1;
        }
        if cursor < lines.len() {
            let parts: Vec<&str> = lines[cursor].split(',').collect();
            if parts.len() >= 2 {
                time_info = Some(TimeInfo {
                    time_code: parts[0].trim().to_string(),
                    local_code: parts[1].trim().to_string(),
                });
            }
            cursor += 1;
        }
        if cursor < lines.len() {
            sampling_time_quality = Some(SamplingTimeQuality {
                tmq_code: lines[cursor].trim().to_string(),
            });
        }

        // 解析模拟量通道
        let mut analogs = Vec::with_capacity(channels.analog);
        for i in 0..channels.analog {
            let line_idx = i + 2;
            if line_idx < lines.len() {
                let mut ch = AnalogChannel::from_cfg_line(lines[line_idx]);
                if ch.index == 0 {
                    ch.index = i + 1;
                }
                if is_reference_like_ccbm(&ch.equipment) || is_iec61850_reference(&ch.equipment) {
                    let reference = std::mem::take(&mut ch.equipment);
                    ch.ext = Some(AnalogExt {
                        reference: Some(reference),
                        ..Default::default()
                    });
                }
                analogs.push(ch);
            }
        }

        // 解析状态量通道
        let mut statuses = Vec::with_capacity(channels.status);
        let status_start = channels.analog + 2;
        for i in 0..channels.status {
            let line_idx = status_start + i;
            if line_idx < lines.len() {
                let mut ch = StatusChannel::from_cfg_line(lines[line_idx]);
                if ch.index == 0 {
                    ch.index = i + 1;
                }
                if is_reference_like_ccbm(&ch.equipment) || is_iec61850_reference(&ch.equipment) {
                    let reference = std::mem::take(&mut ch.equipment);
                    ch.ext = Some(StatusExt {
                        reference: Some(reference),
                        ..Default::default()
                    });
                }
                statuses.push(ch);
            }
        }

        Ok(Config {
            header,
            channels,
            analogs,
            statuses,
            sampling,
            start_time,
            trigger_time,
            data_type,
            timemult,
            time_info,
            sampling_time_quality,
        })
    }

    /// 从文件读取 CFG 配置
    pub fn from_file(path: &Path) -> Result<Config> {
        let (text, _) = encoding::read_text_gbk(path)?;
        Config::from_str(&text)
    }

    /// 序列化为 CFG 文本
    #[allow(clippy::inherent_to_string)]
    pub fn to_string(&self) -> String {
        let mut lines = Vec::new();

        // 头部
        lines.push(self.header.to_line());
        // 通道数
        lines.push(self.channels.to_line());
        // 模拟量通道
        for analog in &self.analogs {
            lines.push(analog.to_cfg_line());
        }
        // 状态量通道
        for status in &self.statuses {
            lines.push(status.to_cfg_line());
        }
        // 采样频率
        lines.push(format!("{}", self.sampling.freq));
        // 采样段数
        lines.push(format!("{}", self.sampling.segments.len()));
        for seg in &self.sampling.segments {
            lines.push(format!("{:.0},{:.0}", seg.samp_rate, seg.end_point));
        }
        // 时间
        lines.push(time::format_cfg(&self.start_time));
        lines.push(time::format_cfg(&self.trigger_time));
        // 数据格式
        lines.push(self.data_type.as_str().to_string());
        // timemult
        lines.push(format!("{}", self.timemult));
        // 可选字段
        if let Some(ref ti) = self.time_info {
            lines.push(format!("{},{}", ti.time_code, ti.local_code));
        }
        if let Some(ref stq) = self.sampling_time_quality {
            lines.push(stq.tmq_code.clone());
        }

        lines.join("\n")
    }

    /// 写入 CFG 文件（默认 GBK 编码）
    pub fn write_file(&self, path: &Path) -> Result<()> {
        encoding::write_text(path, &self.to_string(), Encoding::Gbk)
    }

    /// 获取模拟量通道（index 为 1 基）
    pub fn analog(&self, index: usize) -> Option<&AnalogChannel> {
        self.analogs.iter().find(|a| a.index == index)
    }

    /// 获取状态量通道（index 为 1 基）
    pub fn status(&self, index: usize) -> Option<&StatusChannel> {
        self.statuses.iter().find(|s| s.index == index)
    }
}
