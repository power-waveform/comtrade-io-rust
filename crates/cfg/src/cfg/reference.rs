//! IEC 61850 源参引识别辅助函数

/// 判断部分录波装置把 IEC 61850 参引误写入 CFG `ccbm` 字段的兼容格式。
///
/// `ccbm` 应表示被监视元件，但少数装置会写入类似
/// `MUSV$TVTR1$MX$Vol` 或 `SVOUTMUSV$TCTR1$MX$AmpR` 的源引用。
/// 这类值不能用于设备归组，应转存到扩展参引字段。
pub fn is_reference_like_ccbm(value: &str) -> bool {
    let normalized = value.trim().to_ascii_uppercase();
    (normalized.starts_with("MUSV") || normalized.starts_with("SVOUT")) && normalized.contains('$')
}

/// 判断 CCBM/Monitored_Component 是否是 IEC 61850 源参引。
///
/// 一些录波装置会把 `PTRC$ST$Tr$general`、`TCTR$MX$Amp$` 等源参引
/// 直接写入 CFG 的 ccbm 字段。该字段按标准本应是被监视元件名称，
/// 因此解析时应将此类值保存到扩展 `reference`，不能拿来做设备归组。
/// 这里采用保守的结构判断：至少包含两个 `$` 分隔符且每段有内容；
/// 普通设备名称（即使包含单个 `$`）不会被误判。
pub fn is_iec61850_reference(value: &str) -> bool {
    let value = value.trim();
    if value.is_empty() {
        return false;
    }
    let parts: Vec<_> = value.split('$').collect();
    // 末尾 `$` 是 IEC 参引的常见写法（例如 `TCTR$MX$Amp$`），
    // 因此只要求前三段非空，后续空段允许存在。
    parts.len() >= 3 && parts.iter().take(3).all(|part| !part.trim().is_empty())
}
