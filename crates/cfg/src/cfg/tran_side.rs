//! CFG 转换标识枚举（一次侧/二次侧）

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
