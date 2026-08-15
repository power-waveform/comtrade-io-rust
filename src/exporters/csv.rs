//! CSV 生成器

use crate::Comtrade;

/// 生成 CSV 文本
pub fn to_csv(ct: &Comtrade, headers: bool) -> String {
    let mut output = String::new();
    let cfg = &ct.config;

    if headers {
        let mut header_parts = vec!["Point".to_string(), "Time".to_string()];
        for ch in &cfg.analogs {
            let name = if ch.name.is_empty() {
                format!("A{}", ch.index)
            } else {
                ch.name.clone()
            };
            header_parts.push(csv_escape(&name));
        }
        for ch in &cfg.statuses {
            let name = if ch.name.is_empty() {
                format!("D{}", ch.index)
            } else {
                ch.name.clone()
            };
            header_parts.push(csv_escape(&name));
        }
        output.push_str(&header_parts.join(","));
        output.push('\n');
    }

    if let Some(ref dat) = ct.data {
        for row in 0..dat.len() {
            let mut parts = Vec::new();
            // Point
            parts.push(format!("{}", dat.sample_index[row]));
            // Time
            parts.push(format!("{}", dat.timestamp_us[row]));

            // 模拟量
            for col in &dat.analogs {
                parts.push(format!("{}", col[row]));
            }

            // 状态量
            for col in &dat.statuses {
                parts.push(format!("{}", col[row]));
            }

            output.push_str(&parts.join(","));
            output.push('\n');
        }
    }

    output
}

/// CSV 字段转义（RFC4180）
fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}
