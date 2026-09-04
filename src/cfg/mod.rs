//! CFG 配置文件读写模块
//!
//! 支持 COMTRADE 1991/1999 版本 CFG 文件的解析与序列化。

mod channel;
mod sampling;

pub use channel::{
    is_iec61850_reference, is_reference_like_ccbm, parse_f64_or, AnalogChannel, AnalogExt,
    DataType, StatusChannel, StatusExt, TranSide, Version,
};
pub use sampling::{Sampling, Segment, DEFAULT_NOMINAL_FREQ};

use std::path::Path;

use crate::encoding::{self, Encoding};
use crate::error::{Error, FileRole, Result};
use crate::time::{self, Timestamp};

/// 文件头
#[derive(Debug, Clone)]
pub struct Header {
    /// 站名（录波装置所在变电站/站点名称）
    pub station: String,
    /// 录波装置名称
    pub recorder: String,
    /// 录波数据标准版本（1991/1999/2001/2008/2013/2017）
    pub version: Version,
}

impl Header {
    fn from_line(line: &str) -> Header {
        let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
        let station = parts.first().unwrap_or(&"").to_string();
        let recorder = parts.get(1).unwrap_or(&"").to_string();
        // 六个版本号全部识别；头部无版本字段或版本号无法识别时回落 1991
        // （对齐 Python `Version.from_value(str_arr[2], Version.V1991)`）。
        // 原实现只匹配 "1999"，导致 2001/2008/2013/2017 的文件被静默判成 1991。
        let version = parts
            .get(2)
            .and_then(|s| Version::parse(s))
            .unwrap_or_default();
        Header {
            station,
            recorder,
            version,
        }
    }

    fn to_line(&self) -> String {
        format!(
            "{},{},{}",
            self.station,
            self.recorder,
            self.version.as_str()
        )
    }
}

/// 通道计数
#[derive(Debug, Clone, Copy)]
pub struct ChannelCount {
    /// 通道总数（模拟量 + 状态量）
    pub total: usize,
    /// 模拟量通道数
    pub analog: usize,
    /// 状态量通道数
    pub status: usize,
}

impl ChannelCount {
    fn from_line(line: &str) -> Result<ChannelCount> {
        let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
        if parts.len() < 3 {
            return Err(Error::parse(FileRole::Cfg, 2, "通道数行格式错误"));
        }
        let total = parts[0]
            .parse::<usize>()
            .map_err(|_| Error::number(FileRole::Cfg, parts[0].to_string()))?;
        let analog = extract_digits(parts.get(1).unwrap_or(&"0"))
            .map_err(|_| Error::number(FileRole::Cfg, parts[1].to_string()))?;
        let status = extract_digits(parts.get(2).unwrap_or(&"0"))
            .map_err(|_| Error::number(FileRole::Cfg, parts[2].to_string()))?;
        Ok(ChannelCount {
            total,
            analog,
            status,
        })
    }

    fn to_line(self) -> String {
        format!("{},{}A,{}D", self.total, self.analog, self.status)
    }
}

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

/// 从 "96A" / "192D" 类字符串中提取数字
fn extract_digits(s: &str) -> std::result::Result<usize, std::num::ParseIntError> {
    let digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return Ok(0);
    }
    digits.parse()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic_cfg() {
        let cfg_text = "\
            STATION,RECORDER,1999\n\
            12,6A,6D\n\
            1,IA, A, ,A,1.0,0.0,0.0,-32767,32767,1.0,1.0,S\n\
            2,IB, B, ,A,1.0,0.0,0.0,-32767,32767,1.0,1.0,S\n\
            3,IC, C, ,A,1.0,0.0,0.0,-32767,32767,1.0,1.0,S\n\
            4,UA, A, ,V,1.0,0.0,0.0,-32767,32767,1.0,1.0,S\n\
            5,UB, B, ,V,1.0,0.0,0.0,-32767,32767,1.0,1.0,S\n\
            6,UC, C, ,V,1.0,0.0,0.0,-32767,32767,1.0,1.0,S\n\
            1,D1, , ,0\n\
            2,D2, , ,0\n\
            3,D3, , ,0\n\
            4,D4, , ,0\n\
            5,D5, , ,0\n\
            6,D6, , ,0\n\
            50.0\n\
            1\n\
            1200,1200\n\
            01/15/2023,10:30:45.123456\n\
            01/15/2023,10:30:45.123456\n\
            ASCII\n\
            1.0";
        let config = Config::from_str(cfg_text).unwrap();
        assert_eq!(config.header.station, "STATION");
        assert_eq!(config.channels.analog, 6);
        assert_eq!(config.channels.status, 6);
        assert_eq!(config.analogs.len(), 6);
        assert_eq!(config.statuses.len(), 6);
        assert_eq!(config.data_type, DataType::Ascii);
    }

    #[test]
    fn ccbm_reference_compatibility_moves_illegal_values_to_reference() {
        let cfg_text = "\
            STATION,RECORDER,1999\n\
            2,1A,1D\n\
            1,Ua,A,MUSV$TVTR1$MX$Vol,V,1,0,0,-1,1,1,1,S\n\
            1,Trip,,SVOUTMUSV$TCTR1$MX$AmpR,0\n\
            50\n\
            1\n\
            100,100\n\
            01/01/2020,00:00:00.000000\n\
            01/01/2020,00:00:00.000000\n\
            ASCII\n\
            1";
        let cfg = Config::from_str(cfg_text).unwrap();
        assert!(cfg.analogs[0].equipment.is_empty());
        assert_eq!(
            cfg.analogs[0]
                .ext
                .as_ref()
                .and_then(|e| e.reference.as_deref()),
            Some("MUSV$TVTR1$MX$Vol")
        );
        assert!(cfg.statuses[0].equipment.is_empty());
        assert_eq!(
            cfg.statuses[0]
                .ext
                .as_ref()
                .and_then(|e| e.reference.as_deref()),
            Some("SVOUTMUSV$TCTR1$MX$AmpR")
        );
    }

    #[test]
    fn ccbm_reference_detection_does_not_match_normal_equipment() {
        assert!(is_reference_like_ccbm("MUSV$TVTR1$MX$Vol"));
        assert!(is_reference_like_ccbm("svoutmusv$tctr1$mx$ampr"));
        assert!(!is_reference_like_ccbm("线路1"));
        assert!(!is_reference_like_ccbm("MUSV设备"));
    }

    #[test]
    fn iec61850_ccbm_is_kept_as_reference_and_not_equipment() {
        assert!(is_iec61850_reference("PTRC$ST$Tr$general"));
        assert!(is_iec61850_reference("TCTR$MX$Amp$"));
        assert!(!is_iec61850_reference("线路$1"));
        let cfg_text = "\
            STATION,RECORDER,1999\n\
            1,0A,1D\n\
            1,Trip,,PTRC$ST$Tr$general,0\n\
            50\n\
            1\n\
            100,100\n\
            01/01/2020,00:00:00.000000\n\
            01/01/2020,00:00:00.000000\n\
            ASCII\n\
            1";
        let cfg = Config::from_str(cfg_text).unwrap();
        assert!(cfg.statuses[0].equipment.is_empty());
        assert_eq!(
            cfg.statuses[0]
                .ext
                .as_ref()
                .and_then(|ext| ext.reference.as_deref()),
            Some("PTRC$ST$Tr$general")
        );
    }

    #[test]
    fn test_round_trip() {
        let original = "\
            STATION,RECORDER,1999\n\
            12,6A,6D\n\
            1,IA, A, ,A,1.0,0.0,0.0,-32767,32767,1.0,1.0,S\n\
            2,IB, B, ,A,1.0,0.0,0.0,-32767,32767,1.0,1.0,S\n\
            3,IC, C, ,A,1.0,0.0,0.0,-32767,32767,1.0,1.0,S\n\
            4,UA, A, ,V,1.0,0.0,0.0,-32767,32767,1.0,1.0,S\n\
            5,UB, B, ,V,1.0,0.0,0.0,-32767,32767,1.0,1.0,S\n\
            6,UC, C, ,V,1.0,0.0,0.0,-32767,32767,1.0,1.0,S\n\
            1,D1, , ,0\n\
            2,D2, , ,0\n\
            3,D3, , ,0\n\
            4,D4, , ,0\n\
            5,D5, , ,0\n\
            6,D6, , ,0\n\
            50\n\
            1\n\
            1200,1200\n\
            01/15/2023,10:30:45.123456\n\
            01/15/2023,10:30:45.123456\n\
            ASCII\n\
            1";
        let config = Config::from_str(original).unwrap();
        let serialized = config.to_string();
        let reparsed = Config::from_str(&serialized).unwrap();
        assert_eq!(reparsed.header.station, config.header.station);
        assert_eq!(reparsed.channels.analog, config.channels.analog);
        assert_eq!(reparsed.analogs.len(), config.analogs.len());
        assert_eq!(reparsed.start_time, config.start_time);
    }

    #[test]
    fn test_empty_first_line_keeps_row_alignment() {
        // 回归：设备在首行输出空行时（部分南瑞文件），过滤空行会把
        // 通道数行误判为文件头导致整体错位。空行必须保留以维持行号。
        let cfg_text = "\
            \n\
            2,1A,1D\n\
            1,Ia,a,0,A,1.0,0.0,0.0,-16384,16384\n\
            1,D1, , ,0\n\
            50\n\
            1\n\
            1000,317\n\
            01/15/2016,10:30:45.123456\n\
            01/15/2016,10:30:45.123456\n\
            BINARY\n\
            1.0";
        let config = Config::from_str(cfg_text).unwrap();
        assert_eq!(config.channels.analog, 1);
        assert_eq!(config.channels.status, 1);
        assert_eq!(config.analogs.len(), 1);
        assert_eq!(config.sampling.freq, 50.0);
        // 空首行被当作空 header（station 为空），但后续行必须不错位
        assert_eq!(config.start_time.year, 2016);
    }

    #[test]
    fn test_version_parse_all_six() {
        for (s, v) in [
            ("1991", Version::V1991),
            ("1999", Version::V1999),
            ("2001", Version::V2001),
            ("2008", Version::V2008),
            ("2013", Version::V2013),
            ("2017", Version::V2017),
        ] {
            assert_eq!(Version::parse(s), Some(v), "版本 {}", s);
            assert_eq!(v.as_str(), s, "as_str 应往返一致");
            assert!(!v.standard().is_empty());
        }
        // 前后空白应容忍
        assert_eq!(Version::parse(" 2013 "), Some(Version::V2013));
        // 未知版本号返回 None，由调用方决定回落
        assert_eq!(Version::parse("2020"), None);
        assert_eq!(Version::parse("abc"), None);
        assert_eq!(Version::parse(""), None);
    }

    #[test]
    fn test_version_is_1999_or_later() {
        // 只有 1991 没有 CFG 可选尾部字段
        assert!(!Version::V1991.is_1999_or_later());
        for v in [
            Version::V1999,
            Version::V2001,
            Version::V2008,
            Version::V2013,
            Version::V2017,
        ] {
            assert!(v.is_1999_or_later(), "{} 应视为 1999 及以后", v.as_str());
        }
        assert_eq!(Version::default(), Version::V1991);
    }

    #[test]
    fn test_header_parses_all_versions() {
        // 关键回归：原实现只匹配 "1999"，2001/2008/2013/2017 会被静默判成 1991
        for s in ["1991", "1999", "2001", "2008", "2013", "2017"] {
            let h = Header::from_line(&format!("ST,REC,{}", s));
            assert_eq!(h.station, "ST");
            assert_eq!(h.recorder, "REC");
            assert_eq!(h.version.as_str(), s, "头部版本 {} 应被识别", s);
            // 序列化必须写回原版本号，不能塌成 1991
            assert_eq!(h.to_line(), format!("ST,REC,{}", s));
        }
    }

    #[test]
    fn test_header_defaults_to_1991_when_version_absent() {
        // 头部为空 / 缺版本字段 → 1991
        assert_eq!(Header::from_line("").version, Version::V1991);
        assert_eq!(Header::from_line("ST").version, Version::V1991);
        assert_eq!(Header::from_line("ST,REC").version, Version::V1991);
        assert_eq!(Header::from_line("ST,REC,").version, Version::V1991);
        // 无法识别的版本号同样回落 1991（对齐 Python）
        assert_eq!(Header::from_line("ST,REC,2020").version, Version::V1991);
        assert_eq!(Header::from_line("ST,REC,abc").version, Version::V1991);
        // 回落不应影响站名/设备名
        let h = Header::from_line("ST,REC,2020");
        assert_eq!((h.station.as_str(), h.recorder.as_str()), ("ST", "REC"));
    }

    #[test]
    fn test_cfg_2013_keeps_version_through_round_trip() {
        // 2013 版 CFG：可选尾部字段与 1999 布局兼容，不应因版本号而丢失
        let text = "\
            GHBZ,220kV线路故障,2013\n\
            2,1A,1D\n\
            1,IA, A, ,A,1.0,0.0,0.0,-32767,32767,1.0,1.0,S\n\
            1,D1, , ,0\n\
            50\n\
            1\n\
            1200,1200\n\
            01/15/2023,10:30:45.123456\n\
            01/15/2023,10:30:45.123456\n\
            ASCII\n\
            1\n\
            UTC-8,UTC-8\n\
            0000";
        let cfg = Config::from_str(text).unwrap();
        assert_eq!(cfg.header.version, Version::V2013);
        assert!(cfg.header.version.is_1999_or_later());
        assert!(cfg.time_info.is_some(), "2013 版的时间码行应被解析");
        assert!(cfg.sampling_time_quality.is_some());

        let reparsed = Config::from_str(&cfg.to_string()).unwrap();
        assert_eq!(reparsed.header.version, Version::V2013, "版本号须往返保真");
        assert_eq!(reparsed.timemult, cfg.timemult);
        assert_eq!(
            reparsed.time_info.as_ref().map(|t| t.time_code.clone()),
            cfg.time_info.as_ref().map(|t| t.time_code.clone())
        );
    }
}
