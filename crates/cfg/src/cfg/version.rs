//! CFG 标准版本号枚举

/// 版本枚举
///
/// 六个版本号对应的标准（与 Python 基线 `model/type/version.py` 一致）：
///
/// | 版本 | 标准 |
/// |---|---|
/// | 1991 | IEEE Std C37.111-1991 |
/// | 1999 | IEEE Std C37.111-1999 |
/// | 2001 | IEC 60255-24:2001 |
/// | 2008 | GB/T 22386-2008 |
/// | 2013 | IEC 60255-24:2013 |
/// | 2017 | GB/T 14598.24-2017 |
///
/// 1991 是唯一没有 CFG 可选尾部字段（`timemult` / 时间码 / 采样时间品质）的版本，
/// 因此"是否 1999 及以后"是实际影响解析与序列化的判据，用 [`Version::is_1999_or_later`]。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Version {
    /// IEEE Std C37.111-1991
    #[default]
    V1991,
    /// IEEE Std C37.111-1999
    V1999,
    /// IEC 60255-24:2001
    V2001,
    /// GB/T 22386-2008
    V2008,
    /// IEC 60255-24:2013
    V2013,
    /// GB/T 14598.24-2017
    V2017,
}

impl Version {
    /// 版本号字符串（如 `"1999"`）
    pub fn as_str(&self) -> &'static str {
        match self {
            Version::V1991 => "1991",
            Version::V1999 => "1999",
            Version::V2001 => "2001",
            Version::V2008 => "2008",
            Version::V2013 => "2013",
            Version::V2017 => "2017",
        }
    }

    /// 该版本号对应的标准名称
    pub fn standard(&self) -> &'static str {
        match self {
            Version::V1991 => "IEEE Std C37.111-1991",
            Version::V1999 => "IEEE Std C37.111-1999",
            Version::V2001 => "IEC 60255-24:2001",
            Version::V2008 => "GB/T 22386-2008",
            Version::V2013 => "IEC 60255-24:2013",
            Version::V2017 => "GB/T 14598.24-2017",
        }
    }

    /// 解析版本号字符串，无法识别时返回 `None`。
    ///
    /// 调用方通常用 `.unwrap_or_default()` 回落到 `V1991`——头部缺版本字段
    /// 或版本号无法识别时按最早版本处理（对齐 Python `Version.from_value(x, V1991)`）。
    pub fn parse(s: &str) -> Option<Version> {
        match s.trim() {
            "1991" => Some(Version::V1991),
            "1999" => Some(Version::V1999),
            "2001" => Some(Version::V2001),
            "2008" => Some(Version::V2008),
            "2013" => Some(Version::V2013),
            "2017" => Some(Version::V2017),
            _ => None,
        }
    }

    /// 是否为 1999 及以后的版本。
    ///
    /// 1991 版 CFG 没有 `timemult` 及其后的可选行，1999 起才引入；
    /// 2001/2008/2013/2017 均在 1999 的基础上扩展，可选字段布局与 1999 兼容。
    /// 所有按版本分支的解析/序列化逻辑都应判断这个，而不是直接比 `== V1999`，
    /// 否则 2001 及以后的文件会被当成 1991 处理。
    pub fn is_1999_or_later(&self) -> bool {
        !matches!(self, Version::V1991)
    }
}
