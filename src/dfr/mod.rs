//! DFR 文件只读模块
//!
//! DFR 是 WNDR 文本头 + `[Data]` 二进制区的单文件格式，只读。

mod binary;
mod converter;
mod wndr;

pub use binary::DfrBinary;
pub use wndr::WndrSection;

use std::path::Path;

use crate::cfg::Config;
use crate::dat::DatFile;
use crate::encoding;
use crate::error::{Error, FileRole, Result};
use crate::time::Timestamp;

/// DFR 文件
#[derive(Debug, Clone)]
pub struct DfrFile {
    /// WNDR 文本头
    pub wndr: WndrSection,
    /// 二进制区
    pub binary: DfrBinary,
    /// 文件修改时间（作为录波时间）
    pub file_time: Timestamp,
}

impl Default for DfrFile {
    fn default() -> Self {
        DfrFile {
            wndr: WndrSection::default(),
            binary: DfrBinary::default(),
            file_time: Timestamp {
                year: 2000,
                month: 1,
                day: 1,
                hour: 0,
                minute: 0,
                second: 0,
                micro: 0,
            },
        }
    }
}

/// 常量
pub const WNDR_TEXT_SIZE: usize = 4088;
pub const DATA_MARKER: &[u8] = b"[Data]\r\n";
pub const DEFAULT_FULL_SCALE: u32 = 32768;
pub const DEFAULT_SAMPLES_PER_CYCLE: u32 = 24;
pub const DEFAULT_GRID_FREQ: f64 = 50.0;
pub const DEFAULT_END_POINT: usize = 12612;

/// 已知设备注册表：device_id → 私有头尺寸
pub fn device_header_size(device_id: &str) -> Option<usize> {
    match device_id {
        "2704V042" | "2704V072" => Some(248),
        _ => None,
    }
}

impl DfrFile {
    /// 从文件读取 DFR
    pub fn from_file(path: &Path) -> Result<DfrFile> {
        let bytes = std::fs::read(path).map_err(|e| Error::Io {
            path: path.to_path_buf(),
            source: e,
        })?;

        // 获取文件修改时间作为起止时间
        let metadata = std::fs::metadata(path).map_err(|e| Error::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
        let mtime = metadata.modified().map_err(|e| Error::Io {
            path: path.to_path_buf(),
            source: e,
        })?;

        // 从 SystemTime 构建 Timestamp
        let time = system_time_to_timestamp(mtime);

        DfrFile::from_bytes(&bytes, time)
    }

    /// 从字节解析 DFR
    pub fn from_bytes(data: &[u8], time: Timestamp) -> Result<DfrFile> {
        // 切分 WNDR 文本头与二进制区
        let wndr_bytes = if data.len() > WNDR_TEXT_SIZE {
            &data[..WNDR_TEXT_SIZE]
        } else {
            data
        };

        // cp1251 解码 WNDR
        let wndr_text = encoding::decode_with(wndr_bytes, encoding::Encoding::Cp1251);
        let wndr = WndrSection::from_text(&wndr_text)?;

        // 定位 [Data]\r\n 标记；老格式缺标记时从固定 WNDR 头后读取。
        let data_offset = match find_data_marker(data) {
            Some(offset) => offset,
            None if data.len() >= WNDR_TEXT_SIZE => WNDR_TEXT_SIZE,
            None => {
                return Err(Error::binary(
                    FileRole::Dfr,
                    data.len(),
                    "缺少 [Data]\\r\\n 标记，且文件短于 WNDR 文本头",
                ));
            },
        };
        let binary_data = &data[data_offset..];
        let binary = DfrBinary::from_raw(binary_data);

        Ok(DfrFile {
            wndr,
            binary,
            file_time: time,
        })
    }

    /// 转换为 Config
    pub fn to_config(&self, time: Timestamp) -> Config {
        converter::wndr_to_config(&self.wndr, time)
    }

    /// 转换为 DatFile
    pub fn to_data(&self, cfg: &Config) -> Result<DatFile> {
        self.binary.to_data(cfg)
    }
}

/// 定位 `[Data]\r\n` 标记
fn find_data_marker(data: &[u8]) -> Option<usize> {
    data.windows(DATA_MARKER.len())
        .position(|w| w == DATA_MARKER)
        .map(|p| p + DATA_MARKER.len())
}

/// SystemTime → Timestamp（简化）
fn system_time_to_timestamp(st: std::time::SystemTime) -> Timestamp {
    use std::time::UNIX_EPOCH;
    let dur = st.duration_since(UNIX_EPOCH).unwrap_or_default();
    let total_secs = dur.as_secs();
    let micros = dur.subsec_micros();

    // 从 Unix 时间戳计算日期（简化算法）
    let mut days = (total_secs / 86400) as i64;
    let secs_of_day = (total_secs % 86400) as u32;

    let mut year = 1970u16;
    loop {
        let dy = days_in_year(year) as i64;
        if days < dy {
            break;
        }
        days -= dy;
        year += 1;
    }

    let mut month = 1u8;
    loop {
        let dm = days_in_month(year, month) as i64;
        if days < dm {
            break;
        }
        days -= dm;
        month += 1;
    }

    let day = (days + 1) as u8;
    let hour = (secs_of_day / 3600) as u8;
    let minute = ((secs_of_day % 3600) / 60) as u8;
    let second = (secs_of_day % 60) as u8;

    Timestamp::new(year, month, day, hour, minute, second, micros).unwrap_or(Timestamp {
        year: 2000,
        month: 1,
        day: 1,
        hour: 0,
        minute: 0,
        second: 0,
        micro: 0,
    })
}

fn days_in_year(year: u16) -> u16 {
    if is_leap(year) {
        366
    } else {
        365
    }
}

fn is_leap(year: u16) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

fn days_in_month(year: u16, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap(year) {
                29
            } else {
                28
            }
        },
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_time() -> Timestamp {
        Timestamp::new(2023, 1, 1, 0, 0, 0, 0).unwrap()
    }

    #[test]
    fn test_short_dfr_without_data_marker_returns_error() {
        let data = b"[WNDR]\r\nSTATION\r\n0,0A,0D\r\n32768\r\n24\r\n0\r\n";
        let err = DfrFile::from_bytes(data, test_time()).expect_err("短 DFR 不应 panic 或成功");
        assert!(
            err.to_string().contains("缺少 [Data]"),
            "错误应说明缺少 Data 标记，实际: {}",
            err
        );
    }
}
