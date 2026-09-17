//! 时间格式化

use crate::time::Timestamp;

/// 格式化为 CFG 标准格式: MM/DD/YYYY,HH:MM:SS.ffffff
pub fn format_cfg(ts: &Timestamp) -> String {
    format!(
        "{:02}/{:02}/{:04},{:02}:{:02}:{:02}.{:06}",
        ts.month, ts.day, ts.year, ts.hour, ts.minute, ts.second, ts.micro
    )
}
