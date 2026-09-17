//! 手写 JSON 生成器
//!
//! 生成 Comtrade 的 JSON 文本，结构对齐 Python save_json 输出。

use cbase::time;
use model::Comtrade;

/// 生成 Comtrade 的 JSON 文本
pub fn to_json(ct: &Comtrade, indent: Option<usize>) -> String {
    let mut w = JsonWriter::new(indent);
    w.object_begin();

    // Config 扁平化字段
    let cfg = &ct.config;
    w.key_str("station", &cfg.header.station);
    w.key_str("rec_dev_name", &cfg.header.recorder);
    w.key_str("version", cfg.header.version.as_str());
    w.key("channel_num");
    {
        w.object_begin();
        w.key_num("total", cfg.channels.total as f64);
        w.key_num("analog", cfg.channels.analog as f64);
        w.key_num("status", cfg.channels.status as f64);
        w.object_end();
    }
    w.key_str("data_type", cfg.data_type.as_str());
    w.key_num("timemult", cfg.timemult);
    w.key_str("start_time", &time::format_cfg(&cfg.start_time));
    w.key_str("trigger_time", &time::format_cfg(&cfg.trigger_time));

    // 采样
    w.key("sampling");
    {
        w.object_begin();
        w.key_num("freq", cfg.sampling.freq);
        w.key("segments");
        {
            w.array_begin();
            for seg in &cfg.sampling.segments {
                w.object_begin();
                w.key_num("samp_rate", seg.samp_rate);
                w.key_num("end_point", seg.end_point as f64);
                w.object_end();
            }
            w.array_end();
        }
        w.object_end();
    }

    // 模拟通道
    w.key("analogs");
    {
        w.array_begin();
        for ch in &cfg.analogs {
            w.object_begin();
            w.key_num("index", ch.index as f64);
            w.key_str("name", &ch.name);
            w.key_str("phase", &ch.phase);
            w.key_str("equipment", &ch.equipment);
            w.key_str("unit", &ch.unit);
            w.key_num("multiplier", ch.multiplier);
            w.key_num("offset", ch.offset);
            w.key_num("delay", ch.delay);
            w.key_num("min_value", ch.min_value);
            w.key_num("max_value", ch.max_value);
            w.key_num("primary", ch.primary);
            w.key_num("secondary", ch.secondary);
            w.key_str("tran_side", ch.tran_side.as_str());
            // 数据列
            if let Some(ref dat) = ct.data {
                if let Some(samples) = dat.analog_samples(ch.index - 1) {
                    w.key("data");
                    w.array_begin();
                    for v in samples {
                        w.num(*v);
                    }
                    w.array_end();
                }
            }
            w.object_end();
        }
        w.array_end();
    }

    // 状态通道
    w.key("statuses");
    {
        w.array_begin();
        for ch in &cfg.statuses {
            w.object_begin();
            w.key_num("index", ch.index as f64);
            w.key_str("name", &ch.name);
            w.key_str("phase", &ch.phase);
            w.key_str("equipment", &ch.equipment);
            w.key_num("contact", ch.contact as f64);
            if let Some(ref dat) = ct.data {
                if let Some(samples) = dat.status_samples(ch.index - 1) {
                    w.key("data");
                    w.array_begin();
                    for v in samples {
                        w.num(*v as f64);
                    }
                    w.array_end();
                }
            }
            w.object_end();
        }
        w.array_end();
    }

    w.object_end();
    w.finish()
}

/// 小型 JSON 写入器
struct JsonWriter {
    buf: String,
    indent: Option<usize>,
    depth: usize,
    need_comma: bool,
    #[allow(dead_code)]
    first_in_container: Vec<bool>,
}

impl JsonWriter {
    fn new(indent: Option<usize>) -> Self {
        JsonWriter {
            buf: String::new(),
            indent,
            depth: 0,
            need_comma: false,
            first_in_container: Vec::new(),
        }
    }

    fn finish(self) -> String {
        self.buf
    }

    fn write_indent(&mut self) {
        if let Some(n) = self.indent {
            self.buf.push('\n');
            for _ in 0..self.depth * n {
                self.buf.push(' ');
            }
        }
    }

    fn write_comma(&mut self) {
        if self.need_comma {
            self.buf.push(',');
        } else {
            self.need_comma = true;
        }
    }

    fn key(&mut self, key: &str) {
        self.write_comma();
        self.write_indent();
        self.buf.push('"');
        self.buf.push_str(key);
        self.buf.push_str("\":");
        self.need_comma = false;
    }

    fn key_str(&mut self, key: &str, value: &str) {
        self.key(key);
        self.str(value);
    }

    fn key_num(&mut self, key: &str, value: f64) {
        self.key(key);
        self.num(value);
    }

    fn str(&mut self, value: &str) {
        self.write_comma();
        self.write_indent();
        self.buf.push('"');
        self.buf.push_str(&escape_json(value));
        self.buf.push('"');
    }

    fn num(&mut self, value: f64) {
        self.write_comma();
        self.write_indent();
        if value == value.trunc() && value.is_finite() {
            self.buf.push_str(&format!("{:.0}", value));
        } else {
            self.buf.push_str(&format!("{}", value));
        }
    }

    fn object_begin(&mut self) {
        self.write_comma();
        self.write_indent();
        self.buf.push('{');
        self.depth += 1;
        self.need_comma = false;
    }

    fn object_end(&mut self) {
        self.depth -= 1;
        self.write_indent();
        self.buf.push('}');
        self.need_comma = true;
    }

    fn array_begin(&mut self) {
        self.write_comma();
        self.write_indent();
        self.buf.push('[');
        self.depth += 1;
        self.need_comma = false;
    }

    fn array_end(&mut self) {
        self.depth -= 1;
        self.write_indent();
        self.buf.push(']');
        self.need_comma = true;
    }
}

/// JSON 字符串转义
fn escape_json(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            '\x08' => result.push_str("\\b"),
            '\x0C' => result.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                result.push_str(&format!("\\u{:04x}", c as u32));
            },
            _ => result.push(c),
        }
    }
    result
}
