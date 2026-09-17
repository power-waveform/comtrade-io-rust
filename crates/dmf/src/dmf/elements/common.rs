//! 公共属性解析辅助（各设备元素共享）

use crate::dmf::{DmfAnalogChannel, DmfStatusChannel};
use cbase::equipment::{AccBran, AcvChn};

/// 属性查表（大小写敏感，与 XML 规范一致）
pub fn attr<'a>(attrs: &'a [(&str, &str)], name: &str) -> Option<&'a str> {
    attrs.iter().find(|(k, _)| *k == name).map(|(_, v)| *v)
}

/// 读取整型属性，缺失/非法/空串均为 0
pub fn attr_usize(attrs: &[(&str, &str)], name: &str) -> usize {
    attr(attrs, name)
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0)
}

/// 读取浮点属性，缺失/非法/空串均为 `default`
pub fn attr_f64(attrs: &[(&str, &str)], name: &str, default: f64) -> f64 {
    attr(attrs, name)
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(default)
}

/// 读取字符串属性
pub fn attr_str(attrs: &[(&str, &str)], name: &str) -> String {
    attr(attrs, name).unwrap_or("").to_string()
}

pub fn parse_acv_attrs(attrs: &[(&str, &str)]) -> AcvChn {
    AcvChn {
        ua_idx: attr_usize(attrs, "ua_idx"),
        ub_idx: attr_usize(attrs, "ub_idx"),
        uc_idx: attr_usize(attrs, "uc_idx"),
        un_idx: attr_usize(attrs, "un_idx"),
        ul_idx: attr_usize(attrs, "ul_idx"),
    }
}

pub fn parse_acc_attrs(attrs: &[(&str, &str)]) -> AccBran {
    // 偏离 Python 基线：读 bran_idx 写 idx，导致二次解析时 idx 归零。此处读写统一为 bran_idx。
    let dir_raw = attr_str(attrs, "dir");
    AccBran {
        bran_idx: attr_usize(attrs, "bran_idx"),
        ia_idx: attr_usize(attrs, "ia_idx"),
        ib_idx: attr_usize(attrs, "ib_idx"),
        ic_idx: attr_usize(attrs, "ic_idx"),
        in_idx: attr_usize(attrs, "in_idx"),
        dir: parse_dir(&dir_raw),
        dir_raw,
    }
}

/// 归一化方向标志。
///
/// DMF 用 `POS` / `NEG`（样本中大小写不一），INF 用 `1` / `-1`（规范 §1.4）。
/// 缺省为正方向 `1`。
pub fn parse_dir(s: &str) -> i32 {
    let t = s.trim();
    if t.is_empty() {
        return 1;
    }
    if let Ok(v) = t.parse::<i32>() {
        return if v < 0 { -1 } else { 1 };
    }
    match t.to_ascii_lowercase().as_str() {
        "neg" | "negative" | "-" => -1,
        _ => 1,
    }
}

pub fn parse_analog_attrs(attrs: &[(&str, &str)]) -> DmfAnalogChannel {
    let mut ch = DmfAnalogChannel::default();
    let mut has_au = false;
    for (name, value) in attrs {
        match *name {
            "idx_cfg" => ch.idx_cfg = value.parse().unwrap_or(0),
            "idx_org" => ch.idx_org = value.parse().unwrap_or(0),
            "type" => ch.ch_type = value.to_string(),
            "flag" => ch.flag = value.to_string(),
            "freq" => ch.freq = value.parse().unwrap_or(50.0),
            "au" => {
                has_au = true;
                ch.au = value.parse().unwrap_or(0.0)
            },
            "bu" => ch.bu = value.parse().unwrap_or(0.0),
            "sIUnit" => ch.unit = value.to_string(),
            "multiplier" => ch.multiplier = value.parse().unwrap_or(1.0),
            "primary" => ch.primary = value.parse().unwrap_or(1.0),
            "secondary" => ch.secondary = value.parse().unwrap_or(1.0),
            "ps" => ch.ps = value.to_string(),
            // 兼容早期 Python 写出的 idx_rl 与当前样本使用的 idx_rlt。
            "idx_rlt" | "idx_rl" => ch.idx_rlt = value.parse().unwrap_or(0),
            "ph" => ch.ph = value.to_string(),
            _ => {},
        }
    }
    // DMF 早期版本可能省略 au。交流模拟量的标准默认值为 1；
    // 显式写入（包括显式 0）的值必须保留。
    if !has_au && is_ac_channel_type(&ch.ch_type) {
        ch.au = 1.0;
    }
    ch
}

pub fn is_ac_channel_type(kind: &str) -> bool {
    matches!(
        kind.trim().to_ascii_lowercase().as_str(),
        "a" | "ac" | "acv" | "acc" | "acvoltage" | "ac current" | "acccurrent" | "accurrent"
    )
}

pub fn parse_status_attrs(attrs: &[(&str, &str)]) -> DmfStatusChannel {
    let mut ch = DmfStatusChannel::default();
    for (name, value) in attrs {
        match *name {
            "idx_cfg" => ch.idx_cfg = value.parse().unwrap_or(0),
            "idx_org" => ch.idx_org = value.parse().unwrap_or(0),
            "type" => ch.ch_type = value.to_string(),
            "flag" => ch.flag = value.to_string(),
            "contact" => ch.contact = value.to_string(),
            "srcRef" => ch.src_ref = value.to_string(),
            _ => {},
        }
    }
    ch
}
