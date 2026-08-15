//! 轻量 XML 拉式解析器（面向 DMF 场景）
//!
//! 支持：XML 声明跳过、注释/CDATA 跳过、开始/自闭合/结束标签、
//! 属性（单/双引号）、实体解码。标签名返回本地名（去前缀）。

use crate::equipment::{
    AccBran, AcvChn, BranchNum, Capacitance, Exciter, Generator, Igap, Impedance, MutualInductance,
    Px, SyncReactance, Ufe, UfeChns, UnChns,
};
use crate::error::{Error, Result};

/// XML 事件
#[derive(Debug, Clone)]
#[allow(dead_code)]
enum XmlEvent<'a> {
    Start {
        name: &'a str,
        attrs: Vec<(&'a str, &'a str)>,
        empty: bool,
    },
    End {
        name: &'a str,
    },
    Text(&'a str),
    Eof,
}

struct XmlReader<'a> {
    input: &'a [u8],
    pos: usize,
}

impl<'a> XmlReader<'a> {
    fn new(input: &'a [u8]) -> Self {
        XmlReader { input, pos: 0 }
    }

    fn next_event(&mut self) -> Result<XmlEvent<'a>> {
        loop {
            self.skip_whitespace();
            if self.pos >= self.input.len() {
                return Ok(XmlEvent::Eof);
            }

            if self.input[self.pos] == b'<' {
                self.pos += 1;
                if self.pos >= self.input.len() {
                    return Err(Error::Xml {
                        position: self.pos,
                        message: "意外的文件结束".into(),
                    });
                }

                match self.input[self.pos] {
                    b'?' => {
                        // XML 声明
                        self.skip_to(b'>');
                        self.pos += 1;
                        continue;
                    },
                    b'!' => {
                        // 注释或 CDATA
                        self.pos += 1;
                        if self.starts_with(b"--") {
                            // 注释
                            self.pos += 2;
                            self.skip_comment()?;
                        } else if self.starts_with(b"[CDATA[") {
                            // CDATA
                            self.pos += 7;
                            let text = self.read_cdata()?;
                            return Ok(XmlEvent::Text(text));
                        }
                        continue;
                    },
                    b'/' => {
                        // 结束标签
                        self.pos += 1;
                        let name = self.read_name();
                        self.skip_to(b'>');
                        self.pos += 1;
                        return Ok(XmlEvent::End { name });
                    },
                    _ => {
                        // 开始标签或自闭合标签
                        let name = self.read_name();
                        let attrs = self.read_attrs()?;
                        self.skip_whitespace();
                        let empty = self.pos < self.input.len() && self.input[self.pos] == b'/';
                        if empty {
                            self.pos += 1;
                        }
                        if self.pos < self.input.len() && self.input[self.pos] == b'>' {
                            self.pos += 1;
                        }
                        return Ok(XmlEvent::Start { name, attrs, empty });
                    },
                }
            } else {
                // 文本内容
                let start = self.pos;
                while self.pos < self.input.len() && self.input[self.pos] != b'<' {
                    self.pos += 1;
                }
                let text =
                    std::str::from_utf8(&self.input[start..self.pos]).map_err(|_| Error::Xml {
                        position: start,
                        message: "无效的 UTF-8 文本".into(),
                    })?;
                let text = text.trim();
                if !text.is_empty() {
                    return Ok(XmlEvent::Text(text));
                }
                continue;
            }
        }
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.input.len() && self.input[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn skip_to(&mut self, c: u8) {
        while self.pos < self.input.len() && self.input[self.pos] != c {
            self.pos += 1;
        }
    }

    fn skip_comment(&mut self) -> Result<()> {
        while self.pos + 2 < self.input.len() {
            if self.input[self.pos] == b'-'
                && self.input[self.pos + 1] == b'-'
                && self.input[self.pos + 2] == b'>'
            {
                self.pos += 3;
                return Ok(());
            }
            self.pos += 1;
        }
        Err(Error::Xml {
            position: self.pos,
            message: "未闭合的注释".into(),
        })
    }

    fn starts_with(&self, s: &[u8]) -> bool {
        self.input[self.pos..].starts_with(s)
    }

    fn read_cdata(&mut self) -> Result<&'a str> {
        let start = self.pos;
        while self.pos + 2 < self.input.len() {
            if self.input[self.pos] == b']'
                && self.input[self.pos + 1] == b']'
                && self.input[self.pos + 2] == b'>'
            {
                let text =
                    std::str::from_utf8(&self.input[start..self.pos]).map_err(|_| Error::Xml {
                        position: start,
                        message: "无效的 UTF-8 CDATA".into(),
                    })?;
                self.pos += 3;
                return Ok(text);
            }
            self.pos += 1;
        }
        Err(Error::Xml {
            position: start,
            message: "未闭合的 CDATA".into(),
        })
    }

    fn read_name(&mut self) -> &'a str {
        self.skip_whitespace();
        let start = self.pos;
        while self.pos < self.input.len()
            && (self.input[self.pos].is_ascii_alphanumeric()
                || self.input[self.pos] == b'_'
                || self.input[self.pos] == b':'
                || self.input[self.pos] == b'-')
        {
            self.pos += 1;
        }
        let full_name = std::str::from_utf8(&self.input[start..self.pos]).unwrap_or("");
        // 返回本地名（去前缀）
        if let Some(colon_pos) = full_name.rfind(':') {
            &full_name[colon_pos + 1..]
        } else {
            full_name
        }
    }

    fn read_attrs(&mut self) -> Result<Vec<(&'a str, &'a str)>> {
        let mut attrs = Vec::new();
        loop {
            self.skip_whitespace();
            if self.pos >= self.input.len() {
                break;
            }
            let c = self.input[self.pos];
            if c == b'>' || c == b'/' {
                break;
            }
            // 属性名
            let name = self.read_name();
            if name.is_empty() {
                break;
            }
            self.skip_whitespace();
            if self.pos < self.input.len() && self.input[self.pos] == b'=' {
                self.pos += 1;
                self.skip_whitespace();
                let value = self.read_attr_value()?;
                attrs.push((name, value));
            } else {
                break;
            }
        }
        Ok(attrs)
    }

    fn read_attr_value(&mut self) -> Result<&'a str> {
        if self.pos >= self.input.len() {
            return Ok("");
        }
        let quote = self.input[self.pos];
        if quote != b'"' && quote != b'\'' {
            return Ok("");
        }
        self.pos += 1;
        let start = self.pos;
        while self.pos < self.input.len() && self.input[self.pos] != quote {
            self.pos += 1;
        }
        let value = std::str::from_utf8(&self.input[start..self.pos]).map_err(|_| Error::Xml {
            position: start,
            message: "无效的 UTF-8 属性值".into(),
        })?;
        if self.pos < self.input.len() {
            self.pos += 1; // skip closing quote
        }
        Ok(value)
    }
}

// ========== DMF 解析 ==========

use super::*;

/// 属性查表（大小写敏感，与 XML 规范一致）
fn attr<'a>(attrs: &'a [(&str, &str)], name: &str) -> Option<&'a str> {
    attrs.iter().find(|(k, _)| *k == name).map(|(_, v)| *v)
}

/// 读取整型属性，缺失/非法/空串均为 0
fn attr_usize(attrs: &[(&str, &str)], name: &str) -> usize {
    attr(attrs, name)
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0)
}

/// 读取浮点属性，缺失/非法/空串均为 `default`
fn attr_f64(attrs: &[(&str, &str)], name: &str, default: f64) -> f64 {
    attr(attrs, name)
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(default)
}

/// 读取字符串属性
fn attr_str(attrs: &[(&str, &str)], name: &str) -> String {
    attr(attrs, name).unwrap_or("").to_string()
}

/// 当前所处的设备容器。
///
/// DMF 的 `ACVChn` / `ACC_Bran` / `RX` 等子元素同时出现在 Bus、Line 与 TransformerWinding 下，
/// 必须按"当前容器"分派挂载，否则会挂错父级（Python 基线用递归收集，无法区分归属）。
enum Container {
    Bus,
    Line,
    Transformer,
    Winding,
    /// 发电机（§1.6）
    Generator,
    /// 励磁机（§1.7）
    Exciter,
    /// 未建模容器（如 `SDL_SysConfig`）：其内部子元素一并忽略，
    /// 避免把 `SDL_*` 内的 `SDL_Limit` 之类误挂到上一个设备上。
    Ignored,
}

pub fn parse_dmf(bytes: &[u8]) -> Result<DmfFile> {
    let mut reader = XmlReader::new(bytes);
    let mut dmf = DmfFile::default();
    let mut current_bus: Option<Bus> = None;
    let mut current_line: Option<Line> = None;
    let mut current_transformer: Option<Transformer> = None;
    let mut current_winding: Option<TransformerWinding> = None;
    let mut current_generator: Option<Generator> = None;
    let mut current_exciter: Option<Exciter> = None;
    // 容器栈：栈顶决定子元素挂载目标
    let mut stack: Vec<Container> = Vec::new();

    loop {
        let event = reader.next_event()?;
        match event {
            XmlEvent::Start { name, attrs, empty } => {
                match name {
                    "ComtradeModel" => {
                        dmf.station_name = attr_str(&attrs, "station_name");
                        dmf.version = attr_str(&attrs, "version");
                        dmf.reference = attr_str(&attrs, "reference");
                        dmf.rec_dev_name = attr_str(&attrs, "rec_dev_name");
                    },
                    "AnalogChannel" => {
                        dmf.analogs.push(parse_analog_attrs(&attrs));
                        if !empty {
                            // 其下的 SDL_* 一律忽略
                            stack.push(Container::Ignored);
                        }
                    },
                    "StatusChannel" => {
                        dmf.statuses.push(parse_status_attrs(&attrs));
                        if !empty {
                            stack.push(Container::Ignored);
                        }
                    },
                    "Bus" => {
                        let bus = parse_bus_attrs(&attrs);
                        if empty {
                            dmf.buses.push(bus);
                        } else {
                            current_bus = Some(bus);
                            stack.push(Container::Bus);
                        }
                    },
                    "Line" => {
                        let line = parse_line_attrs(&attrs);
                        if empty {
                            dmf.lines.push(line);
                        } else {
                            current_line = Some(line);
                            stack.push(Container::Line);
                        }
                    },
                    "Transformer" => {
                        let tr = parse_transformer_attrs(&attrs);
                        if empty {
                            dmf.transformers.push(tr);
                        } else {
                            current_transformer = Some(tr);
                            stack.push(Container::Transformer);
                        }
                    },
                    "TransformerWinding" => {
                        let w = parse_winding_attrs(&attrs);
                        // 偏离 Python 基线：自闭合 <TransformerWinding /> 不产生 End 事件，
                        // 原实现只在 End 分支收尾，导致自闭合绕组被静默丢弃。此处立即收尾。
                        if empty {
                            if let Some(ref mut t) = current_transformer {
                                t.windings.push(w);
                            }
                        } else {
                            current_winding = Some(w);
                            stack.push(Container::Winding);
                        }
                    },
                    "Generator" => {
                        let g = parse_generator_attrs(&attrs);
                        if empty {
                            dmf.generators.push(g);
                        } else {
                            current_generator = Some(g);
                            stack.push(Container::Generator);
                        }
                    },
                    "Exciter" => {
                        let e = parse_exciter_attrs(&attrs);
                        if empty {
                            dmf.exciters.push(e);
                        } else {
                            current_exciter = Some(e);
                            stack.push(Container::Exciter);
                        }
                    },
                    // ===== 子元素：按当前容器分派 =====
                    "ACVChn" => {
                        let acv = parse_acv_attrs(&attrs);
                        match stack.last() {
                            Some(Container::Bus) => {
                                if let Some(ref mut b) = current_bus {
                                    b.acv = acv;
                                }
                            },
                            Some(Container::Line) => {
                                if let Some(ref mut l) = current_line {
                                    l.voltages.push(acv);
                                }
                            },
                            Some(Container::Winding) => {
                                if let Some(ref mut w) = current_winding {
                                    w.acv = acv;
                                }
                            },
                            Some(Container::Generator) => {
                                if let Some(ref mut g) = current_generator {
                                    g.acv = acv;
                                }
                            },
                            Some(Container::Exciter) => {
                                if let Some(ref mut e) = current_exciter {
                                    e.acv = acv;
                                }
                            },
                            _ => {},
                        }
                    },
                    "ACC_Bran" => {
                        let acc = parse_acc_attrs(&attrs);
                        match stack.last() {
                            Some(Container::Line) => {
                                if let Some(ref mut l) = current_line {
                                    l.currents.push(acc);
                                }
                            },
                            Some(Container::Winding) => {
                                if let Some(ref mut w) = current_winding {
                                    w.currents.push(acc);
                                }
                            },
                            Some(Container::Generator) => {
                                if let Some(ref mut g) = current_generator {
                                    // 按 bran_idx 分派到对应 TA 通道组
                                    match acc.bran_idx {
                                        0 | 1 => g.ta = acc,
                                        2 => g.ta_z1 = acc,
                                        3 => g.ta_z2 = acc,
                                        4 => g.ta_z3 = acc,
                                        _ => {},
                                    }
                                }
                            },
                            Some(Container::Exciter) => {
                                if let Some(ref mut e) = current_exciter {
                                    match acc.bran_idx {
                                        0 | 1 => e.ta = acc,
                                        2 => e.ta_z = acc,
                                        _ => {},
                                    }
                                }
                            },
                            _ => {},
                        }
                    },
                    "RX" => {
                        if matches!(stack.last(), Some(Container::Line)) {
                            if let Some(ref mut l) = current_line {
                                l.impedance = Impedance {
                                    r1: attr_f64(&attrs, "r1", 0.0),
                                    x1: attr_f64(&attrs, "x1", 0.0),
                                    r0: attr_f64(&attrs, "r0", 0.0),
                                    x0: attr_f64(&attrs, "x0", 0.0),
                                };
                            }
                        }
                    },
                    "CG" => {
                        if matches!(stack.last(), Some(Container::Line)) {
                            if let Some(ref mut l) = current_line {
                                // DMF 按属性名读取，与 INF 的位置序（C1,G1,C0,G0）无关
                                l.capacitance = Capacitance {
                                    c1: attr_f64(&attrs, "c1", 0.0),
                                    g1: attr_f64(&attrs, "g1", 0.0),
                                    c0: attr_f64(&attrs, "c0", 0.0),
                                    g0: attr_f64(&attrs, "g0", 0.0),
                                };
                            }
                        }
                    },
                    "MR" => {
                        if matches!(stack.last(), Some(Container::Line)) {
                            if let Some(ref mut l) = current_line {
                                l.mutual_inductance = MutualInductance {
                                    idx: attr_usize(&attrs, "idx"),
                                    mr0: attr_f64(&attrs, "mr0", 0.0),
                                    mx0: attr_f64(&attrs, "mx0", 0.0),
                                };
                            }
                        }
                    },
                    "PX" => {
                        if matches!(stack.last(), Some(Container::Line)) {
                            if let Some(ref mut l) = current_line {
                                l.px = Px {
                                    px: attr_f64(&attrs, "px", 0.0),
                                    px0: attr_f64(&attrs, "px0", 0.0),
                                };
                            }
                        }
                    },
                    "DifferentialCurrent" => {
                        if matches!(stack.last(), Some(Container::Line)) {
                            if let Some(ref mut l) = current_line {
                                l.differential_current = parse_acv_attrs(&attrs);
                            }
                        }
                    },
                    "Igap" => {
                        if matches!(stack.last(), Some(Container::Winding)) {
                            if let Some(ref mut w) = current_winding {
                                // 样本使用小写 zgap_idx / zsgap_idx，规范文档写作 zGap_idx，两者都接受
                                w.igap = Igap {
                                    zgap_idx: attr(&attrs, "zgap_idx")
                                        .or_else(|| attr(&attrs, "zGap_idx"))
                                        .and_then(|v| v.trim().parse().ok())
                                        .unwrap_or(0),
                                    zsgap_idx: attr(&attrs, "zsgap_idx")
                                        .or_else(|| attr(&attrs, "zSGap_idx"))
                                        .and_then(|v| v.trim().parse().ok())
                                        .unwrap_or(0),
                                };
                            }
                        }
                    },
                    "AnaChn" => {
                        let idx = attr_usize(&attrs, "idx_cfg");
                        if idx > 0 {
                            match stack.last() {
                                Some(Container::Bus) => {
                                    if let Some(ref mut b) = current_bus {
                                        b.ana_chns.push(idx);
                                    }
                                },
                                Some(Container::Line) => {
                                    if let Some(ref mut l) = current_line {
                                        l.ana_chns.push(idx);
                                    }
                                },
                                Some(Container::Transformer) => {
                                    if let Some(ref mut t) = current_transformer {
                                        t.ana_chns.push(idx);
                                    }
                                },
                                Some(Container::Generator) => {
                                    if let Some(ref mut g) = current_generator {
                                        g.oth_achns.push(idx);
                                    }
                                },
                                Some(Container::Exciter) => {
                                    if let Some(ref mut e) = current_exciter {
                                        e.oth_achns.push(idx);
                                    }
                                },
                                _ => {},
                            }
                        }
                    },
                    "StaChn" => {
                        let idx = attr_usize(&attrs, "idx_cfg");
                        if idx > 0 {
                            match stack.last() {
                                Some(Container::Bus) => {
                                    if let Some(ref mut b) = current_bus {
                                        b.sta_chns.push(idx);
                                    }
                                },
                                Some(Container::Line) => {
                                    if let Some(ref mut l) = current_line {
                                        l.sta_chns.push(idx);
                                    }
                                },
                                Some(Container::Transformer) => {
                                    if let Some(ref mut t) = current_transformer {
                                        t.sta_chns.push(idx);
                                    }
                                },
                                Some(Container::Generator) => {
                                    if let Some(ref mut g) = current_generator {
                                        g.sta_chns.push(idx);
                                    }
                                },
                                Some(Container::Exciter) => {
                                    if let Some(ref mut e) = current_exciter {
                                        e.sta_chns.push(idx);
                                    }
                                },
                                _ => {},
                            }
                        }
                    },
                    // 未建模元素（SDL_* 等）：与 Python 基线一致地丢弃。
                    // 非自闭合的需要压栈，否则其子元素会被误认为属于外层设备。
                    _ => {
                        if !empty {
                            stack.push(Container::Ignored);
                        }
                    },
                }
            },
            XmlEvent::End { name } => {
                match name {
                    "Bus" => {
                        if let Some(b) = current_bus.take() {
                            dmf.buses.push(b);
                        }
                    },
                    "Line" => {
                        if let Some(l) = current_line.take() {
                            dmf.lines.push(l);
                        }
                    },
                    "Transformer" => {
                        if let Some(t) = current_transformer.take() {
                            dmf.transformers.push(t);
                        }
                    },
                    "TransformerWinding" => {
                        if let Some(w) = current_winding.take() {
                            if let Some(ref mut t) = current_transformer {
                                t.windings.push(w);
                            }
                        }
                    },
                    "Generator" => {
                        if let Some(g) = current_generator.take() {
                            dmf.generators.push(g);
                        }
                    },
                    "Exciter" => {
                        if let Some(e) = current_exciter.take() {
                            dmf.exciters.push(e);
                        }
                    },
                    _ => {},
                }
                stack.pop();
            },
            XmlEvent::Text(_) => {},
            XmlEvent::Eof => break,
        }
    }

    Ok(dmf)
}

fn parse_acv_attrs(attrs: &[(&str, &str)]) -> AcvChn {
    AcvChn {
        ua_idx: attr_usize(attrs, "ua_idx"),
        ub_idx: attr_usize(attrs, "ub_idx"),
        uc_idx: attr_usize(attrs, "uc_idx"),
        un_idx: attr_usize(attrs, "un_idx"),
        ul_idx: attr_usize(attrs, "ul_idx"),
    }
}

fn parse_acc_attrs(attrs: &[(&str, &str)]) -> AccBran {
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
fn parse_dir(s: &str) -> i32 {
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

fn parse_analog_attrs(attrs: &[(&str, &str)]) -> DmfAnalogChannel {
    let mut ch = DmfAnalogChannel::default();
    for (name, value) in attrs {
        match *name {
            "idx_cfg" => ch.idx_cfg = value.parse().unwrap_or(0),
            "idx_org" => ch.idx_org = value.parse().unwrap_or(0),
            "type" => ch.ch_type = value.to_string(),
            "flag" => ch.flag = value.to_string(),
            "freq" => ch.freq = value.parse().unwrap_or(50.0),
            "au" => ch.au = value.parse().unwrap_or(0.0),
            "bu" => ch.bu = value.parse().unwrap_or(0.0),
            "sIUnit" => ch.unit = value.to_string(),
            "multiplier" => ch.multiplier = value.parse().unwrap_or(1.0),
            "primary" => ch.primary = value.parse().unwrap_or(1.0),
            "secondary" => ch.secondary = value.parse().unwrap_or(1.0),
            "ps" => ch.ps = value.to_string(),
            "ph" => ch.ph = value.to_string(),
            _ => {},
        }
    }
    ch
}

fn parse_status_attrs(attrs: &[(&str, &str)]) -> DmfStatusChannel {
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

fn parse_bus_attrs(attrs: &[(&str, &str)]) -> Bus {
    Bus {
        idx: attr_usize(attrs, "idx"),
        name: attr_str(attrs, "bus_name"),
        src_ref: attr_str(attrs, "srcRef"),
        sys_id: String::new(),
        v_rtg: attr_f64(attrs, "VRtg", 0.0),
        v_rtg_snd: attr_f64(attrs, "VRtgSnd", 0.0),
        tv_pos: attr_str(attrs, "VRtgSnd_Pos"),
        is_location: attr_str(attrs, "is_location"),
        bus_uuid: attr_str(attrs, "bus_uuid"),
        acv: AcvChn::default(),
        ana_chns: Vec::new(),
        sta_chns: Vec::new(),
    }
}

fn parse_line_attrs(attrs: &[(&str, &str)]) -> Line {
    Line {
        idx: attr_usize(attrs, "idx"),
        name: attr_str(attrs, "line_name"),
        bus_id: attr_str(attrs, "bus_ID"),
        src_ref: attr_str(attrs, "srcRef"),
        sys_id: String::new(),
        object_type: String::new(),
        v_rtg: attr_f64(attrs, "VRtg", 0.0),
        a_rtg: attr_f64(attrs, "ARtg", 0.0),
        a_rtg_snd: attr_f64(attrs, "ARtgSnd", 0.0),
        line_len: attr_f64(attrs, "LinLen", 0.0),
        bran_num: attr_usize(attrs, "bran_num"),
        line_uuid: attr_str(attrs, "line_uuid"),
        remote_id: attr_str(attrs, "remote_ID"),
        remote_flag: attr_str(attrs, "remote_Flag"),
        differential_id: attr_str(attrs, "differential_ID"),
        other_id: String::new(),
        reactor: None,
        impedance: Impedance::default(),
        capacitance: Capacitance::default(),
        mutual_inductance: MutualInductance::default(),
        px: Px::default(),
        currents: Vec::new(),
        voltages: Vec::new(),
        differential_current: AcvChn::default(),
        oth_achns: Vec::new(),
        ana_chns: Vec::new(),
        sta_chns: Vec::new(),
    }
}

fn parse_transformer_attrs(attrs: &[(&str, &str)]) -> Transformer {
    Transformer {
        idx: attr_usize(attrs, "idx"),
        name: attr_str(attrs, "trm_name"),
        src_ref: attr_str(attrs, "srcRef"),
        sys_id: String::new(),
        object_type: String::new(),
        pwr_rtg: attr_f64(attrs, "pwrRtg", 0.0),
        winding_num: 0,
        ta_self_comp: String::new(),
        transformer_uuid: attr_str(attrs, "transformer_uuid"),
        windings: Vec::new(),
        oth_achns: Vec::new(),
        ana_chns: Vec::new(),
        sta_chns: Vec::new(),
    }
}

fn parse_winding_attrs(attrs: &[(&str, &str)]) -> TransformerWinding {
    TransformerWinding {
        location: attr_str(attrs, "location"),
        src_ref: attr_str(attrs, "srcRef"),
        v_rtg: attr_f64(attrs, "VRtg", 0.0),
        a_rtg: attr_f64(attrs, "ARtg", 0.0),
        bran_num: attr_usize(attrs, "bran_num"),
        bus_id: attr_str(attrs, "bus_ID"),
        // 偏离 Python 基线：wG 是绕组接线组别代号（如 y0 / yn0），
        // 原 Rust 实现声明为 f64 并 parse().unwrap_or(0.0)，使全部组别静默归零。
        wg: attr_str(attrs, "wG"),
        acv: AcvChn::default(),
        currents: Vec::new(),
        igap: Igap::default(),
        ta_zs: 0,
        ta_zs_gap: 0,
        sta_chns: Vec::new(),
    }
}

/// 解析发电机属性（§1.6）。DMF 对发电机/励磁机的元素与属性命名规范未定义，
/// 此处按与现有 Bus/Line 风格一致的属性名读取。
fn parse_generator_attrs(attrs: &[(&str, &str)]) -> Generator {
    Generator {
        idx: attr_usize(attrs, "idx"),
        name: attr_str(attrs, "gen_name"),
        src_ref: attr_str(attrs, "srcRef"),
        sys_id: attr_str(attrs, "sys_ID"),
        trm_id: attr_str(attrs, "trm_ID"),
        object_type: attr_str(attrs, "object_type"),
        freq: attr_f64(attrs, "freq", 0.0),
        capacity: attr_f64(attrs, "capacity", 0.0),
        factor: attr_f64(attrs, "factor", 0.0),
        v1: attr_f64(attrs, "V1", 0.0),
        branch_num: BranchNum {
            z1: attr_usize(attrs, "branch_z1"),
            z2: attr_usize(attrs, "branch_z2"),
            z3: attr_usize(attrs, "branch_z3"),
        },
        rotor_i: attr_f64(attrs, "rotor_I", 0.0),
        rotor_v2: attr_f64(attrs, "rotor_V2", 0.0),
        ufe: Ufe {
            rated: attr_f64(attrs, "ufe_rated", 0.0),
            no_load: attr_f64(attrs, "ufe_no_load", 0.0),
        },
        x: SyncReactance {
            xd: attr_f64(attrs, "xd", 0.0),
            xq: attr_f64(attrs, "xq", 0.0),
            xd_prime: attr_f64(attrs, "xd_prime", 0.0),
            xs: attr_f64(attrs, "xs", 0.0),
        },
        excitation_mode: attr(attrs, "excitation_mode")
            .and_then(|v| v.trim().parse::<i32>().ok())
            .unwrap_or(0),
        igt_dir: attr(attrs, "igt_dir")
            .and_then(|v| v.trim().parse::<i32>().ok())
            .unwrap_or(0),
        acv: AcvChn::default(),
        ta: AccBran::default(),
        ta_z1: AccBran::default(),
        ta_z2: AccBran::default(),
        ta_z3: AccBran::default(),
        ufe_chns: UfeChns {
            ufe: attr_usize(attrs, "ufe_chn"),
            pos: attr_usize(attrs, "pos_ufe_chn"),
            neg: attr_usize(attrs, "neg_ufe_chn"),
        },
        ife_chn: attr_usize(attrs, "ife_chn"),
        un_chns: UnChns {
            terminal: attr_usize(attrs, "un_terminal"),
            neutral: attr_usize(attrs, "un_neutral"),
            longitudinal: attr_usize(attrs, "un_longitudinal"),
        },
        ta_ido_chn: attr_usize(attrs, "ta_ido_chn"),
        oth_achns: Vec::new(),
        sta_chns: Vec::new(),
    }
}

/// 解析励磁机属性（§1.7）。
fn parse_exciter_attrs(attrs: &[(&str, &str)]) -> Exciter {
    Exciter {
        idx: attr_usize(attrs, "idx"),
        name: attr_str(attrs, "exc_name"),
        src_ref: attr_str(attrs, "srcRef"),
        sys_id: attr_str(attrs, "sys_ID"),
        pwr_id: attr_str(attrs, "pwr_ID"),
        object_type: attr_str(attrs, "object_type"),
        freq: attr_f64(attrs, "freq", 0.0),
        v1: attr_f64(attrs, "V1", 0.0),
        acv: AcvChn::default(),
        ta: AccBran::default(),
        ta_z: AccBran::default(),
        oth_achns: Vec::new(),
        sta_chns: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_dir_normalization() {
        // DMF 拼写（大小写混杂）
        assert_eq!(parse_dir("POS"), 1);
        assert_eq!(parse_dir("pos"), 1);
        assert_eq!(parse_dir("NEG"), -1);
        assert_eq!(parse_dir("neg"), -1);
        // INF 拼写
        assert_eq!(parse_dir("1"), 1);
        assert_eq!(parse_dir("-1"), -1);
        // 缺省 / 非法值均为正方向
        assert_eq!(parse_dir(""), 1);
        assert_eq!(parse_dir("???"), 1);
    }

    #[test]
    fn test_children_dispatch_by_container() {
        // ACVChn / ACC_Bran 同时出现在 Bus、Line 与 TransformerWinding 下，
        // 必须按所属容器挂载，不能被递归收集串到别的父级上
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<scl:ComtradeModel station_name="S" version="1.0" reference="0" rec_dev_name="R">
    <scl:Bus idx="1" bus_name="B1" VRtg="220.0" VRtgSnd="100.0" VRtgSnd_Pos="BUS">
        <scl:ACVChn ua_idx="1" ub_idx="2" uc_idx="3" un_idx="4" ul_idx="0"/>
        <scl:SDL_Protect a_trip1="99"/>
    </scl:Bus>
    <scl:Line idx="2" line_name="L1" bus_ID="1" VRtg="220.0" ARtg="1250.0" LinLen="10.5" bran_num="1">
        <scl:RX r1="0.1" x1="0.2" r0="0.3" x0="0.4"/>
        <scl:ACC_Bran bran_idx="1" ia_idx="11" ib_idx="12" ic_idx="13" in_idx="14" dir="NEG"/>
        <scl:AnaChn idx_cfg="11"/>
        <scl:StaChn idx_cfg="7"/>
    </scl:Line>
    <scl:Transformer idx="3" trm_name="T1" pwrRtg="180.0">
        <scl:TransformerWinding location="High" VRtg="220.0" ARtg="1.0" bran_num="1" wG="y0">
            <scl:ACVChn ua_idx="21" ub_idx="22" uc_idx="23" un_idx="24"/>
            <scl:ACC_Bran bran_idx="1" ia_idx="31" ib_idx="32" ic_idx="33" in_idx="34" dir="POS"/>
            <scl:Igap zgap_idx="41" zsgap_idx="42"/>
        </scl:TransformerWinding>
        <scl:TransformerWinding location="Low" VRtg="35.0" ARtg="1.0" bran_num="1" wG="yn0"/>
    </scl:Transformer>
</scl:ComtradeModel>"#;
        let dmf = parse_dmf(xml.as_bytes()).unwrap();

        assert_eq!(dmf.buses.len(), 1);
        let b = &dmf.buses[0];
        assert_eq!(b.name, "B1");
        assert_eq!((b.acv.ua_idx, b.acv.un_idx), (1, 4));
        assert_eq!(b.v_rtg, 220.0);
        assert_eq!(b.v_rtg_snd, 100.0);
        assert_eq!(b.tv_pos, "BUS");

        assert_eq!(dmf.lines.len(), 1);
        let l = &dmf.lines[0];
        assert_eq!(l.impedance.r1, 0.1);
        assert_eq!(
            l.currents.len(),
            1,
            "Bus 的 ACVChn 不应串成 Line 的电流分支"
        );
        assert_eq!(l.currents[0].bran_idx, 1);
        assert_eq!(l.currents[0].dir, -1);
        assert_eq!(l.currents[0].dir_raw, "NEG");
        assert_eq!(l.ana_chns, vec![11]);
        assert_eq!(l.sta_chns, vec![7]);
        assert!(l.voltages.is_empty(), "该线路无 ACVChn，不应从别处继承");

        assert_eq!(dmf.transformers.len(), 1);
        let t = &dmf.transformers[0];
        // 自闭合的第二个绕组必须收尾入列，不能被静默丢弃
        assert_eq!(t.windings.len(), 2);
        let w0 = &t.windings[0];
        assert_eq!(w0.wg, "y0", "wG 是字符串代号");
        assert_eq!((w0.acv.ua_idx, w0.acv.un_idx), (21, 24));
        assert_eq!(w0.currents[0].ia_idx, 31);
        assert_eq!((w0.igap.zgap_idx, w0.igap.zsgap_idx), (41, 42));
        let w1 = &t.windings[1];
        assert_eq!(w1.wg, "yn0");
        assert!(w1.acv.is_empty(), "自闭合绕组不应继承上一绕组的通道");
        assert!(w1.currents.is_empty());
    }

    #[test]
    fn test_unmodeled_element_children_are_not_misattributed() {
        // 未建模的非空元素其子元素不得被挂到外层设备上
        let xml = r#"<?xml version="1.0"?>
<scl:ComtradeModel station_name="S" version="1.0" reference="0" rec_dev_name="R">
    <scl:Line idx="1" line_name="L" bus_ID="" VRtg="0" ARtg="0" LinLen="0" bran_num="0">
        <scl:SDL_Group>
            <scl:ACC_Bran bran_idx="9" ia_idx="99" ib_idx="98" ic_idx="97" in_idx="96" dir="POS"/>
            <scl:StaChn idx_cfg="123"/>
        </scl:SDL_Group>
    </scl:Line>
</scl:ComtradeModel>"#;
        let dmf = parse_dmf(xml.as_bytes()).unwrap();
        let l = &dmf.lines[0];
        assert!(l.currents.is_empty(), "SDL_* 内的 ACC_Bran 应被丢弃");
        assert!(l.sta_chns.is_empty(), "SDL_* 内的 StaChn 应被丢弃");
    }

    #[test]
    fn test_parse_generator_and_exciter() {
        // 发电机 + 励磁机合成 XML（§1.6 / §1.7）
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<scl:ComtradeModel station_name="S" version="1.0" reference="0" rec_dev_name="R">
    <scl:Generator idx="5" gen_name="1号发电机" srcRef="g1" sys_ID="" trm_ID="20,主变A套" object_type="STEAM_TURBINE" freq="50" capacity="300" factor="0.85" V1="20" branch_z1="2" branch_z2="2" branch_z3="0" rotor_I="1800" rotor_V2="75" ufe_rated="100" ufe_no_load="50" xd="1.8" xq="1.7" xd_prime="0.3" xs="0.1" excitation_mode="0" igt_dir="1" ufe_chn="49" pos_ufe_chn="50" neg_ufe_chn="51" ife_chn="52" un_terminal="53" un_neutral="54" un_longitudinal="55" ta_ido_chn="56">
        <scl:ACVChn ua_idx="40" ub_idx="41" uc_idx="42" un_idx="0" ul_idx="0"/>
        <scl:ACC_Bran bran_idx="1" ia_idx="43" ib_idx="44" ic_idx="45" in_idx="0" dir="POS"/>
        <scl:ACC_Bran bran_idx="2" ia_idx="46" ib_idx="47" ic_idx="48" in_idx="0" dir="NEG"/>
        <scl:AnaChn idx_cfg="57"/>
        <scl:StaChn idx_cfg="100"/>
    </scl:Generator>
    <scl:Exciter idx="6" exc_name="1号励磁机" srcRef="e1" sys_ID="" pwr_ID="5,1号发电机" object_type="PRIMARY" freq="100" V1="0.5">
        <scl:ACVChn ua_idx="60" ub_idx="61" uc_idx="62" un_idx="63" ul_idx="0"/>
        <scl:ACC_Bran bran_idx="1" ia_idx="64" ib_idx="65" ic_idx="66" in_idx="0" dir="POS"/>
        <scl:ACC_Bran bran_idx="2" ia_idx="67" ib_idx="68" ic_idx="69" in_idx="0" dir="NEG"/>
    </scl:Exciter>
</scl:ComtradeModel>"#;
        let dmf = parse_dmf(xml.as_bytes()).unwrap();

        assert_eq!(dmf.generators.len(), 1);
        let g = &dmf.generators[0];
        assert_eq!(g.idx, 5);
        assert_eq!(g.name, "1号发电机");
        assert_eq!(g.trm_id, "20,主变A套");
        assert_eq!(g.object_type, "STEAM_TURBINE");
        assert_eq!(g.freq, 50.0);
        assert_eq!(g.capacity, 300.0);
        assert_eq!(g.factor, 0.85);
        assert_eq!(g.v1, 20.0);
        assert_eq!(
            (g.branch_num.z1, g.branch_num.z2, g.branch_num.z3),
            (2, 2, 0)
        );
        assert_eq!(g.rotor_i, 1800.0);
        assert_eq!(g.rotor_v2, 75.0);
        assert_eq!((g.ufe.rated, g.ufe.no_load), (100.0, 50.0));
        assert_eq!((g.x.xd, g.x.xq, g.x.xd_prime, g.x.xs), (1.8, 1.7, 0.3, 0.1));
        assert_eq!(g.excitation_mode, 0);
        assert_eq!(g.igt_dir, 1);
        assert_eq!((g.acv.ua_idx, g.acv.ub_idx, g.acv.uc_idx), (40, 41, 42));
        // bran_idx=1 → ta，bran_idx=2 → ta_z1
        assert_eq!((g.ta.ia_idx, g.ta.dir), (43, 1));
        assert_eq!((g.ta_z1.ia_idx, g.ta_z1.dir), (46, -1));
        assert_eq!(
            (g.ufe_chns.ufe, g.ufe_chns.pos, g.ufe_chns.neg),
            (49, 50, 51)
        );
        assert_eq!(g.ife_chn, 52);
        assert_eq!(
            (
                g.un_chns.terminal,
                g.un_chns.neutral,
                g.un_chns.longitudinal
            ),
            (53, 54, 55)
        );
        assert_eq!(g.ta_ido_chn, 56);
        assert_eq!(g.oth_achns, vec![57]);
        assert_eq!(g.sta_chns, vec![100]);

        assert_eq!(dmf.exciters.len(), 1);
        let e = &dmf.exciters[0];
        assert_eq!(e.idx, 6);
        assert_eq!(e.name, "1号励磁机");
        assert_eq!(e.pwr_id, "5,1号发电机");
        assert_eq!(e.object_type, "PRIMARY");
        assert_eq!(e.freq, 100.0);
        assert_eq!(e.v1, 0.5);
        assert_eq!(
            (e.acv.ua_idx, e.acv.ub_idx, e.acv.uc_idx, e.acv.un_idx),
            (60, 61, 62, 63)
        );
        assert_eq!((e.ta.ia_idx, e.ta.dir), (64, 1));
        assert_eq!((e.ta_z.ia_idx, e.ta_z.dir), (67, -1));
    }

    #[test]
    fn test_generator_exciter_round_trip() {
        // parse → serialize → reparse 逐字段比对
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<scl:ComtradeModel station_name="S" version="1.0" reference="0" rec_dev_name="R">
    <scl:Generator idx="5" gen_name="G1" srcRef="" sys_ID="" trm_ID="20,T" object_type="STEAM_TURBINE" freq="50" capacity="300" factor="0.85" V1="20" branch_z1="2" branch_z2="2" branch_z3="0" rotor_I="1800" rotor_V2="75" ufe_rated="100" ufe_no_load="50" xd="1.8" xq="1.7" xd_prime="0.3" xs="0.1" excitation_mode="0" igt_dir="1" ufe_chn="49" pos_ufe_chn="50" neg_ufe_chn="51" ife_chn="52" un_terminal="53" un_neutral="54" un_longitudinal="55" ta_ido_chn="56">
        <scl:ACVChn ua_idx="40" ub_idx="41" uc_idx="42" un_idx="0" ul_idx="0"/>
        <scl:ACC_Bran bran_idx="1" ia_idx="43" ib_idx="44" ic_idx="45" in_idx="0" dir="POS"/>
        <scl:ACC_Bran bran_idx="2" ia_idx="46" ib_idx="47" ic_idx="48" in_idx="0" dir="NEG"/>
        <scl:AnaChn idx_cfg="57"/>
        <scl:StaChn idx_cfg="100"/>
    </scl:Generator>
    <scl:Exciter idx="6" exc_name="E1" srcRef="" sys_ID="" pwr_ID="5,G1" object_type="PRIMARY" freq="100" V1="0.5">
        <scl:ACVChn ua_idx="60" ub_idx="61" uc_idx="62" un_idx="63" ul_idx="0"/>
        <scl:ACC_Bran bran_idx="1" ia_idx="64" ib_idx="65" ic_idx="66" in_idx="0" dir="POS"/>
        <scl:ACC_Bran bran_idx="2" ia_idx="67" ib_idx="68" ic_idx="69" in_idx="0" dir="NEG"/>
    </scl:Exciter>
</scl:ComtradeModel>"#;
        let dmf1 = parse_dmf(xml.as_bytes()).unwrap();
        let text = crate::dmf::xml_writer::write_dmf(&dmf1);
        let dmf2 = parse_dmf(text.as_bytes()).unwrap();

        let g1 = &dmf1.generators[0];
        let g2 = &dmf2.generators[0];
        assert_eq!(g1.idx, g2.idx);
        assert_eq!(g1.name, g2.name);
        assert_eq!(g1.trm_id, g2.trm_id);
        assert_eq!(g1.object_type, g2.object_type);
        assert_eq!(g1.freq, g2.freq);
        assert_eq!(g1.capacity, g2.capacity);
        assert_eq!(g1.factor, g2.factor);
        assert_eq!(g1.v1, g2.v1);
        assert_eq!(g1.branch_num, g2.branch_num);
        assert_eq!(g1.rotor_i, g2.rotor_i);
        assert_eq!(g1.rotor_v2, g2.rotor_v2);
        assert_eq!(g1.ufe, g2.ufe);
        assert_eq!(g1.x, g2.x);
        assert_eq!(g1.excitation_mode, g2.excitation_mode);
        assert_eq!(g1.igt_dir, g2.igt_dir);
        assert_eq!(g1.acv, g2.acv);
        assert_eq!(g1.ta, g2.ta);
        assert_eq!(g1.ta_z1, g2.ta_z1);
        assert_eq!(g1.ufe_chns, g2.ufe_chns);
        assert_eq!(g1.ife_chn, g2.ife_chn);
        assert_eq!(g1.un_chns, g2.un_chns);
        assert_eq!(g1.ta_ido_chn, g2.ta_ido_chn);
        assert_eq!(g1.oth_achns, g2.oth_achns);
        assert_eq!(g1.sta_chns, g2.sta_chns);

        let e1 = &dmf1.exciters[0];
        let e2 = &dmf2.exciters[0];
        assert_eq!(e1.idx, e2.idx);
        assert_eq!(e1.name, e2.name);
        assert_eq!(e1.pwr_id, e2.pwr_id);
        assert_eq!(e1.object_type, e2.object_type);
        assert_eq!(e1.freq, e2.freq);
        assert_eq!(e1.v1, e2.v1);
        assert_eq!(e1.acv, e2.acv);
        assert_eq!(e1.ta, e2.ta);
        assert_eq!(e1.ta_z, e2.ta_z);
    }
}
