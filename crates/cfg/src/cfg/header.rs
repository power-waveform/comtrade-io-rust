//! CFG 文件头与通道计数

use super::version::Version;
use cbase::error::{Error, FileRole, Result};

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
    /// 从 CFG 首行解析文件头（3 字段，缺失/非法版本回落 1991）
    pub fn from_line(line: &str) -> Header {
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

    /// 序列化为 CFG 首行
    pub fn to_line(&self) -> String {
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
    pub(crate) fn from_line(line: &str) -> Result<ChannelCount> {
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

    pub(crate) fn to_line(self) -> String {
        format!("{},{}A,{}D", self.total, self.analog, self.status)
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
