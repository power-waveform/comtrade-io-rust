//! CFG 配置模块：通道模型定义

use super::tran_side::TranSide;

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
        let monitored_component = if self.equipment.trim().is_empty() {
            self.ext
                .as_ref()
                .and_then(|ext| ext.reference.as_deref())
                .unwrap_or("")
        } else {
            &self.equipment
        };
        format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{}",
            self.index,
            self.name,
            self.phase,
            monitored_component,
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
        let monitored_component = if self.equipment.trim().is_empty() {
            self.ext
                .as_ref()
                .and_then(|ext| ext.reference.as_deref())
                .unwrap_or("")
        } else {
            &self.equipment
        };
        format!(
            "{},{},{},{},{}",
            self.index, self.name, self.phase, monitored_component, self.contact,
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
