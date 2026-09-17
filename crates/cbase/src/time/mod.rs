//! COMTRADE 时间解析与格式化模块
//!
//! 不依赖 chrono，自研轻量日期时间类型，精度到微秒。

use crate::error::{Error, Result};

pub mod format;
pub mod parse;

pub use self::format::format_cfg;
pub use self::parse::parse;

/// 本地墙钟时间（无时区），精度到微秒
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timestamp {
    /// 年
    pub year: u16,
    /// 月（1-12）
    pub month: u8,
    /// 日（1-31）
    pub day: u8,
    /// 时（0-23）
    pub hour: u8,
    /// 分（0-59）
    pub minute: u8,
    /// 秒（0-59）
    pub second: u8,
    /// 微秒（0-999999）
    pub micro: u32,
}

impl Timestamp {
    /// 创建 Timestamp，校验日期合法性
    pub fn new(
        year: u16,
        month: u8,
        day: u8,
        hour: u8,
        minute: u8,
        second: u8,
        micro: u32,
    ) -> Result<Self> {
        if month == 0 || month > 12 {
            return Err(Error::Time(format!("无效月份: {}", month)));
        }
        if day == 0 || day > 31 {
            return Err(Error::Time(format!("无效日期: {}", day)));
        }
        if hour > 23 || minute > 59 || second > 59 {
            return Err(Error::Time("无效时间".into()));
        }
        if micro > 999_999 {
            return Err(Error::Time(format!("无效微秒: {}", micro)));
        }
        // 校验每月天数
        let max_day = days_in_month(year, month);
        if day > max_day {
            // 2/29 非法时降级为 2/28
            if month == 2 && day == 29 {
                return Ok(Self {
                    year,
                    month,
                    day: 28,
                    hour,
                    minute,
                    second,
                    micro,
                });
            }
            return Err(Error::Time(format!(
                "无效日期: {}-{:02}-{:02}",
                year, month, day
            )));
        }
        Ok(Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
            micro,
        })
    }

    /// 转为自某纪元的微秒数（用于加减运算）
    fn to_micros_since_epoch(self) -> i64 {
        let mut days = 0i64;
        // 从 1 年到 year-1 的天数
        for y in 1..self.year as i64 {
            days += days_in_year(y as u16) as i64;
        }
        for m in 1..self.month as i64 {
            days += days_in_month(self.year, m as u8) as i64;
        }
        days += (self.day - 1) as i64;

        days * 24 * 3600 * 1_000_000
            + self.hour as i64 * 3600 * 1_000_000
            + self.minute as i64 * 60 * 1_000_000
            + self.second as i64 * 1_000_000
            + self.micro as i64
    }

    /// 从微秒偏移量重建（用于加减运算）
    fn from_micros_since_epoch(mut total: i64) -> Option<Self> {
        if total < 0 {
            return None;
        }
        let micro = (total % 1_000_000) as u32;
        total /= 1_000_000;
        let second = (total % 60) as u8;
        total /= 60;
        let minute = (total % 60) as u8;
        total /= 60;
        let hour = (total % 24) as u8;
        total /= 24;
        // 从天数反推年月日
        let mut year = 1u16;
        loop {
            let dy = days_in_year(year) as i64;
            if total < dy {
                break;
            }
            total -= dy;
            year += 1;
        }
        let mut month = 1u8;
        loop {
            let dm = days_in_month(year, month) as i64;
            if total < dm {
                break;
            }
            total -= dm;
            month += 1;
        }
        let day = (total + 1) as u8;
        Some(Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
            micro,
        })
    }

    /// 增加微秒偏移
    pub fn add_micros(&self, us: i64) -> Option<Self> {
        let total = (*self).to_micros_since_epoch() + us;
        Self::from_micros_since_epoch(total)
    }
}

pub(crate) fn days_in_year(year: u16) -> u16 {
    if is_leap(year) {
        366
    } else {
        365
    }
}

pub(crate) fn is_leap(year: u16) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

pub(crate) fn days_in_month(year: u16, month: u8) -> u8 {
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
    use super::format::format_cfg;
    use super::parse::parse;
    use super::*;

    #[test]
    fn test_parse_standard() {
        let ts = parse("01/15/2023,10:30:45.123456").unwrap();
        assert_eq!(ts.year, 2023);
        assert_eq!(ts.month, 1);
        assert_eq!(ts.day, 15);
        assert_eq!(ts.hour, 10);
        assert_eq!(ts.minute, 30);
        assert_eq!(ts.second, 45);
        assert_eq!(ts.micro, 123456);
    }

    #[test]
    fn test_parse_european() {
        let ts = parse("15/01/2023,10:30:45.123456").unwrap();
        assert_eq!(ts.day, 15);
        assert_eq!(ts.month, 1);
    }

    #[test]
    fn test_parse_iso() {
        let ts = parse("2023-01-15 10:30:45.123456").unwrap();
        assert_eq!(ts.year, 2023);
        assert_eq!(ts.month, 1);
        assert_eq!(ts.day, 15);
    }

    #[test]
    fn test_parse_short_micro() {
        let ts = parse("01/15/2023,10:30:45.123").unwrap();
        assert_eq!(ts.micro, 123000);
    }

    #[test]
    fn test_parse_variable_digits() {
        // Python strptime 允许 1-2 位数字，本实现对齐（此前固定 2 位导致失败）
        let ts = parse("1/7/2013,7:18:24.000660").unwrap();
        assert_eq!(ts.year, 2013);
        assert_eq!(ts.month, 1);
        assert_eq!(ts.day, 7);
        assert_eq!(ts.hour, 7);
        assert_eq!(ts.minute, 18);
        assert_eq!(ts.second, 24);

        // 欧洲日期 16/3/2014，时分秒单数字
        let ts = parse("16/3/2014,13:27:8.000777").unwrap();
        assert_eq!(ts.day, 16);
        assert_eq!(ts.month, 3);
        assert_eq!(ts.second, 8);
    }

    #[test]
    fn test_parse_european_with_space() {
        let ts = parse("13/05/2015, 11:29:10.416750").unwrap();
        assert_eq!(ts.year, 2015);
        assert_eq!(ts.month, 5);
        assert_eq!(ts.day, 13);
        assert_eq!(ts.hour, 11);
    }

    #[test]
    fn test_parse_dot_separated() {
        // 点分隔日期（部分南瑞设备输出）
        let ts = parse("07.04.13,03:15:02.051000").unwrap();
        assert_eq!(ts.year, 2013);
        assert_eq!(ts.month, 7);
        assert_eq!(ts.day, 4);
        assert_eq!(ts.hour, 3);
    }

    #[test]
    fn test_parse_two_digit_year() {
        let ts = parse("05/18/15,14:08:15.890000").unwrap();
        assert_eq!(ts.year, 2015);
        assert_eq!(ts.month, 5);
        assert_eq!(ts.day, 18);
    }

    #[test]
    fn test_parse_duplicate_dot_microsecond() {
        // 设备输出 `05:20:14.45.800000`（重复点）时对齐 Python：取首个点后数字段补 6 位
        let ts = parse("04/14/14,05:20:14.45.800000").unwrap();
        assert_eq!(ts.year, 2014);
        assert_eq!(ts.month, 4);
        assert_eq!(ts.day, 14);
        assert_eq!(ts.micro, 450000);
    }

    #[test]
    fn test_format_cfg() {
        let ts = Timestamp::new(2023, 1, 15, 10, 30, 45, 123456).unwrap();
        assert_eq!(format_cfg(&ts), "01/15/2023,10:30:45.123456");
    }
}
