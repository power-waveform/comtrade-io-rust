//! 采样信息模型

/// 额定频率缺省值（Hz）
///
/// 当 CFG 未给出或给出非法（`<= 0` / 非有限）额定频率时使用，
/// 对齐 Python 基线 `freq if freq else 50`。
pub const DEFAULT_NOMINAL_FREQ: f64 = 50.0;

/// 采样段
///
/// `samp_rate` / `end_point` 来自 CFG 文件本身；`start_point` / `count` /
/// `cycle_point_num` 是重算时才产生的派生字段，CFG 不携带，故解析出的段一律为 `None`
/// （对齐 Python 基线中这三个字段的 `| None` 类型与 `default=None`）。
#[derive(Debug, Clone, Copy)]
pub struct Segment {
    /// 该段采样率（Hz）
    pub samp_rate: f64,
    /// 该段累计结束点
    ///
    /// 即"该段最后一个采样点的 1-based 序号"，数值上等于半开区间
    /// `[start_point, end_point)` 的右端点（0-based）——两种表述取值相同，
    /// 对齐 CFG 的 `endsamp` 字段语义。
    pub end_point: usize,
    /// 该段起始采样点号（0-based，半开区间 `[start_point, end_point)` 的左端点）
    ///
    /// 派生字段，仅 `recalculate_segments`（dat）填充。
    pub start_point: Option<usize>,
    /// 该段采样点数（`end_point - start_point`）
    ///
    /// 派生字段，仅 `recalculate_segments`（dat）填充。
    pub count: Option<usize>,
    /// 该段每周波采样点数（`samp_rate / 额定频率`）
    ///
    /// 派生字段，仅 `recalculate_segments`（dat）按额定频率填充。
    pub cycle_point_num: Option<f64>,
}

impl Segment {
    /// 构造 CFG 原生段：仅采样率与结束点，派生字段留空
    pub fn new(samp_rate: f64, end_point: usize) -> Self {
        Segment {
            samp_rate,
            end_point,
            start_point: None,
            count: None,
            cycle_point_num: None,
        }
    }

    /// 构造重算段：附带 `start_point` / `count` / `cycle_point_num` 三个派生字段
    ///
    /// `start_point` 与 `end_point` 构成半开区间 `[start_point, end_point)`，
    /// `count` 由二者之差算出，`cycle_point_num` 为 `samp_rate / nominal_freq`。
    pub fn derived(
        samp_rate: f64,
        start_point: usize,
        end_point: usize,
        nominal_freq: f64,
    ) -> Self {
        Segment {
            samp_rate,
            end_point,
            start_point: Some(start_point),
            count: Some(end_point.saturating_sub(start_point)),
            cycle_point_num: Some(samp_rate / nominal_freq),
        }
    }
}

/// 采样信息
#[derive(Debug, Clone, Default)]
pub struct Sampling {
    /// 标称频率（Hz）
    pub freq: f64,
    /// 采样段列表
    pub segments: Vec<Segment>,
}

impl Sampling {
    /// 序列化为 CFG 文本（频率行 + 段数行 + 各段行）
    pub fn to_cfg_text(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!("{}", self.freq));
        lines.push(format!("{}", self.segments.len()));
        for seg in &self.segments {
            lines.push(format!("{:.0},{:.0}", seg.samp_rate, seg.end_point));
        }
        lines.join("\n")
    }
}
