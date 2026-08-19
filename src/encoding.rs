//! 编码探测与转码模块
//!
//! 支持 UTF-8（含 BOM 剥离）、GBK、cp1251 编码。

use std::path::Path;

use crate::error::{Error, Result};

/// 文本编码类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    /// UTF-8（含 BOM 剥离）
    Utf8,
    /// GBK（中文编码）
    Gbk,
    /// cp1251（西里尔，DFR WNDR 头专用）
    Cp1251,
}

/// 探测字节序列的编码（UTF-8 严格校验优先，失败回退 GBK）
pub fn detect(bytes: &[u8]) -> Encoding {
    // 剥离 BOM
    let data = strip_bom(bytes);
    if std::str::from_utf8(data).is_ok() {
        Encoding::Utf8
    } else {
        Encoding::Gbk
    }
}

/// 剥离 UTF-8 BOM（如果存在）
fn strip_bom(bytes: &[u8]) -> &[u8] {
    if bytes.len() >= 3 && bytes[0] == 0xEF && bytes[1] == 0xBB && bytes[2] == 0xBF {
        &bytes[3..]
    } else {
        bytes
    }
}

/// 自动探测并解码为 String
pub fn decode(bytes: &[u8]) -> String {
    let enc = detect(bytes);
    decode_with(bytes, enc)
}

/// 按指定编码解码
pub fn decode_with(bytes: &[u8], enc: Encoding) -> String {
    match enc {
        Encoding::Utf8 => {
            let data = strip_bom(bytes);
            String::from_utf8_lossy(data).into_owned()
        },
        Encoding::Gbk => decode_gbk(bytes),
        Encoding::Cp1251 => decode_cp1251(bytes),
    }
}

/// 编码为字节
pub fn encode(text: &str, enc: Encoding) -> Vec<u8> {
    match enc {
        Encoding::Utf8 => text.as_bytes().to_vec(),
        Encoding::Gbk => encode_gbk(text),
        Encoding::Cp1251 => {
            // cp1251 不支持编码回写（DFR 只读）
            text.as_bytes().to_vec()
        },
    }
}

/// 读取文本文件（自动探测编码）
#[allow(dead_code)]
pub fn read_text(path: &Path) -> Result<(String, Encoding)> {
    let bytes = std::fs::read(path).map_err(|e| Error::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    let enc = detect(&bytes);
    let text = decode_with(&bytes, enc);
    Ok((text, enc))
}

/// 读取文本文件，自动探测 UTF-8 / GBK。
///
/// 历史上该入口按 GBK 优先，UTF-8 中文文件会被误解成合法但错误的 GBK 文本。
/// 现在与 [`decode`] 保持一致：UTF-8 严格校验优先，失败再回退 GBK。
pub fn read_text_gbk(path: &Path) -> Result<(String, Encoding)> {
    let bytes = std::fs::read(path).map_err(|e| Error::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    let enc = detect(&bytes);
    let text = decode_with(&bytes, enc);
    Ok((text, enc))
}

/// 写出文本文件（指定编码）
pub fn write_text(path: &Path, text: &str, enc: Encoding) -> Result<()> {
    let bytes = encode(text, enc);
    std::fs::write(path, bytes).map_err(|e| Error::Io {
        path: path.to_path_buf(),
        source: e,
    })
}

// ========== GBK 编解码（使用 encoding_rs） ==========

#[cfg(not(feature = "gbk-builtin"))]
fn decode_gbk(bytes: &[u8]) -> String {
    use encoding_rs::GBK;
    GBK.decode_without_bom_handling(bytes).0.into_owned()
}

#[cfg(not(feature = "gbk-builtin"))]
fn encode_gbk(text: &str) -> Vec<u8> {
    use encoding_rs::GBK;
    GBK.encode(text).0.into_owned()
}

#[cfg(feature = "gbk-builtin")]
fn decode_gbk(bytes: &[u8]) -> String {
    gbk_builtin::decode(bytes)
}

#[cfg(feature = "gbk-builtin")]
fn encode_gbk(text: &str) -> Vec<u8> {
    gbk_builtin::encode(text)
}

#[cfg(feature = "gbk-builtin")]
mod gbk_builtin {
    /// 内置 GBK 解码（精简实现，未映射字符用 U+FFFD）
    pub fn decode(bytes: &[u8]) -> String {
        let mut result = String::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            let b = bytes[i];
            if b < 0x80 {
                result.push(b as char);
                i += 1;
            } else if (0x81..=0xFE).contains(&b) && i + 1 < bytes.len() {
                let b2 = bytes[i + 1];
                if (0x40..=0x7E).contains(&b2) || (0x80..=0xFE).contains(&b2) {
                    let code = ((b as u32) << 8) | (b2 as u32);
                    result.push(gbk_to_unicode(code));
                    i += 2;
                } else {
                    result.push('\u{FFFD}');
                    i += 1;
                }
            } else {
                result.push('\u{FFFD}');
                i += 1;
            }
        }
        result
    }

    pub fn encode(_text: &str) -> Vec<u8> {
        // 精简版：仅 ASCII 区间直通，其余返回原始字节
        // 完整版需要 GBK 码表，约 2.3 万条目
        let mut result = Vec::new();
        for ch in _text.chars() {
            let c = ch as u32;
            if c < 0x80 {
                result.push(c as u8);
            } else if let Some((hi, lo)) = unicode_to_gbk(c) {
                result.push(hi);
                result.push(lo);
            } else {
                result.push(b'?');
            }
        }
        result
    }

    fn gbk_to_unicode(code: u32) -> char {
        // 精简映射表：仅覆盖常用 CJK 区间
        match code {
            // GBK/2 区：GB2312 汉字
            0xB0A1..=0xD7F9 => {
                // 简化实现：U+FFFD 表示未映射
                // 实际需要完整码表
                '\u{FFFD}'
            },
            0xD8A1..=0xF7FE => '\u{FFFD}',
            // GBK/3 区
            0x8140..=0xA0FE => '\u{FFFD}',
            // GBK/4 区
            0xAA40..=0xFEA0 => '\u{FFFD}',
            _ => '\u{FFFD}',
        }
    }

    fn unicode_to_gbk(_cp: u32) -> Option<(u8, u8)> {
        None
    }
}

// ========== cp1251 解码（西里尔，DFR WNDR 头专用） ==========

/// cp1251 → Unicode 映射表（仅非 ASCII 部分，0x80-0xFF）
const CP1251_TABLE: [char; 128] = [
    '\u{0402}', '\u{0403}', '\u{201A}', '\u{0453}', '\u{201E}', '\u{2026}', '\u{2020}', '\u{2021}',
    '\u{20AC}', '\u{2030}', '\u{0409}', '\u{2039}', '\u{040A}', '\u{040C}', '\u{040B}', '\u{040F}',
    '\u{0452}', '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}', '\u{2022}', '\u{2013}', '\u{2014}',
    '\u{FFFD}', '\u{2122}', '\u{0459}', '\u{203A}', '\u{045A}', '\u{045C}', '\u{045B}', '\u{045F}',
    '\u{00A0}', '\u{040E}', '\u{045E}', '\u{0408}', '\u{00A4}', '\u{0490}', '\u{00A6}', '\u{00A7}',
    '\u{0401}', '\u{00A9}', '\u{0404}', '\u{00AB}', '\u{00AC}', '\u{00AD}', '\u{00AE}', '\u{0407}',
    '\u{00B0}', '\u{00B1}', '\u{0406}', '\u{0456}', '\u{0491}', '\u{00B5}', '\u{00B6}', '\u{00B7}',
    '\u{0451}', '\u{2116}', '\u{0454}', '\u{00BB}', '\u{0458}', '\u{0405}', '\u{0455}', '\u{0457}',
    '\u{0410}', '\u{0411}', '\u{0412}', '\u{0413}', '\u{0414}', '\u{0415}', '\u{0416}', '\u{0417}',
    '\u{0418}', '\u{0419}', '\u{041A}', '\u{041B}', '\u{041C}', '\u{041D}', '\u{041E}', '\u{041F}',
    '\u{0420}', '\u{0421}', '\u{0422}', '\u{0423}', '\u{0424}', '\u{0425}', '\u{0426}', '\u{0427}',
    '\u{0428}', '\u{0429}', '\u{042A}', '\u{042B}', '\u{042C}', '\u{042D}', '\u{042E}', '\u{042F}',
    '\u{0430}', '\u{0431}', '\u{0432}', '\u{0433}', '\u{0434}', '\u{0435}', '\u{0436}', '\u{0437}',
    '\u{0438}', '\u{0439}', '\u{043A}', '\u{043B}', '\u{043C}', '\u{043D}', '\u{043E}', '\u{043F}',
    '\u{0440}', '\u{0441}', '\u{0442}', '\u{0443}', '\u{0444}', '\u{0445}', '\u{0446}', '\u{0447}',
    '\u{0448}', '\u{0449}', '\u{044A}', '\u{044B}', '\u{044C}', '\u{044D}', '\u{044E}', '\u{044F}',
];

fn decode_cp1251(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| {
            if b < 0x80 {
                b as char
            } else {
                CP1251_TABLE[(b - 0x80) as usize]
            }
        })
        .collect()
}
