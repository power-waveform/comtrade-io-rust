//! CFG 配置模块：通道模型定义

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

/// 数据格式枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DataType {
    /// ASCII 文本格式
    #[default]
    Ascii,
    /// 二进制格式（16 位）
    Binary,
    /// 二进制格式（32 位）
    Binary32,
    /// 浮点格式（32 位，工程值直存）
    Float32,
}

impl DataType {
    /// 解析数据格式字符串（大小写不敏感），无法识别时返回 `None`
    pub fn parse(s: &str) -> Option<DataType> {
        match s.trim().to_uppercase().as_str() {
            "ASCII" => Some(DataType::Ascii),
            "BINARY" => Some(DataType::Binary),
            "BINARY32" => Some(DataType::Binary32),
            "FLOAT32" => Some(DataType::Float32),
            _ => None,
        }
    }

    /// 数据格式的 CFG 标准字符串（`ASCII` / `BINARY` / `BINARY32` / `FLOAT32`）
    pub fn as_str(&self) -> &'static str {
        match self {
            DataType::Ascii => "ASCII",
            DataType::Binary => "BINARY",
            DataType::Binary32 => "BINARY32",
            DataType::Float32 => "FLOAT32",
        }
    }

    /// 模拟量是否 32 位存储
    pub fn is_32bit(&self) -> bool {
        matches!(self, DataType::Binary32 | DataType::Float32)
    }
}

/// 转换标识
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TranSide {
    /// 二次侧（Secondary）
    #[default]
    S,
    /// 一次侧（Primary）
    P,
}

impl TranSide {
    /// 解析转换标识，`P` → 一次侧，其余（含缺省）→ 二次侧
    pub fn parse(s: &str) -> TranSide {
        match s.trim().to_uppercase().as_str() {
            "P" => TranSide::P,
            _ => TranSide::S,
        }
    }

    /// 转换标识的 CFG 标准字符串（`P` / `S`）
    pub fn as_str(&self) -> &'static str {
        match self {
            TranSide::P => "P",
            TranSide::S => "S",
        }
    }
}

/// 模拟量通道扩展字段（来自 INF/DMF 合并）
#[derive(Debug, Clone, Default)]
pub struct AnalogExt {
    /// 端子排号，通道在端子排上的原始位置（INF/DMF `idx_org`）
    pub idx_org: Option<usize>,
    /// 模拟量频率（Hz），默认 50
    pub freq: Option<f64>,
    /// 模拟量标幺系数 A（INF `Analog_Channels_Parameter` 第 10 字段）
    pub au: Option<f64>,
    /// 模拟量标幺系数 B（INF `Analog_Channels_Parameter` 第 11 字段）
    pub bu: Option<f64>,
    /// 通道类型（DMF `type`）
    pub channel_type: Option<String>,
    /// 通道标识（DMF/INF `flag`）
    pub flag: Option<String>,
    /// IEC 61850 参引
    pub reference: Option<String>,
}

/// 模拟量通道模型
#[derive(Debug, Clone)]
pub struct AnalogChannel {
    // —— CFG 原生 13 字段 ——
    /// 通道编号（1 基）
    pub index: usize,
    /// 通道名（对应变电站一次设备的标识符）
    pub name: String,
    /// 相别标识（`A` / `B` / `C` / `N` 等）
    pub phase: String,
    /// 被监视的电路元件
    pub equipment: String,
    /// 单位（如 `V` / `A` / `W` / `Hz`）
    pub unit: String,
    /// 增益系数（`value = raw * multiplier + offset`）
    pub multiplier: f64,
    /// 偏移量
    pub offset: f64,
    /// 通道时滞（μs）
    pub delay: f64,
    /// 数值最小值（量程下限）
    pub min_value: f64,
    /// 数值最大值（量程上限）
    pub max_value: f64,
    /// 互感器一次系数
    pub primary: f64,
    /// 互感器二次系数
    pub secondary: f64,
    /// 转换标识（P/S）
    pub tran_side: TranSide,
    // —— INF/DMF 合并的扩展字段 ——
    /// INF/DMF 合并的扩展字段（`None` 表示未提供）
    pub ext: Option<AnalogExt>,
}

impl Default for AnalogChannel {
    fn default() -> Self {
        Self {
            index: 0,
            name: String::new(),
            phase: String::new(),
            equipment: String::new(),
            unit: String::new(),
            multiplier: 1.0,
            offset: 0.0,
            delay: 0.0,
            min_value: 0.0,
            max_value: 0.0,
            primary: 1.0,
            secondary: 1.0,
            tran_side: TranSide::S,
            ext: None,
        }
    }
}

impl AnalogChannel {
    /// 从 CFG 行解析（13 字段，缺失尾部字段取默认值）
    pub fn from_cfg_line(line: &str) -> AnalogChannel {
        let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
        let index = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
        let name = parts.get(1).unwrap_or(&"").to_string();
        let phase = parts.get(2).unwrap_or(&"").to_string();
        let equipment = parts.get(3).unwrap_or(&"").to_string();
        let unit = parts.get(4).unwrap_or(&"").to_string();
        let multiplier = parse_f64_or(parts.get(5).unwrap_or(&""), 1.0);
        let offset = parse_f64_or(parts.get(6).unwrap_or(&""), 0.0);
        let delay = parse_f64_or(parts.get(7).unwrap_or(&""), 0.0);
        let min_value = parse_f64_or(parts.get(8).unwrap_or(&""), 0.0);
        let max_value = parse_f64_or(parts.get(9).unwrap_or(&""), 0.0);
        let primary = parse_f64_or(parts.get(10).unwrap_or(&""), 1.0);
        let secondary = parse_f64_or(parts.get(11).unwrap_or(&""), 1.0);
        let tran_side = TranSide::parse(parts.get(12).unwrap_or(&"S"));

        AnalogChannel {
            index,
            name,
            phase,
            equipment,
            unit,
            multiplier,
            offset,
            delay,
            min_value,
            max_value,
            primary,
            secondary,
            tran_side,
            ext: None,
        }
    }

    /// 序列化为 CFG 行
    pub fn to_cfg_line(&self) -> String {
        format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{}",
            self.index,
            self.name,
            self.phase,
            self.equipment,
            self.unit,
            fmt_float(self.multiplier),
            fmt_float(self.offset),
            fmt_float(self.delay),
            fmt_float(self.min_value),
            fmt_float(self.max_value),
            fmt_float(self.primary),
            fmt_float(self.secondary),
            self.tran_side.as_str(),
        )
    }
}

/// 状态量通道扩展字段
#[derive(Debug, Clone, Default)]
pub struct StatusExt {
    /// 端子排号，通道在端子排上的原始位置（INF/DMF `idx_org`）
    pub idx_org: Option<usize>,
    /// 通道类型（DMF `type`）
    pub channel_type: Option<String>,
    /// 通道标识（DMF/INF `flag`）
    pub flag: Option<String>,
    /// 状态通道正常状态（`0` = 常开，`1` = 常闭）
    pub contact: Option<u8>,
    /// IEC 61850 参引
    pub reference: Option<String>,
    /// 保护/断路器/刀闸序号（INF `equipment_no`），如 `Relay_#1`、`Breaker_#1`
    pub equipment_no: Option<String>,
}

/// 状态量通道模型
#[derive(Debug, Clone, Default)]
pub struct StatusChannel {
    /// 通道编号（1 基）
    pub index: usize,
    /// 通道名
    pub name: String,
    /// 相别标识
    pub phase: String,
    /// 被监视的电路元件
    pub equipment: String,
    /// 正常状态（0=常开, 1=常闭）
    pub contact: u8, // 0=常开, 1=常闭
    /// INF/DMF 合并的扩展字段（`None` 表示未提供）
    pub ext: Option<StatusExt>,
}

impl StatusChannel {
    /// 从 CFG 行解析（5 字段，缺失尾部字段取默认值）
    pub fn from_cfg_line(line: &str) -> StatusChannel {
        let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
        let index = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
        let name = parts.get(1).unwrap_or(&"").to_string();
        let phase = parts.get(2).unwrap_or(&"").to_string();
        let equipment = parts.get(3).unwrap_or(&"").to_string();
        let contact = parts.get(4).and_then(|s| s.parse().ok()).unwrap_or(0);

        StatusChannel {
            index,
            name,
            phase,
            equipment,
            contact,
            ext: None,
        }
    }

    /// 序列化为 CFG 行
    pub fn to_cfg_line(&self) -> String {
        format!(
            "{},{},{},{},{}",
            self.index, self.name, self.phase, self.equipment, self.contact,
        )
    }
}

/// 解析浮点数，非法返回默认值
pub fn parse_f64_or(s: &str, default: f64) -> f64 {
    if s.is_empty() {
        return default;
    }
    s.parse().unwrap_or(default)
}

/// 浮点格式化：整值不显示小数点
fn fmt_float(v: f64) -> String {
    if v == v.trunc() && v.is_finite() {
        format!("{:.0}", v)
    } else {
        format!("{}", v)
    }
}
