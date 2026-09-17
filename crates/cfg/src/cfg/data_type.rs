//! CFG 数据格式枚举

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
