//! COMTRADE 时间解析与格式化模块
//!
//! 不依赖 chrono，自研轻量日期时间类型，精度到微秒。

use crate::error::{Error, Result};

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

/// 解析时间字符串，按 Python 优先级表尝试 15 种格式族
pub fn parse(s: &str) -> Result<Timestamp> {
    let s = s.trim();
    // 折叠时间冒号两侧空格
    let s = collapse_colon_spaces(s);
    // 微秒部分对齐
    let s = normalize_microsecond(&s);
    // 2/29 降级标记
    let s = fix_feb29(&s);

    // 按优先级尝试各格式
    for fmt in TIME_FORMATS {
        if let Some(ts) = try_format(&s, fmt) {
            return ts;
        }
    }
    Err(Error::Time(format!("无法解析时间: '{}'", s)))
}

/// 格式化为 CFG 标准格式: MM/DD/YYYY,HH:MM:SS.ffffff
pub fn format_cfg(ts: &Timestamp) -> String {
    format!(
        "{:02}/{:02}/{:04},{:02}:{:02}:{:02}.{:06}",
        ts.month, ts.day, ts.year, ts.hour, ts.minute, ts.second, ts.micro
    )
}

fn collapse_colon_spaces(s: &str) -> String {
    // 简单策略：移除时间部分中冒号两边的空格
    let mut result = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == ':' {
            // 移除冒号前面的空格
            while result.ends_with(' ') {
                result.pop();
            }
            result.push(':');
            i += 1;
            // 跳过冒号后面的空格
            while i < chars.len() && chars[i] == ' ' {
                i += 1;
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }
    result
}

fn normalize_microsecond(s: &str) -> String {
    if let Some(dot_pos) = s.rfind('.') {
        let after_dot = &s[dot_pos + 1..];
        let digits: String = after_dot
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        if digits.len() < 6 {
            let padded = format!("{:0<6}", digits);
            let non_digit_start = after_dot
                .chars()
                .skip_while(|c| c.is_ascii_digit())
                .collect::<String>();
            format!("{}.{}{}", &s[..dot_pos], &padded[..6], non_digit_start)
        } else {
            format!("{}.{}", &s[..dot_pos], &digits[..6])
        }
    } else {
        s.to_string()
    }
}

fn fix_feb29(s: &str) -> String {
    if s.contains("29") && (s.contains("02/") || s.contains("/02/")) {
        s.replace("/29/", "/28/")
    } else {
        s.to_string()
    }
}

/// 时间格式族（按优先级排序，美式优先以匹配 format_cfg 输出）
const TIME_FORMATS: &[&str] = &[
    "%m/%d/%Y,%H:%M:%S.%f", // 四位年，美国格式（月/日/年）——优先，与 format_cfg 一致
    "%d/%m/%Y,%H:%M:%S.%f", // 四位年，欧洲格式（日/月/年）
    "%m/%d/%y,%H:%M:%S.%f", // 两位年，美国格式（月/日/年）
    "%d/%m/%y,%H:%M:%S.%f", // 两位年，欧洲格式（日/月/年）
    "%m/%d/%Y, %H:%M:%S.%f", // 四位年，美国格式，带空格
    "%Y-%m-%d %H:%M:%S",    // ISO日期格式，无微秒
    "%Y-%m-%d %H:%M:%S.%f", // ISO日期格式，带微秒
    "%Y-%m-%dT%H:%M:%S.%f", // ISO日期格式，T分隔，带微秒
    "%Y-%m-%d,%H:%M:%S.%f", // ISO日期格式，逗号分隔，带微秒
    "%Y/%m/%d %H:%M:%S",    // 斜杠分隔的年月日格式
    "%Y/%m/%d %H:%M:%S.%f", // 带微秒的斜杠分隔格式
    "%d/%m/%Y %H:%M:%S",    // 欧洲常用格式，空格分隔
    "%d/%m/%Y %H:%M:%S.%f", // 带微秒的欧洲常用格式
    "%m/%d/%Y %H:%M:%S",    // 美国常用格式，空格分隔
    "%m/%d/%Y %H:%M:%S.%f", // 带微秒的美国常用格式
];

fn try_format(s: &str, fmt: &str) -> Option<Result<Timestamp>> {
    let parts = parse_format_parts(fmt);
    parse_with_parts(s, &parts).map(Ok)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FmtPart {
    D,   // %d 日
    M,   // %m 月
    Y,   // %Y 四位年
    Y2,  // %y 两位年
    H,   // %H 时
    Min, // %M 分
    S,   // %S 秒
    F,   // %f 微秒
    Lit(char),
}

fn parse_format_parts(fmt: &str) -> Vec<FmtPart> {
    let mut parts = Vec::new();
    let chars: Vec<char> = fmt.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '%' && i + 1 < chars.len() {
            let part = match chars[i + 1] {
                'd' => FmtPart::D,
                'm' => FmtPart::M,
                'Y' => FmtPart::Y,
                'y' => FmtPart::Y2,
                'H' => FmtPart::H,
                'M' => FmtPart::Min,
                'S' => FmtPart::S,
                'f' => FmtPart::F,
                c => FmtPart::Lit(c),
            };
            parts.push(part);
            i += 2;
        } else {
            parts.push(FmtPart::Lit(chars[i]));
            i += 1;
        }
    }
    parts
}

fn parse_with_parts(s: &str, parts: &[FmtPart]) -> Option<Timestamp> {
    let chars: Vec<char> = s.chars().collect();
    let mut pos = 0;
    let mut day = 0u8;
    let mut month = 0u8;
    let mut year = 0u16;
    let mut hour = 0u8;
    let mut minute = 0u8;
    let mut second = 0u8;
    let mut micro = 0u32;

    for part in parts {
        match part {
            FmtPart::Lit(c) => {
                if pos >= chars.len() || chars[pos] != *c {
                    return None;
                }
                pos += 1;
            },
            FmtPart::D => {
                let (val, new_pos) = read_number(&chars, pos, 2)?;
                day = val as u8;
                pos = new_pos;
            },
            FmtPart::M => {
                let (val, new_pos) = read_number(&chars, pos, 2)?;
                month = val as u8;
                pos = new_pos;
            },
            FmtPart::Y => {
                let (val, new_pos) = read_number(&chars, pos, 4)?;
                year = val as u16;
                pos = new_pos;
            },
            FmtPart::Y2 => {
                let (val, new_pos) = read_number(&chars, pos, 2)?;
                // 00-68 → 2000-2068, 69-99 → 1969-1999
                year = if val <= 68 {
                    2000 + val as u16
                } else {
                    1900 + val as u16
                };
                pos = new_pos;
            },
            FmtPart::H => {
                let (val, new_pos) = read_number(&chars, pos, 2)?;
                hour = val as u8;
                pos = new_pos;
            },
            FmtPart::Min => {
                let (val, new_pos) = read_number(&chars, pos, 2)?;
                minute = val as u8;
                pos = new_pos;
            },
            FmtPart::S => {
                let (val, new_pos) = read_number(&chars, pos, 2)?;
                second = val as u8;
                pos = new_pos;
            },
            FmtPart::F => {
                let (val, new_pos) = read_number(&chars, pos, 6)?;
                micro = val;
                pos = new_pos;
            },
        }
    }

    if pos != chars.len() {
        return None;
    }

    Timestamp::new(year, month, day, hour, minute, second, micro).ok()
}

fn read_number(chars: &[char], start: usize, digits: usize) -> Option<(u32, usize)> {
    let mut val = 0u32;
    let mut pos = start;
    for _ in 0..digits {
        if pos >= chars.len() || !chars[pos].is_ascii_digit() {
            return None;
        }
        val = val * 10 + (chars[pos] as u32 - '0' as u32);
        pos += 1;
    }
    Some((val, pos))
}

#[cfg(test)]
mod tests {
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
    fn test_format_cfg() {
        let ts = Timestamp::new(2023, 1, 15, 10, 30, 45, 123456).unwrap();
        assert_eq!(format_cfg(&ts), "01/15/2023,10:30:45.123456");
    }
}
