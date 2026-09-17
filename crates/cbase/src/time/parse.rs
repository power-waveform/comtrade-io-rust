//! 时间字符串解析（15 种格式族）

use crate::error::{Error, Result};
use crate::time::Timestamp;

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
    // 只处理时间部分（逗号之后）的微秒：取第一个点后的数字段补齐 6 位，多余丢弃。
    // 对齐 Python `format_time`（`parts[1].ljust(6,"0")[:6]`），但仅限逗号后的点，
    // 避免误伤点分隔日期（如 `07.04.13,...` 日期部分的点）。
    let comma_pos = s.find(',');
    let search_from = comma_pos.map(|c| c + 1).unwrap_or(0);
    if let Some(rel_dot) = s[search_from..].find('.') {
        let dot_pos = search_from + rel_dot;
        let after_dot = &s[dot_pos + 1..];
        let digits: String = after_dot
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        if digits.len() < 6 {
            let padded = format!("{:0<6}", digits);
            format!("{}.{}", &s[..dot_pos], &padded[..6])
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
///
/// 对齐 Python 基线 `data_time_parser.py`，并补充实际数据中出现过的
/// 点分隔日期（如 `07.04.13`）。`%m/%d/%H/%M/%S` 按可变 1-2 位读取，
/// 与 Python `datetime.strptime` 行为一致。
const TIME_FORMATS: &[&str] = &[
    "%m/%d/%Y,%H:%M:%S.%f", // 四位年，美国格式（月/日/年）——优先，与 format_cfg 一致
    "%d/%m/%Y,%H:%M:%S.%f", // 四位年，欧洲格式（日/月/年）
    "%m/%d/%y,%H:%M:%S.%f", // 两位年，美国格式（月/日/年）
    "%d/%m/%y,%H:%M:%S.%f", // 两位年，欧洲格式（日/月/年）
    "%m/%d/%Y, %H:%M:%S.%f", // 四位年，美国格式，带空格
    "%d/%m/%Y, %H:%M:%S.%f", // 四位年，欧洲格式，带空格
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
    "%m.%d.%y,%H:%M:%S.%f", // 点分隔日期，两位年，美国格式
    "%d.%m.%y,%H:%M:%S.%f", // 点分隔日期，两位年，欧洲格式
    "%m.%d.%Y,%H:%M:%S.%f", // 点分隔日期，四位年，美国格式
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
                let (val, new_pos) = read_fixed(&chars, pos, 4)?;
                year = val as u16;
                pos = new_pos;
            },
            FmtPart::Y2 => {
                let (val, new_pos) = read_fixed(&chars, pos, 2)?;
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
    // 读取 1..=digits 位数字（对齐 Python strptime：月/日/时/分/秒允许 1-2 位）
    let mut val = 0u32;
    let mut pos = start;
    let mut n = 0;
    while n < digits && pos < chars.len() && chars[pos].is_ascii_digit() {
        val = val * 10 + (chars[pos] as u32 - '0' as u32);
        pos += 1;
        n += 1;
    }
    if n == 0 {
        None
    } else {
        Some((val, pos))
    }
}

/// 读取恰好 `digits` 位数字（%Y/%y 年份固定位数，避免把两位年误读成四位年）
fn read_fixed(chars: &[char], start: usize, digits: usize) -> Option<(u32, usize)> {
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
