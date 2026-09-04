//! INF 构建器：Config 与 EquipmentGroup 的构建

use super::section::{InfFile, Section, SectionKind};
use crate::cfg::{
    is_iec61850_reference, AnalogChannel, AnalogExt, Config, DataType, Sampling, Segment,
    StatusChannel, StatusExt, TranSide, Version,
};
use crate::equipment::{
    AccBran, AcvChn, BranchNum, Bus, Capacitance, EquipmentGroup, Exciter, Generator, Igap,
    Impedance, Line, MutualInductance, Px, SyncReactance, Transformer, TransformerWinding, Ufe,
    UfeChns, UnChns, WindingLocation,
};
use crate::time;

/// 由 INF 节构建 Config
pub fn build_config(inf: &InfFile, _existing: Option<&Config>) -> Option<Config> {
    let fd = inf.file_description()?;

    // 头部
    let station = fd.get("Station_Name").unwrap_or("").to_string();
    let recorder = fd.get("Recording_Device_ID").unwrap_or("").to_string();
    let rev_year = fd.get("Revision_Year").unwrap_or("1991");
    // 同 CFG 头部：六个版本号全部识别，无法识别时回落 1991。
    // 原实现只匹配 "1999"，2001/2008/2013/2017 会被静默判成 1991。
    let version = Version::parse(rev_year).unwrap_or_default();

    // 通道数
    let total = fd
        .get("Total_Channel_Count")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let analog = fd
        .get("Analog_Channel_Count")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let status = fd
        .get("Status_Channel_Count")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    // 采样
    let freq = fd
        .get("Line_Frequency")
        .and_then(|s| s.parse().ok())
        .unwrap_or(50.0);
    let sr_count = fd
        .get("Sample_Rate_Count")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let mut segments = Vec::new();
    for i in 1..=sr_count {
        let rate_key = format!("Sample_Rate_#{}", i);
        let end_key = format!("End_Sample_Rate_#{}", i);
        let rate = fd
            .get(&rate_key)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);
        let end = fd.get(&end_key).and_then(|s| s.parse().ok()).unwrap_or(0);
        if rate > 0.0 {
            segments.push(Segment::new(rate, end));
        }
    }

    // 时间
    let start_time = fd
        .get("File_Start_Time")
        .and_then(|s| time::parse(s).ok())
        .unwrap_or_else(|| time::parse("01/01/2000,00:00:00.000000").unwrap());
    let trigger_time = fd
        .get("Trigger_Time")
        .and_then(|s| time::parse(s).ok())
        .unwrap_or(start_time);

    // 数据格式
    let file_type = fd.get("File_Type").unwrap_or("ASCII");
    let data_type = DataType::parse(file_type).unwrap_or(DataType::Ascii);

    // timemult
    let timemult = fd
        .get("Time_Multiplier")
        .and_then(|s| s.parse().ok())
        .unwrap_or(1.0);

    let mut config = Config {
        header: crate::cfg::Header {
            station,
            recorder,
            version,
        },
        channels: crate::cfg::ChannelCount {
            total,
            analog,
            status,
        },
        analogs: Vec::new(),
        statuses: Vec::new(),
        sampling: Sampling { freq, segments },
        start_time,
        trigger_time,
        data_type,
        timemult,
        time_info: None,
        sampling_time_quality: None,
    };

    // 构建通道
    build_channels_from_inf(inf, &mut config);

    // 合并参数段
    apply_parameters(inf, &mut config);

    Some(config)
}

fn build_channels_from_inf(inf: &InfFile, config: &mut Config) {
    // 模拟通道
    let analog_sections = inf.sections_of(&SectionKind::AnalogChannel);
    for section in &analog_sections {
        let index = section.index;
        let name = section.get("Channel_ID").unwrap_or("").to_string();
        let phase = section.get("Phase_ID").unwrap_or("").to_string();
        let monitored_component = section.get("Monitored_Component").unwrap_or("").trim();
        let (equipment, reference) = if is_iec61850_reference(monitored_component) {
            (String::new(), Some(monitored_component.to_string()))
        } else {
            (monitored_component.to_string(), None)
        };
        let unit = section.get("Channel_Units").unwrap_or("").to_string();
        let multiplier = section
            .get("Channel_Multiplier")
            .and_then(|s| s.parse().ok())
            .unwrap_or(1.0);
        let offset = section
            .get("Channel_Offset")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);
        let delay = section
            .get("Channel_Skew")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);
        let min_value = section
            .get("Range_Minimum_Limit_Value")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);
        let max_value = section
            .get("Range_Maximum_Limit_Value")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);
        let primary = section
            .get("Channel_Ratio_Primary")
            .and_then(|s| s.parse().ok())
            .unwrap_or(1.0);
        let secondary = section
            .get("Channel_Ratio_Secondary")
            .and_then(|s| s.parse().ok())
            .unwrap_or(1.0);
        let ps = section.get("Data_Primary_Secondary").unwrap_or("S");
        let tran_side = TranSide::parse(ps);

        config.analogs.push(AnalogChannel {
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
            ext: reference.map(|reference| AnalogExt {
                reference: Some(reference),
                ..Default::default()
            }),
        });
    }

    // 状态通道
    let status_sections = inf.sections_of(&SectionKind::StatusChannel);
    for section in &status_sections {
        let index = section.index;
        let name = section.get("Channel_ID").unwrap_or("").to_string();
        let phase = section.get("Phase_ID").unwrap_or("").to_string();
        let monitored_component = section.get("Monitored_Component").unwrap_or("").trim();
        let (equipment, reference) = if is_iec61850_reference(monitored_component) {
            (String::new(), Some(monitored_component.to_string()))
        } else {
            (monitored_component.to_string(), None)
        };
        let contact = section
            .get("Normal_State")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        config.statuses.push(StatusChannel {
            index,
            name,
            phase,
            equipment,
            contact,
            ext: reference.map(|reference| StatusExt {
                reference: Some(reference),
                ..Default::default()
            }),
        });
    }
}

fn apply_parameters(inf: &InfFile, config: &mut Config) {
    // 模拟量参数段
    let param_sections = inf.sections_of(&SectionKind::AnalogChannelsParameter);
    for section in &param_sections {
        for (_key, value) in &section.fields {
            let parts: Vec<&str> = value.split(',').map(|s| s.trim()).collect();
            if parts.len() < 11 {
                continue;
            }
            let idx_cfg: usize = parts[0].parse().unwrap_or(0);
            let idx_org: usize = parts[1].parse().unwrap_or(0);
            // parts[2] 为通道名，CFG 中已有权威值，此处忽略
            let flag = parts[3].to_string();
            let freq_val: f64 = parts[4].parse().unwrap_or(50.0);
            let primary: f64 = parts[5].parse().unwrap_or(1.0);
            let secondary: f64 = parts[7].parse().unwrap_or(1.0);
            let au: f64 = parts[9].parse().unwrap_or(0.0);
            let bu: f64 = parts[10].parse().unwrap_or(0.0);

            if let Some(ch) = config.analogs.iter_mut().find(|a| a.index == idx_cfg) {
                let reference = ch.ext.as_ref().and_then(|e| e.reference.clone());
                ch.ext = Some(AnalogExt {
                    idx_org: Some(idx_org),
                    freq: Some(freq_val),
                    au: Some(au),
                    bu: Some(bu),
                    channel_type: None,
                    flag: Some(flag),
                    reference,
                });
                ch.primary = primary;
                ch.secondary = secondary;
            }
        }
    }

    // 状态量参数段
    let status_param_sections = inf.sections_of(&SectionKind::StatusChannelsParameter);
    for section in &status_param_sections {
        for (_key, value) in &section.fields {
            let parts: Vec<&str> = value.split(',').map(|s| s.trim()).collect();
            if parts.len() < 6 {
                continue;
            }
            let idx_cfg: usize = parts[0].parse().unwrap_or(0);
            let idx_org: usize = parts[1].parse().unwrap_or(0);
            let channel_type = parts[3].to_string();
            let flag = parts[4].to_string();
            let equipment_no = parts[5].to_string();

            if let Some(ch) = config.statuses.iter_mut().find(|s| s.index == idx_cfg) {
                let reference = ch.ext.as_ref().and_then(|e| e.reference.clone());
                ch.ext = Some(StatusExt {
                    idx_org: Some(idx_org),
                    channel_type: if channel_type.is_empty() {
                        None
                    } else {
                        Some(channel_type)
                    },
                    flag: if flag.is_empty() { None } else { Some(flag) },
                    contact: None,
                    reference,
                    equipment_no: if equipment_no.is_empty() {
                        None
                    } else {
                        Some(equipment_no)
                    },
                });
            }
        }
    }
}

// ========== INF 设备段解析辅助 ==========

/// 解析带单位后缀的数值，如 `22.45(km)` / `220000V` / `0.071(Ω/km)` / `180(MVA)`。
///
/// 保留前导负号（`REACTOR=-1(Ω)` 是有效值），从首个非数值字符处截断。
/// 无法解析时返回 `None`。
fn parse_num_with_unit(s: &str) -> Option<f64> {
    let s = s.trim();
    let mut end = 0;
    let mut seen_dot = false;
    let mut seen_exp = false;
    for (i, c) in s.char_indices() {
        let ok = match c {
            '0'..='9' => true,
            // 负号/正号只允许出现在开头或指数符号之后
            '-' | '+' => i == 0 || matches!(s.as_bytes().get(i - 1), Some(b'e') | Some(b'E')),
            '.' if !seen_dot && !seen_exp => {
                seen_dot = true;
                true
            },
            'e' | 'E' if !seen_exp && i > 0 => {
                seen_exp = true;
                true
            },
            _ => false,
        };
        if !ok {
            break;
        }
        end = i + c.len_utf8();
    }
    if end == 0 {
        return None;
    }
    s[..end].parse().ok()
}

/// 解析逗号分隔的通道号列表。
///
/// 偏离 Python 基线：Python 的 `str2ids` 要求通道号严格递增，非递增项被静默跳过。
/// 该过滤会吃掉列表中的 `0`（未配置占位）以及紧随其后的方向标志位，
/// 例如 `TA_CHNS=41,42,43,0,1` 会被压缩为 `[41,42,43]`，方向标志整体丢失。
/// 此处按原文逐项解析，不做递增过滤。
fn parse_num_list(s: &str) -> Vec<usize> {
    s.split(',')
        .filter_map(|p| {
            let p = p.trim();
            if p.is_empty() {
                None
            } else {
                parse_num_with_unit(p).map(|v| v.max(0.0) as usize)
            }
        })
        .collect()
}

/// 解析 `TV_CHNS=Ua,Ub,Uc,Un[,Ul]` 形式的电压通道组。
fn parse_acv(s: &str) -> AcvChn {
    let v = parse_num_list(s);
    AcvChn {
        ua_idx: v.first().copied().unwrap_or(0),
        ub_idx: v.get(1).copied().unwrap_or(0),
        uc_idx: v.get(2).copied().unwrap_or(0),
        un_idx: v.get(3).copied().unwrap_or(0),
        ul_idx: v.get(4).copied().unwrap_or(0),
    }
}

/// 解析线路 `TA_CHNS=Ia,Ib,Ic,Io[,DIR]`（规范 §1.4，5 字段：含 `Io`）。
///
/// `DIR` 为可选尾项：`1` = 正方向（流向线路），`-1` = 反方向，缺省 `1`。
fn parse_line_ta(s: &str, bran_idx: usize) -> AccBran {
    let parts: Vec<&str> = s.split(',').map(|p| p.trim()).collect();
    let num = |i: usize| -> usize {
        parts
            .get(i)
            .and_then(|p| parse_num_with_unit(p))
            .map(|v| v.max(0.0) as usize)
            .unwrap_or(0)
    };
    AccBran {
        bran_idx,
        ia_idx: num(0),
        ib_idx: num(1),
        ic_idx: num(2),
        in_idx: num(3),
        dir: parse_dir_field(parts.get(4)),
        dir_raw: parts.get(4).map(|s| s.to_string()).unwrap_or_default(),
    }
}

/// 解析变压器 `TA_Id_#N=Ia,Ib,Ic,极性符号`（规范 §1.5，4 字段：**无** `Io`）。
///
/// 与线路 `TA_CHNS` 的字段数不同：变压器侧第 4 项已是极性符号，不是零序电流通道。
/// 混用同一个解析器会把极性符号误读成 `in_idx`（Python 基线即如此）。
fn parse_transformer_ta(s: &str, bran_idx: usize) -> AccBran {
    let parts: Vec<&str> = s.split(',').map(|p| p.trim()).collect();
    let num = |i: usize| -> usize {
        parts
            .get(i)
            .and_then(|p| parse_num_with_unit(p))
            .map(|v| v.max(0.0) as usize)
            .unwrap_or(0)
    };
    AccBran {
        bran_idx,
        ia_idx: num(0),
        ib_idx: num(1),
        ic_idx: num(2),
        in_idx: 0,
        dir: parse_dir_field(parts.get(3)),
        dir_raw: parts.get(3).map(|s| s.to_string()).unwrap_or_default(),
    }
}

/// 解析方向/极性字段：`-1` → `-1`，其他（含缺省、空串、非法值）→ `1`。
fn parse_dir_field(raw: Option<&&str>) -> i32 {
    match raw.map(|s| s.trim()) {
        Some(s) if !s.is_empty() && parse_num_with_unit(s).unwrap_or(1.0) < 0.0 => -1,
        _ => 1,
    }
}

/// 解析 `DEV_ID=idx,name`，返回 `(idx, name)`。
///
/// `name` 中可能含逗号，只按首个逗号切分。缺失 idx 时回落到 `fallback_idx`（节序号）。
fn split_dev_id(raw: &str, fallback_idx: usize) -> (usize, String) {
    match raw.split_once(',') {
        Some((idx, name)) => {
            let idx = idx
                .trim()
                .parse::<usize>()
                .ok()
                .filter(|v| *v > 0)
                .unwrap_or(fallback_idx);
            (idx, name.trim().to_string())
        },
        None => {
            let t = raw.trim();
            // 无逗号：整串可能是纯 idx，也可能是纯名字
            match t.parse::<usize>() {
                Ok(v) if v > 0 => (v, String::new()),
                _ => (fallback_idx, t.to_string()),
            }
        },
    }
}

/// 解析 `REACTOR`：`NO`（不区分大小写）表示无并联电抗器 → `None`。
///
/// 样本中还存在 `-1(Ω)` 的写法，同样按"无"处理：负补偿电抗无物理意义。
fn parse_reactor(raw: &str) -> Option<f64> {
    let t = raw.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("NO") {
        return None;
    }
    match parse_num_with_unit(t) {
        Some(v) if v < 0.0 => None,
        other => other,
    }
}

/// 解析 `RX=R1,X1,R0,X0`
fn parse_impedance(s: &str) -> Impedance {
    let v: Vec<f64> = s
        .split(',')
        .map(|p| parse_num_with_unit(p).unwrap_or(0.0))
        .collect();
    Impedance {
        r1: v.first().copied().unwrap_or(0.0),
        x1: v.get(1).copied().unwrap_or(0.0),
        r0: v.get(2).copied().unwrap_or(0.0),
        x0: v.get(3).copied().unwrap_or(0.0),
    }
}

/// 解析 `CG=C1,G1,C0,G0`
///
/// 注意位置序：规范 §1.4 为 `C1,G1,C0,G0`（容/导交替），
/// **不是** DMF `CG` 元素属性所暗示的 `c1,c0,g1,g0`。
/// Python 基线按 `c1,c0,g1,g0` 读取，会把零序电容与正序电导对调。
fn parse_capacitance(s: &str) -> Capacitance {
    let v: Vec<f64> = s
        .split(',')
        .map(|p| parse_num_with_unit(p).unwrap_or(0.0))
        .collect();
    Capacitance {
        c1: v.first().copied().unwrap_or(0.0),
        g1: v.get(1).copied().unwrap_or(0.0),
        c0: v.get(2).copied().unwrap_or(0.0),
        g0: v.get(3).copied().unwrap_or(0.0),
    }
}

/// 解析 `MRX=MR,MX`
fn parse_mutual(s: &str) -> MutualInductance {
    let v: Vec<f64> = s
        .split(',')
        .map(|p| parse_num_with_unit(p).unwrap_or(0.0))
        .collect();
    MutualInductance {
        idx: 0,
        mr0: v.first().copied().unwrap_or(0.0),
        mx0: v.get(1).copied().unwrap_or(0.0),
    }
}

/// 解析 `{H,M,L}_PARAM=接线方式, 一次额定电压(kV), 分支数`
///
/// 第一字段是绕组接线组别**字符串代号**（`Y` / `Y12` / `D11` / `yn0`），不是数值。
fn parse_winding_param(s: &str) -> (String, f64, usize) {
    let parts: Vec<&str> = s.split(',').map(|p| p.trim()).collect();
    let wg = parts.first().copied().unwrap_or("").to_string();
    let v_rtg = parts
        .get(1)
        .and_then(|p| parse_num_with_unit(p))
        .unwrap_or(0.0);
    let bran_num = parts
        .get(2)
        .and_then(|p| parse_num_with_unit(p))
        .map(|v| v.max(0.0) as usize)
        .unwrap_or(0);
    (wg, v_rtg, bran_num)
}

/// 按 `WINDING_NUM` 确定应构建的绕组侧。
///
/// 规范 §1.5：`3` = 三卷变（高/中/低），`2` = 两卷变（高/低，**缺中压侧**），
/// `1` = 自耦变。
///
/// 自耦变（`1`）当前按两卷变（高/低）降级处理 —— 其专有的 `TA_Idcw_#n`
/// 公共绕组电流通道尚未建模。
/// TODO: 建模 `TA_Idcw_#1..#5` 后为自耦变返回独立的绕组布局。
fn winding_layout(winding_num: usize) -> &'static [WindingLocation] {
    match winding_num {
        3 => &[
            WindingLocation::High,
            WindingLocation::Medium,
            WindingLocation::Low,
        ],
        // 2（两卷变）与 1（自耦变）均为高/低两侧
        _ => &[WindingLocation::High, WindingLocation::Low],
    }
}

/// 构建单个绕组侧。该侧既无 `_PARAM` 也无任何 `TA_Id_#N` 时返回 `None`，
/// 避免为不存在的中压侧凭空造出空绕组。
fn build_winding(section: &Section, loc: WindingLocation) -> Option<TransformerWinding> {
    let p = loc.inf_prefix();

    let param = section.get(&format!("{}_PARAM", p));
    let mut currents = Vec::new();
    for (n, ta_idx) in loc.ta_id_indices().iter().enumerate() {
        if let Some(raw) = section.get(&format!("TA_Id_#{}", ta_idx)) {
            let bran = parse_transformer_ta(raw, n + 1);
            if !bran.is_empty() {
                currents.push(bran);
            }
        }
    }

    if param.is_none() && currents.is_empty() {
        return None;
    }

    let (wg, v_rtg, bran_num) = param.map(parse_winding_param).unwrap_or_default();

    Some(TransformerWinding {
        location: loc.as_str().to_string(),
        src_ref: String::new(),
        v_rtg,
        a_rtg: 0.0,
        // `_PARAM` 缺失时用实际解析出的分支数兜底
        bran_num: if bran_num > 0 {
            bran_num
        } else {
            currents.len()
        },
        bus_id: String::new(),
        wg,
        acv: section
            .get(&format!("{}_TV_CHNS", p))
            .map(parse_acv)
            .unwrap_or_default(),
        currents,
        igap: Igap::default(),
        ta_zs: section
            .get(&format!("{}_TA_ZS", p))
            .and_then(parse_num_with_unit)
            .map(|v| v.max(0.0) as usize)
            .unwrap_or(0),
        ta_zs_gap: section
            .get(&format!("{}_TA_ZS_GAP", p))
            .and_then(parse_num_with_unit)
            .map(|v| v.max(0.0) as usize)
            .unwrap_or(0),
        sta_chns: section
            .get(&format!("{}_STATUS_CHNS", p))
            .map(parse_num_list)
            .unwrap_or_default(),
    })
}

/// 解析 N 元组数值（逗号分隔，逐项 `parse_num_with_unit`，不足补 0）。
fn parse_tuple(s: Option<&str>, n: usize) -> Vec<f64> {
    let mut out = vec![0.0; n];
    if let Some(s) = s {
        for (i, part) in s.split(',').take(n).enumerate() {
            if let Some(v) = parse_num_with_unit(part) {
                out[i] = v;
            }
        }
    }
    out
}

/// 解析 N 元组通道号（逗号分隔，逐项转 `usize`，不足补 0）。
fn parse_tuple_usize(s: Option<&str>, n: usize) -> Vec<usize> {
    let mut out = vec![0usize; n];
    if let Some(s) = s {
        for (i, part) in s.split(',').take(n).enumerate() {
            if let Some(v) = parse_num_with_unit(part) {
                out[i] = v.max(0.0) as usize;
            }
        }
    }
    out
}

/// 解析 §1.6 `BRANCH_NUM=z1, z2, z3`
fn parse_branch_num(s: Option<&str>) -> BranchNum {
    let v = parse_tuple_usize(s, 3);
    BranchNum {
        z1: v[0],
        z2: v[1],
        z3: v[2],
    }
}

/// 解析 §1.6 `Ufe=额定励磁电压, 额定空载励磁电压`
fn parse_ufe(s: Option<&str>) -> Ufe {
    let v = parse_tuple(s, 2);
    Ufe {
        rated: v[0],
        no_load: v[1],
    }
}

/// 解析 §1.6 `X=Xd, Xq, Xd', Xs`
fn parse_sync_reactance(s: Option<&str>) -> SyncReactance {
    let v = parse_tuple(s, 4);
    SyncReactance {
        xd: v[0],
        xq: v[1],
        xd_prime: v[2],
        xs: v[3],
    }
}

/// 解析 §1.6 `Ufe_CHNS=Ufe, +Ufe, -Ufe`
fn parse_ufe_chns(s: Option<&str>) -> UfeChns {
    let v = parse_tuple_usize(s, 3);
    UfeChns {
        ufe: v[0],
        pos: v[1],
        neg: v[2],
    }
}

/// 解析 `UN_CHNS=机端零序, 中性点零序, 纵向零序`
fn parse_un_chns(s: Option<&str>) -> UnChns {
    let v = parse_tuple_usize(s, 3);
    UnChns {
        terminal: v[0],
        neutral: v[1],
        longitudinal: v[2],
    }
}

/// 构建设备组
///
/// 偏离 Python 基线：原 Rust 实现读 `bus_name` / `line_name` / `trm_name`
/// （DMF XML 的属性名），而 INF 段用的是 `DEV_ID=idx,name`，
/// 结果全部字段落空——设备只剩空名字和节序号。此处按 INF 真实 Key 解析。
pub fn build_equipment_group(inf: &InfFile) -> EquipmentGroup {
    let mut eg = EquipmentGroup::default();

    for section in &inf.sections {
        match &section.kind {
            SectionKind::Bus => {
                let (idx, name) = split_dev_id(section.get("DEV_ID").unwrap_or(""), section.index);
                eg.buses.push(Bus {
                    idx,
                    name,
                    src_ref: String::new(),
                    sys_id: section.get("SYS_ID").unwrap_or("").to_string(),
                    // `RATED_VALUE=220000V` 是一次额定电压，单位 V；模型以 kV 存储
                    v_rtg: section
                        .get("RATED_VALUE")
                        .and_then(parse_num_with_unit)
                        .map(|v| v / 1000.0)
                        .unwrap_or(0.0),
                    v_rtg_snd: 0.0,
                    tv_pos: section.get("TV_POS").unwrap_or("").to_string(),
                    is_location: String::new(),
                    bus_uuid: String::new(),
                    acv: section.get("TV_CHNS").map(parse_acv).unwrap_or_default(),
                    ana_chns: section
                        .get("OTH_ACHNS")
                        .map(parse_num_list)
                        .unwrap_or_default(),
                    sta_chns: section
                        .get("STATUS_CHNS")
                        .map(parse_num_list)
                        .unwrap_or_default(),
                });
            },
            SectionKind::Line => {
                let (idx, name) = split_dev_id(section.get("DEV_ID").unwrap_or(""), section.index);

                // 电流分支：`TA_CHNS` 为第一组，`TA_CHNS_#2` 为第二组
                let mut currents = Vec::new();
                if let Some(raw) = section.get("TA_CHNS") {
                    currents.push(parse_line_ta(raw, 1));
                }
                if let Some(raw) = section.get("TA_CHNS_#2") {
                    currents.push(parse_line_ta(raw, 2));
                }

                // 电压通道组：`TV_CHNS` 与 `TV_CHNS_#n` 兼容并存（规范 §1.4）
                let mut voltages = Vec::new();
                if let Some(raw) = section.get("TV_CHNS") {
                    voltages.push(parse_acv(raw));
                }
                for n in 1..=8 {
                    if let Some(raw) = section.get(&format!("TV_CHNS_#{}", n)) {
                        let acv = parse_acv(raw);
                        if !voltages.contains(&acv) {
                            voltages.push(acv);
                        }
                    }
                }

                let bran_num = currents.len();
                eg.lines.push(Line {
                    idx,
                    name,
                    bus_id: String::new(),
                    src_ref: String::new(),
                    sys_id: section.get("SYS_ID").unwrap_or("").to_string(),
                    object_type: section.get("OBJECT_TYPE").unwrap_or("").to_string(),
                    v_rtg: 0.0,
                    a_rtg: 0.0,
                    a_rtg_snd: 0.0,
                    line_len: section
                        .get("LENGTH")
                        .and_then(parse_num_with_unit)
                        .unwrap_or(0.0),
                    bran_num,
                    line_uuid: String::new(),
                    remote_id: String::new(),
                    remote_flag: String::new(),
                    differential_id: String::new(),
                    other_id: section.get("OTHER_ID").unwrap_or("").to_string(),
                    reactor: section.get("REACTOR").and_then(parse_reactor),
                    impedance: section.get("RX").map(parse_impedance).unwrap_or_default(),
                    capacitance: section.get("CG").map(parse_capacitance).unwrap_or_default(),
                    mutual_inductance: section.get("MRX").map(parse_mutual).unwrap_or_default(),
                    px: Px::default(),
                    currents,
                    voltages,
                    differential_current: AcvChn::default(),
                    oth_achns: section
                        .get("OTH_ACHNS")
                        .map(parse_num_list)
                        .unwrap_or_default(),
                    ana_chns: Vec::new(),
                    sta_chns: section
                        .get("STATUS_CHNS")
                        .map(parse_num_list)
                        .unwrap_or_default(),
                });
            },
            SectionKind::Transformer => {
                let (idx, name) = split_dev_id(section.get("DEV_ID").unwrap_or(""), section.index);
                let winding_num = section
                    .get("WINDING_NUM")
                    .and_then(parse_num_with_unit)
                    .map(|v| v.max(0.0) as usize)
                    .unwrap_or(0);

                let windings = winding_layout(winding_num)
                    .iter()
                    .filter_map(|loc| build_winding(section, *loc))
                    .collect();

                eg.transformers.push(Transformer {
                    idx,
                    name,
                    src_ref: String::new(),
                    sys_id: section.get("SYS_ID").unwrap_or("").to_string(),
                    object_type: section.get("OBJECT_TYPE").unwrap_or("").to_string(),
                    pwr_rtg: section
                        .get("CAPACITY")
                        .and_then(parse_num_with_unit)
                        .unwrap_or(0.0),
                    winding_num,
                    ta_self_comp: section.get("TA_SELF_COMP").unwrap_or("").to_string(),
                    transformer_uuid: String::new(),
                    windings,
                    oth_achns: section
                        .get("OTH_ACHNS")
                        .map(parse_num_list)
                        .unwrap_or_default(),
                    ana_chns: Vec::new(),
                    sta_chns: section
                        .get("STATUS_CHNS")
                        .map(parse_num_list)
                        .unwrap_or_default(),
                });
            },
            SectionKind::Generator => {
                // 规范 §1.6 `[ZYHD POWER_#n]`
                let (idx, name) = split_dev_id(section.get("DEV_ID").unwrap_or(""), section.index);
                eg.generators.push(Generator {
                    idx,
                    name,
                    src_ref: String::new(),
                    sys_id: section.get("SYS_ID").unwrap_or("").to_string(),
                    // TRM_ID 与 DEV_ID 同语法（idx,name），原文存储保 round-trip
                    trm_id: section.get("TRM_ID").unwrap_or("").to_string(),
                    object_type: section.get("OBJECT_TYPE").unwrap_or("").to_string(),
                    freq: section
                        .get("FREQ")
                        .and_then(parse_num_with_unit)
                        .unwrap_or(0.0),
                    capacity: section
                        .get("CAPACITY")
                        .and_then(parse_num_with_unit)
                        .unwrap_or(0.0),
                    factor: section
                        .get("FACTOR")
                        .and_then(parse_num_with_unit)
                        .unwrap_or(0.0),
                    v1: section
                        .get("V1")
                        .and_then(parse_num_with_unit)
                        .unwrap_or(0.0),
                    branch_num: parse_branch_num(section.get("BRANCH_NUM")),
                    rotor_i: section
                        .get("Rotor_I")
                        .and_then(parse_num_with_unit)
                        .unwrap_or(0.0),
                    rotor_v2: section
                        .get("Rotor_V2")
                        .and_then(parse_num_with_unit)
                        .unwrap_or(0.0),
                    ufe: parse_ufe(section.get("Ufe")),
                    x: parse_sync_reactance(section.get("X")),
                    excitation_mode: section
                        .get("EXCITATION_MODE")
                        .and_then(parse_num_with_unit)
                        .map(|v| v as i32)
                        .unwrap_or(0),
                    igt_dir: section
                        .get("IGT_DIR")
                        .and_then(parse_num_with_unit)
                        .map(|v| v as i32)
                        .unwrap_or(0),
                    // TV_CHNS 仅 3 字段（Ua,Ub,Uc），un_idx 恒 0
                    acv: parse_acv(section.get("TV_CHNS").unwrap_or("")),
                    ta: parse_transformer_ta(section.get("TA_CHNS").unwrap_or(""), 1),
                    ta_z1: parse_transformer_ta(section.get("TA_Z1_CHNS").unwrap_or(""), 1),
                    ta_z2: parse_transformer_ta(section.get("TA_Z2_CHNS").unwrap_or(""), 2),
                    ta_z3: parse_transformer_ta(section.get("TA_Z3_CHNS").unwrap_or(""), 3),
                    ufe_chns: parse_ufe_chns(section.get("Ufe_CHNS")),
                    ife_chn: section
                        .get("Ife_CHN")
                        .and_then(parse_num_with_unit)
                        .map(|v| v.max(0.0) as usize)
                        .unwrap_or(0),
                    un_chns: parse_un_chns(section.get("UN_CHNS")),
                    ta_ido_chn: section
                        .get("TA_Ido_CHN")
                        .and_then(parse_num_with_unit)
                        .map(|v| v.max(0.0) as usize)
                        .unwrap_or(0),
                    oth_achns: section
                        .get("OTH_ACHNS")
                        .map(parse_num_list)
                        .unwrap_or_default(),
                    sta_chns: section
                        .get("STATUS_CHNS")
                        .map(parse_num_list)
                        .unwrap_or_default(),
                });
            },
            SectionKind::Exciter => {
                // 规范 §1.7 `[ZYHD EXCITATION_#n]`
                let (idx, name) = split_dev_id(section.get("DEV_ID").unwrap_or(""), section.index);
                eg.exciters.push(Exciter {
                    idx,
                    name,
                    src_ref: String::new(),
                    sys_id: section.get("SYS_ID").unwrap_or("").to_string(),
                    // PWR_ID 与 DEV_ID 同语法（idx,name），原文存储保 round-trip
                    pwr_id: section.get("PWR_ID").unwrap_or("").to_string(),
                    object_type: section.get("OBJECT_TYPE").unwrap_or("").to_string(),
                    freq: section
                        .get("FREQ")
                        .and_then(parse_num_with_unit)
                        .unwrap_or(0.0),
                    v1: section
                        .get("V1")
                        .and_then(parse_num_with_unit)
                        .unwrap_or(0.0),
                    // TV_CHNS 为 4 字段（Ua,Ub,Uc,Un）
                    acv: parse_acv(section.get("TV_CHNS").unwrap_or("")),
                    ta: parse_transformer_ta(section.get("TA_CHNS").unwrap_or(""), 1),
                    ta_z: parse_transformer_ta(section.get("TA_Z_CHNS").unwrap_or(""), 1),
                    oth_achns: section
                        .get("OTH_ACHNS")
                        .map(parse_num_list)
                        .unwrap_or_default(),
                    sta_chns: section
                        .get("STATUS_CHNS")
                        .map(parse_num_list)
                        .unwrap_or_default(),
                });
            },
            _ => {},
        }
    }

    eg
}

// ========== 序列化辅助函数 ==========

pub fn build_file_description(cfg: &Config) -> Section {
    let mut fields = vec![
        ("Station_Name".to_string(), cfg.header.station.clone()),
        (
            "Recording_Device_ID".to_string(),
            cfg.header.recorder.clone(),
        ),
        (
            "Revision_Year".to_string(),
            cfg.header.version.as_str().to_string(),
        ),
        (
            "Total_Channel_Count".to_string(),
            cfg.channels.total.to_string(),
        ),
        (
            "Analog_Channel_Count".to_string(),
            cfg.channels.analog.to_string(),
        ),
        (
            "Status_Channel_Count".to_string(),
            cfg.channels.status.to_string(),
        ),
        ("Line_Frequency".to_string(), cfg.sampling.freq.to_string()),
    ];

    for (i, seg) in cfg.sampling.segments.iter().enumerate() {
        fields.push((format!("Sample_Rate_#{}", i + 1), seg.samp_rate.to_string()));
        fields.push((
            format!("End_Sample_Rate_#{}", i + 1),
            seg.end_point.to_string(),
        ));
    }
    fields.push((
        "Sample_Rate_Count".to_string(),
        cfg.sampling.segments.len().to_string(),
    ));

    fields.push((
        "File_Start_Time".to_string(),
        time::format_cfg(&cfg.start_time),
    ));
    fields.push((
        "Trigger_Time".to_string(),
        time::format_cfg(&cfg.trigger_time),
    ));
    fields.push(("File_Type".to_string(), cfg.data_type.as_str().to_string()));
    fields.push(("Time_Multiplier".to_string(), cfg.timemult.to_string()));

    Section {
        area: "Public".into(),
        kind: SectionKind::FileDescription,
        index: 0,
        fields,
        raw: String::new(),
    }
}

pub fn build_analog_section(ch: &AnalogChannel) -> Section {
    let monitored_component = if ch.equipment.trim().is_empty() {
        ch.ext
            .as_ref()
            .and_then(|ext| ext.reference.as_deref())
            .unwrap_or("")
    } else {
        &ch.equipment
    };
    let fields = vec![
        ("Channel_ID".to_string(), ch.name.clone()),
        ("Phase_ID".to_string(), ch.phase.clone()),
        (
            "Monitored_Component".to_string(),
            monitored_component.to_string(),
        ),
        ("Channel_Units".to_string(), ch.unit.clone()),
        ("Channel_Multiplier".to_string(), ch.multiplier.to_string()),
        ("Channel_Offset".to_string(), ch.offset.to_string()),
        ("Channel_Skew".to_string(), ch.delay.to_string()),
        (
            "Range_Minimum_Limit_Value".to_string(),
            ch.min_value.to_string(),
        ),
        (
            "Range_Maximum_Limit_Value".to_string(),
            ch.max_value.to_string(),
        ),
        ("Channel_Ratio_Primary".to_string(), ch.primary.to_string()),
        (
            "Channel_Ratio_Secondary".to_string(),
            ch.secondary.to_string(),
        ),
        (
            "Data_Primary_Secondary".to_string(),
            ch.tran_side.as_str().to_string(),
        ),
    ];

    Section {
        area: "Public".into(),
        kind: SectionKind::AnalogChannel,
        index: ch.index,
        fields,
        raw: String::new(),
    }
}

pub fn build_status_section(ch: &StatusChannel) -> Section {
    let monitored_component = if ch.equipment.trim().is_empty() {
        ch.ext
            .as_ref()
            .and_then(|ext| ext.reference.as_deref())
            .unwrap_or("")
    } else {
        &ch.equipment
    };
    let fields = vec![
        ("Channel_ID".to_string(), ch.name.clone()),
        ("Phase_ID".to_string(), ch.phase.clone()),
        (
            "Monitored_Component".to_string(),
            monitored_component.to_string(),
        ),
        ("Normal_State".to_string(), ch.contact.to_string()),
    ];

    Section {
        area: "Public".into(),
        kind: SectionKind::StatusChannel,
        index: ch.index,
        fields,
        raw: String::new(),
    }
}

pub fn build_analog_parameter(ch: &AnalogChannel) -> String {
    let ext = ch.ext.as_ref();
    let idx_org = ext.and_then(|e| e.idx_org).unwrap_or(0);
    let flag = ext.and_then(|e| e.flag.as_deref()).unwrap_or("");
    let freq_val = ext.and_then(|e| e.freq).unwrap_or(50.0);
    let au = ext.and_then(|e| e.au).unwrap_or(0.0);
    let bu = ext.and_then(|e| e.bu).unwrap_or(0.0);

    format!(
        "{}, {}, {}, {}, {}, {}, , {}, , {}, {}",
        ch.index, idx_org, ch.name, flag, freq_val, ch.primary, ch.secondary, au, bu
    )
}

pub fn build_status_parameter(ch: &StatusChannel) -> String {
    let ext = ch.ext.as_ref();
    let idx_org = ext.and_then(|e| e.idx_org).unwrap_or(0);
    let ch_type = ext.and_then(|e| e.channel_type.as_deref()).unwrap_or("");
    let flag = ext.and_then(|e| e.flag.as_deref()).unwrap_or("");
    let eq_no = ext.and_then(|e| e.equipment_no.as_deref()).unwrap_or("");

    format!(
        "{}, {}, {}, {}, {}, {}",
        ch.index, idx_org, ch.name, ch_type, flag, eq_no
    )
}

// ========== 设备段序列化 ==========

/// 格式化通道号列表为 `a, b, c` 形式
fn fmt_num_list(idxs: &[usize]) -> String {
    idxs.iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

/// 格式化 `DEV_ID=idx,name`
fn fmt_dev_id(idx: usize, name: &str) -> String {
    format!("{},{}", idx, name)
}

/// 格式化电压通道组为 `Ua, Ub, Uc, Un`
fn fmt_acv(acv: &AcvChn) -> String {
    format!(
        "{}, {}, {}, {}",
        acv.ua_idx, acv.ub_idx, acv.uc_idx, acv.un_idx
    )
}

/// 格式化发电机机端电压通道为 `Ua, Ub, Uc`（规范 §1.6 `TV_CHNS` 仅 3 字段，无 `Un`）。
fn fmt_acv3(acv: &AcvChn) -> String {
    format!("{}, {}, {}", acv.ua_idx, acv.ub_idx, acv.uc_idx)
}

/// 格式化 4 字段极性 TA 通道为 `Ia, Ib, Ic, 极性`（变压器侧 / 发电机 / 励磁机共用）。
fn fmt_acc_polarity(bran: &AccBran) -> String {
    format!(
        "{}, {}, {}, {}",
        bran.ia_idx, bran.ib_idx, bran.ic_idx, bran.dir
    )
}

/// 格式化 `BRANCH_NUM=z1, z2, z3`
fn fmt_branch_num(b: &BranchNum) -> String {
    format!("{}, {}, {}", b.z1, b.z2, b.z3)
}

/// 格式化 `Ufe=额定, 空载`
fn fmt_ufe(u: &Ufe) -> String {
    format!("{}, {}", u.rated, u.no_load)
}

/// 格式化 `X=Xd, Xq, Xd', Xs`
fn fmt_sync_reactance(x: &SyncReactance) -> String {
    format!("{}, {}, {}, {}", x.xd, x.xq, x.xd_prime, x.xs)
}

/// 格式化 `Ufe_CHNS=Ufe, +Ufe, -Ufe`
fn fmt_ufe_chns(u: &UfeChns) -> String {
    format!("{}, {}, {}", u.ufe, u.pos, u.neg)
}

/// 格式化 `UN_CHNS=机端, 中性点, 纵向`
fn fmt_un_chns(u: &UnChns) -> String {
    format!("{}, {}, {}", u.terminal, u.neutral, u.longitudinal)
}

/// 偏离 Python 基线：设备段所在区域是 `ZYHD`（私有扩展区），不是 `Public`。
/// 原实现写 `Public`，导致重新解析时段头区域名与源文件不符。
const ZYHD: &str = "ZYHD";

pub fn build_bus_section(bus: &Bus) -> Section {
    // 母线段的 Key 集合以样本文件为准（规范 PDF 未定义母线段）。
    // `RATED_VALUE` 单位为 V，模型以 kV 存储，写出时换算回去。
    let mut fields = vec![
        ("DEV_ID".to_string(), fmt_dev_id(bus.idx, &bus.name)),
        ("SYS_ID".to_string(), bus.sys_id.clone()),
        (
            "RATED_VALUE".to_string(),
            format!("{}V", bus.v_rtg * 1000.0),
        ),
        ("TV_CHNS".to_string(), fmt_acv(&bus.acv)),
    ];
    // `TV_POS` 可缺省（样本中 Bus_#6 / #7 就没有该项），空值不写出以保 round-trip
    if !bus.tv_pos.is_empty() {
        fields.push(("TV_POS".to_string(), bus.tv_pos.clone()));
    }
    if !bus.sta_chns.is_empty() {
        fields.push(("STATUS_CHNS".to_string(), fmt_num_list(&bus.sta_chns)));
    }

    Section {
        area: ZYHD.into(),
        kind: SectionKind::Bus,
        index: bus.idx,
        fields,
        raw: String::new(),
    }
}

pub fn build_line_section(line: &Line) -> Section {
    // Key 顺序对齐规范 §1.4
    let mut fields = vec![
        ("DEV_ID".to_string(), fmt_dev_id(line.idx, &line.name)),
        ("SYS_ID".to_string(), line.sys_id.clone()),
        ("OBJECT_TYPE".to_string(), line.object_type.clone()),
        ("LENGTH".to_string(), format!("{}(km)", line.line_len)),
        (
            "RX".to_string(),
            format!(
                "{}(Ω/km), {}(Ω/km), {}(Ω/km), {}(Ω/km)",
                line.impedance.r1, line.impedance.x1, line.impedance.r0, line.impedance.x0
            ),
        ),
        // 位置序 `C1,G1,C0,G0`（规范 §1.4），与 DMF 的属性序不同
        (
            "CG".to_string(),
            format!(
                "{}(μf/km), {}(S/km), {}(μf/km), {}(S/km)",
                line.capacitance.c1, line.capacitance.g1, line.capacitance.c0, line.capacitance.g0
            ),
        ),
        (
            "MRX".to_string(),
            format!(
                "{}(Ω/km), {}(Ω/km)",
                line.mutual_inductance.mr0, line.mutual_inductance.mx0
            ),
        ),
        // `None` 按规范写 `NO`（无并联电抗器）
        (
            "REACTOR".to_string(),
            match line.reactor {
                Some(v) => format!("{}(Ω)", v),
                None => "NO".to_string(),
            },
        ),
    ];

    if !line.other_id.is_empty() {
        fields.insert(1, ("OTHER_ID".to_string(), line.other_id.clone()));
    }

    // 电流分支：第一组写 `TA_CHNS`，第二组写 `TA_CHNS_#2`
    for (i, bran) in line.currents.iter().enumerate() {
        let key = if i == 0 {
            "TA_CHNS".to_string()
        } else {
            format!("TA_CHNS_#{}", i + 1)
        };
        fields.push((
            key,
            format!(
                "{}, {}, {}, {}, {}",
                bran.ia_idx, bran.ib_idx, bran.ic_idx, bran.in_idx, bran.dir
            ),
        ));
    }

    // 电压通道组：第一组写 `TV_CHNS`，其余写 `TV_CHNS_#n`
    for (i, acv) in line.voltages.iter().enumerate() {
        let key = if i == 0 {
            "TV_CHNS".to_string()
        } else {
            format!("TV_CHNS_#{}", i + 1)
        };
        fields.push((key, fmt_acv(acv)));
    }

    if !line.oth_achns.is_empty() {
        fields.push(("OTH_ACHNS".to_string(), fmt_num_list(&line.oth_achns)));
    }
    if !line.sta_chns.is_empty() {
        fields.push(("STATUS_CHNS".to_string(), fmt_num_list(&line.sta_chns)));
    }

    Section {
        area: ZYHD.into(),
        kind: SectionKind::Line,
        index: line.idx,
        fields,
        raw: String::new(),
    }
}

pub fn build_transformer_section(tr: &Transformer) -> Section {
    // Key 顺序对齐规范 §1.5
    let mut fields = vec![
        ("DEV_ID".to_string(), fmt_dev_id(tr.idx, &tr.name)),
        ("SYS_ID".to_string(), tr.sys_id.clone()),
        ("OBJECT_TYPE".to_string(), tr.object_type.clone()),
        ("CAPACITY".to_string(), format!("{}(MVA)", tr.pwr_rtg)),
    ];
    if !tr.ta_self_comp.is_empty() {
        fields.push(("TA_SELF_COMP".to_string(), tr.ta_self_comp.clone()));
    }
    fields.push(("WINDING_NUM".to_string(), tr.winding_num.to_string()));

    // 先写各侧 `_PARAM`，再写全部 `TA_Id_#N`，最后写各侧 `_TV_CHNS`——与样本顺序一致
    for w in &tr.windings {
        if let Some(loc) = w.location_kind() {
            fields.push((
                format!("{}_PARAM", loc.inf_prefix()),
                format!("{}, {:.3}(KV), {}", w.wg, w.v_rtg, w.bran_num),
            ));
        }
    }
    for w in &tr.windings {
        let Some(loc) = w.location_kind() else {
            continue;
        };
        let slots = loc.ta_id_indices();
        for (i, bran) in w.currents.iter().enumerate() {
            // 分支序号超出该侧可用 `TA_Id_#N` 槽位时丢弃，避免写到别的侧上
            let Some(slot) = slots.get(i) else { break };
            fields.push((
                format!("TA_Id_#{}", slot),
                // 变压器侧只有三相 + 极性符号，无零序通道（规范 §1.5）
                format!(
                    "{}, {}, {}, {}",
                    bran.ia_idx, bran.ib_idx, bran.ic_idx, bran.dir
                ),
            ));
        }
    }
    for w in &tr.windings {
        let Some(loc) = w.location_kind() else {
            continue;
        };
        let p = loc.inf_prefix();
        if !w.acv.is_empty() {
            fields.push((format!("{}_TV_CHNS", p), fmt_acv(&w.acv)));
        }
        if w.ta_zs != 0 {
            fields.push((format!("{}_TA_ZS", p), w.ta_zs.to_string()));
        }
        if w.ta_zs_gap != 0 {
            fields.push((format!("{}_TA_ZS_GAP", p), w.ta_zs_gap.to_string()));
        }
        if !w.sta_chns.is_empty() {
            fields.push((format!("{}_STATUS_CHNS", p), fmt_num_list(&w.sta_chns)));
        }
    }

    if !tr.oth_achns.is_empty() {
        fields.push(("OTH_ACHNS".to_string(), fmt_num_list(&tr.oth_achns)));
    }
    if !tr.sta_chns.is_empty() {
        fields.push(("STATUS_CHNS".to_string(), fmt_num_list(&tr.sta_chns)));
    }

    Section {
        area: ZYHD.into(),
        kind: SectionKind::Transformer,
        index: tr.idx,
        fields,
        raw: String::new(),
    }
}

pub fn build_generator_section(g: &Generator) -> Section {
    // Key 顺序对齐规范 §1.6 `[ZYHD POWER_#n]`
    let mut fields = vec![
        ("DEV_ID".to_string(), fmt_dev_id(g.idx, &g.name)),
        ("SYS_ID".to_string(), g.sys_id.clone()),
    ];
    if !g.trm_id.is_empty() {
        fields.push(("TRM_ID".to_string(), g.trm_id.clone()));
    }
    fields.push(("OBJECT_TYPE".to_string(), g.object_type.clone()));
    fields.push(("FREQ".to_string(), format!("{}(Hz)", g.freq)));
    fields.push(("CAPACITY".to_string(), format!("{}(MW)", g.capacity)));
    fields.push(("FACTOR".to_string(), g.factor.to_string()));
    fields.push(("V1".to_string(), format!("{}(KV)", g.v1)));
    fields.push(("BRANCH_NUM".to_string(), fmt_branch_num(&g.branch_num)));
    fields.push(("Rotor_I".to_string(), format!("{}(A)", g.rotor_i)));
    fields.push(("Rotor_V2".to_string(), format!("{}(mV)", g.rotor_v2)));
    fields.push(("Ufe".to_string(), fmt_ufe(&g.ufe)));
    fields.push(("X".to_string(), fmt_sync_reactance(&g.x)));
    fields.push(("EXCITATION_MODE".to_string(), g.excitation_mode.to_string()));
    fields.push(("IGT_DIR".to_string(), g.igt_dir.to_string()));
    fields.push(("TV_CHNS".to_string(), fmt_acv3(&g.acv)));
    fields.push(("TA_CHNS".to_string(), fmt_acc_polarity(&g.ta)));
    // 中性点三分支组：只在非空时写出（保 round-trip）
    if !g.ta_z1.is_empty() {
        fields.push(("TA_Z1_CHNS".to_string(), fmt_acc_polarity(&g.ta_z1)));
    }
    if !g.ta_z2.is_empty() {
        fields.push(("TA_Z2_CHNS".to_string(), fmt_acc_polarity(&g.ta_z2)));
    }
    if !g.ta_z3.is_empty() {
        fields.push(("TA_Z3_CHNS".to_string(), fmt_acc_polarity(&g.ta_z3)));
    }
    let ufe_empty = g.ufe_chns.ufe == 0 && g.ufe_chns.pos == 0 && g.ufe_chns.neg == 0;
    if !ufe_empty {
        fields.push(("Ufe_CHNS".to_string(), fmt_ufe_chns(&g.ufe_chns)));
    }
    if g.ife_chn != 0 {
        fields.push(("Ife_CHN".to_string(), g.ife_chn.to_string()));
    }
    if g.un_chns.terminal != 0 || g.un_chns.neutral != 0 || g.un_chns.longitudinal != 0 {
        fields.push(("UN_CHNS".to_string(), fmt_un_chns(&g.un_chns)));
    }
    if g.ta_ido_chn != 0 {
        fields.push(("TA_Ido_CHN".to_string(), g.ta_ido_chn.to_string()));
    }
    if !g.oth_achns.is_empty() {
        fields.push(("OTH_ACHNS".to_string(), fmt_num_list(&g.oth_achns)));
    }
    if !g.sta_chns.is_empty() {
        fields.push(("STATUS_CHNS".to_string(), fmt_num_list(&g.sta_chns)));
    }

    Section {
        area: ZYHD.into(),
        kind: SectionKind::Generator,
        index: g.idx,
        fields,
        raw: String::new(),
    }
}

pub fn build_exciter_section(e: &Exciter) -> Section {
    // Key 顺序对齐规范 §1.7 `[ZYHD EXCITATION_#n]`
    let mut fields = vec![
        ("DEV_ID".to_string(), fmt_dev_id(e.idx, &e.name)),
        ("SYS_ID".to_string(), e.sys_id.clone()),
    ];
    if !e.pwr_id.is_empty() {
        fields.push(("PWR_ID".to_string(), e.pwr_id.clone()));
    }
    fields.push(("OBJECT_TYPE".to_string(), e.object_type.clone()));
    fields.push(("FREQ".to_string(), format!("{}(Hz)", e.freq)));
    fields.push(("V1".to_string(), format!("{}(KV)", e.v1)));
    fields.push(("TV_CHNS".to_string(), fmt_acv(&e.acv)));
    fields.push(("TA_CHNS".to_string(), fmt_acc_polarity(&e.ta)));
    if !e.ta_z.is_empty() {
        fields.push(("TA_Z_CHNS".to_string(), fmt_acc_polarity(&e.ta_z)));
    }
    if !e.oth_achns.is_empty() {
        fields.push(("OTH_ACHNS".to_string(), fmt_num_list(&e.oth_achns)));
    }
    if !e.sta_chns.is_empty() {
        fields.push(("STATUS_CHNS".to_string(), fmt_num_list(&e.sta_chns)));
    }

    Section {
        area: ZYHD.into(),
        kind: SectionKind::Exciter,
        index: e.idx,
        fields,
        raw: String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_num_with_unit() {
        assert_eq!(parse_num_with_unit("22.45(km)"), Some(22.45));
        assert_eq!(parse_num_with_unit("220000V"), Some(220000.0));
        assert_eq!(parse_num_with_unit("0.071(Ω/km)"), Some(0.071));
        assert_eq!(parse_num_with_unit("180(MVA)"), Some(180.0));
        // 负号必须保留：REACTOR=-1(Ω) 是有效值
        assert_eq!(parse_num_with_unit("-1(Ω)"), Some(-1.0));
        assert_eq!(parse_num_with_unit(" 110.000(KV) "), Some(110.0));
        assert_eq!(parse_num_with_unit("1.5e3Hz"), Some(1500.0));
        // 非数值开头返回 None，不能误当 0
        assert_eq!(parse_num_with_unit("NO"), None);
        assert_eq!(parse_num_with_unit(""), None);
        assert_eq!(parse_num_with_unit("Y12"), None);
    }

    #[test]
    fn test_parse_num_list_keeps_zeros_and_order() {
        // 关键回归：Python 基线的"严格递增"过滤会把 0 和其后的项吃掉
        assert_eq!(parse_num_list("41, 42, 43, 0, 1"), vec![41, 42, 43, 0, 1]);
        assert_eq!(parse_num_list("136, 138, 140, 0"), vec![136, 138, 140, 0]);
        // 乱序也必须原样保留
        assert_eq!(parse_num_list("33, 38, 34, 39"), vec![33, 38, 34, 39]);
        assert_eq!(parse_num_list(""), Vec::<usize>::new());
    }

    #[test]
    fn test_parse_line_ta_five_fields() {
        // 线路：Ia,Ib,Ic,Io,DIR
        let b = parse_line_ta("41, 42, 43, 0, 1", 1);
        assert_eq!((b.ia_idx, b.ib_idx, b.ic_idx, b.in_idx), (41, 42, 43, 0));
        assert_eq!(b.dir, 1);
        assert_eq!(b.bran_idx, 1);

        // DIR 缺省时为正方向
        let b = parse_line_ta("41, 42, 43, 44", 2);
        assert_eq!(b.in_idx, 44, "第 4 项是零序电流通道，不是方向");
        assert_eq!(b.dir, 1);
        assert_eq!(b.bran_idx, 2);

        // 反方向
        let b = parse_line_ta("41, 42, 43, 0, -1", 1);
        assert_eq!(b.dir, -1);
    }

    #[test]
    fn test_parse_transformer_ta_four_fields() {
        // 变压器：Ia,Ib,Ic,极性符号 —— 第 4 项不是零序通道
        let b = parse_transformer_ta("1, 2, 3, 1", 1);
        assert_eq!((b.ia_idx, b.ib_idx, b.ic_idx), (1, 2, 3));
        assert_eq!(b.in_idx, 0, "变压器侧无零序电流通道");
        assert_eq!(b.dir, 1);

        let b = parse_transformer_ta("29, 30, 31, -1", 1);
        assert_eq!(b.dir, -1, "极性符号 -1 表示电流流出变压器");
        assert_eq!(b.in_idx, 0);
    }

    #[test]
    fn test_split_dev_id() {
        assert_eq!(split_dev_id("7,220kV 母线A", 1), (7, "220kV 母线A".into()));
        // 名字里含逗号：只按首个逗号切分
        assert_eq!(split_dev_id("20,主变A,备用", 1), (20, "主变A,备用".into()));
        // 缺 idx 时回落到节序号
        assert_eq!(split_dev_id(",线路名", 5), (5, "线路名".into()));
        assert_eq!(split_dev_id("", 3), (3, String::new()));
        assert_eq!(split_dev_id("仅名字", 3), (3, "仅名字".into()));
        assert_eq!(
            split_dev_id("0,零号", 4),
            (4, "零号".into()),
            "idx=0 非法，回落"
        );
    }

    #[test]
    fn test_parse_reactor() {
        assert_eq!(parse_reactor("NO"), None);
        assert_eq!(parse_reactor("no"), None);
        assert_eq!(parse_reactor(""), None);
        // 负补偿电抗无物理意义，按"无"处理
        assert_eq!(parse_reactor("-1(Ω)"), None);
        // 0 是有效值，不能塌成 None
        assert_eq!(parse_reactor("0(Ω)"), Some(0.0));
        assert_eq!(parse_reactor("330.5(Ω)"), Some(330.5));
    }

    #[test]
    fn test_parse_capacitance_field_order() {
        // 规范 §1.4：CG=C1,G1,C0,G0（容/导交替），不是 c1,c0,g1,g0
        let c = parse_capacitance("1.1(μf/km), 2.2(S/km), 3.3(μf/km), 4.4(S/km)");
        assert_eq!((c.c1, c.g1, c.c0, c.g0), (1.1, 2.2, 3.3, 4.4));
    }

    #[test]
    fn test_parse_impedance_and_mutual() {
        let z = parse_impedance("0.071(Ω/km), 0.44(Ω/km), 0.351(Ω/km), 1.324(Ω/km)");
        assert_eq!((z.r1, z.x1, z.r0, z.x0), (0.071, 0.44, 0.351, 1.324));
        let m = parse_mutual("0.05(Ω/km), 0.6(Ω/km)");
        assert_eq!((m.mr0, m.mx0), (0.05, 0.6));
    }

    #[test]
    fn test_parse_winding_param() {
        // 第一字段是字符串代号，不是数值
        assert_eq!(
            parse_winding_param("Y12, 110.000(KV), 1"),
            ("Y12".into(), 110.0, 1)
        );
        assert_eq!(
            parse_winding_param("D11, 10.000(KV), 2"),
            ("D11".into(), 10.0, 2)
        );
        assert_eq!(
            parse_winding_param("yn0, 35(KV), 1"),
            ("yn0".into(), 35.0, 1)
        );
    }

    #[test]
    fn test_winding_layout_and_ta_slots() {
        use WindingLocation::*;
        // 三卷变：高/中/低
        assert_eq!(winding_layout(3), &[High, Medium, Low]);
        // 两卷变：高/低，**缺中压侧**
        assert_eq!(winding_layout(2), &[High, Low]);
        // 自耦变当前降级为两卷变
        assert_eq!(winding_layout(1), &[High, Low]);

        // 每侧可有两路（低压侧三路）分支电流
        assert_eq!(High.ta_id_indices(), &[1, 2]);
        assert_eq!(Medium.ta_id_indices(), &[3, 4]);
        assert_eq!(Low.ta_id_indices(), &[5, 6, 7]);
    }

    #[test]
    fn test_build_winding_skips_absent_side() {
        // 两卷变：只有 H_PARAM / L_PARAM 与 TA_Id_#1 / #5，中压侧应为 None
        let section = Section {
            area: "ZYHD".into(),
            kind: SectionKind::Transformer,
            index: 1,
            fields: vec![
                ("H_PARAM".into(), "Y, 220.000(KV), 1".into()),
                ("L_PARAM".into(), "D11, 35.000(KV), 1".into()),
                ("TA_Id_#1".into(), "1, 2, 3, 1".into()),
                ("TA_Id_#5".into(), "29, 30, 31, -1".into()),
            ],
            raw: String::new(),
        };
        assert!(build_winding(&section, WindingLocation::High).is_some());
        assert!(
            build_winding(&section, WindingLocation::Medium).is_none(),
            "中压侧无 PARAM 也无 TA_Id，不应构建空绕组"
        );
        let low = build_winding(&section, WindingLocation::Low).unwrap();
        assert_eq!(low.wg, "D11");
        assert_eq!(low.currents[0].dir, -1);
    }

    #[test]
    fn test_build_winding_second_branch() {
        // 高压侧两路分支：TA_Id_#1 与 TA_Id_#2
        let section = Section {
            area: "ZYHD".into(),
            kind: SectionKind::Transformer,
            index: 1,
            fields: vec![
                ("H_PARAM".into(), "Y, 220.000(KV), 2".into()),
                ("TA_Id_#1".into(), "1, 2, 3, 1".into()),
                ("TA_Id_#2".into(), "4, 5, 6, 1".into()),
            ],
            raw: String::new(),
        };
        let w = build_winding(&section, WindingLocation::High).unwrap();
        assert_eq!(w.bran_num, 2);
        assert_eq!(w.currents.len(), 2);
        assert_eq!(w.currents[0].ia_idx, 1);
        assert_eq!(w.currents[1].ia_idx, 4);
        assert_eq!(w.currents[1].bran_idx, 2);
    }

    #[test]
    fn test_revision_year_all_versions() {
        // INF `Revision_Year` 与 CFG 头部同样支持六个版本号。
        // 原实现只匹配 "1999"，2001/2008/2013/2017 会被静默判成 1991。
        let build = |year: &str| -> Version {
            let inf = InfFile {
                sections: vec![Section {
                    area: "Public".into(),
                    kind: SectionKind::FileDescription,
                    index: 0,
                    fields: vec![
                        ("Station_Name".into(), "ST".into()),
                        ("Recording_Device_ID".into(), "REC".into()),
                        ("Revision_Year".into(), year.into()),
                    ],
                    raw: String::new(),
                }],
            };
            build_config(&inf, None).unwrap().header.version
        };

        for s in ["1991", "1999", "2001", "2008", "2013", "2017"] {
            assert_eq!(build(s).as_str(), s, "Revision_Year={} 应被识别", s);
        }
        // 无法识别时回落 1991
        assert_eq!(build("2020"), Version::V1991);
        assert_eq!(build(""), Version::V1991);
    }

    /// 合成发电机段（§1.6），用于解析与 round-trip 测试。
    fn sample_generator_section() -> Section {
        Section {
            area: "ZYHD".into(),
            kind: SectionKind::Generator,
            index: 1,
            fields: vec![
                ("DEV_ID".into(), "5,1号发电机".into()),
                ("SYS_ID".into(), "".into()),
                ("TRM_ID".into(), "20,主变A套".into()),
                ("OBJECT_TYPE".into(), "STEAM_TURBINE".into()),
                ("FREQ".into(), "50(Hz)".into()),
                ("CAPACITY".into(), "300(MW)".into()),
                ("FACTOR".into(), "0.85".into()),
                ("V1".into(), "20(KV)".into()),
                ("BRANCH_NUM".into(), "2, 2, 0".into()),
                ("Rotor_I".into(), "1800(A)".into()),
                ("Rotor_V2".into(), "75(mV)".into()),
                ("Ufe".into(), "100, 50".into()),
                ("X".into(), "1.8, 1.7, 0.3, 0.1".into()),
                ("EXCITATION_MODE".into(), "0".into()),
                ("IGT_DIR".into(), "1".into()),
                ("TV_CHNS".into(), "40, 41, 42".into()),
                ("TA_CHNS".into(), "43, 44, 45, 1".into()),
                ("TA_Z1_CHNS".into(), "46, 47, 48, -1".into()),
                ("Ufe_CHNS".into(), "49, 50, 51".into()),
                ("Ife_CHN".into(), "52".into()),
                ("UN_CHNS".into(), "53, 54, 55".into()),
                ("TA_Ido_CHN".into(), "56".into()),
                ("OTH_ACHNS".into(), "57, 58".into()),
                ("STATUS_CHNS".into(), "100, 101".into()),
            ],
            raw: String::new(),
        }
    }

    #[test]
    fn test_build_generator_parses_all_fields() {
        let section = sample_generator_section();
        let inf = InfFile {
            sections: vec![section],
        };
        let eg = build_equipment_group(&inf);
        assert_eq!(eg.generators.len(), 1);
        let g = &eg.generators[0];
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
        // TV_CHNS 仅 3 字段
        assert_eq!((g.acv.ua_idx, g.acv.ub_idx, g.acv.uc_idx), (40, 41, 42));
        assert_eq!(g.acv.un_idx, 0);
        // TA_CHNS 4 字段极性
        assert_eq!((g.ta.ia_idx, g.ta.ib_idx, g.ta.ic_idx), (43, 44, 45));
        assert_eq!(g.ta.in_idx, 0);
        assert_eq!(g.ta.dir, 1);
        // TA_Z1_CHNS 反极性
        assert_eq!(g.ta_z1.dir, -1);
        assert_eq!(g.ta_z1.bran_idx, 1);
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
        assert_eq!(g.oth_achns, vec![57, 58]);
        assert_eq!(g.sta_chns, vec![100, 101]);
    }

    #[test]
    fn test_generator_section_round_trip() {
        let section = sample_generator_section();
        let inf = InfFile {
            sections: vec![section],
        };
        let eg = build_equipment_group(&inf);
        let rebuilt = build_generator_section(&eg.generators[0]);
        let eg2 = build_equipment_group(&InfFile {
            sections: vec![rebuilt],
        });
        let g1 = &eg.generators[0];
        let g2 = &eg2.generators[0];
        // 逐字段比对
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
    }

    /// 合成励磁机段（§1.7）。
    fn sample_exciter_section() -> Section {
        Section {
            area: "ZYHD".into(),
            kind: SectionKind::Exciter,
            index: 1,
            fields: vec![
                ("DEV_ID".into(), "6,1号励磁机".into()),
                ("SYS_ID".into(), "".into()),
                ("PWR_ID".into(), "5,1号发电机".into()),
                ("OBJECT_TYPE".into(), "PRIMARY".into()),
                ("FREQ".into(), "100(Hz)".into()),
                ("V1".into(), "0.5(KV)".into()),
                ("TV_CHNS".into(), "60, 61, 62, 63".into()),
                ("TA_CHNS".into(), "64, 65, 66, 1".into()),
                ("TA_Z_CHNS".into(), "67, 68, 69, -1".into()),
                ("OTH_ACHNS".into(), "70".into()),
                ("STATUS_CHNS".into(), "102, 103".into()),
            ],
            raw: String::new(),
        }
    }

    #[test]
    fn test_build_exciter_parses_all_fields() {
        let section = sample_exciter_section();
        let inf = InfFile {
            sections: vec![section],
        };
        let eg = build_equipment_group(&inf);
        assert_eq!(eg.exciters.len(), 1);
        let e = &eg.exciters[0];
        assert_eq!(e.idx, 6);
        assert_eq!(e.name, "1号励磁机");
        assert_eq!(e.pwr_id, "5,1号发电机");
        assert_eq!(e.object_type, "PRIMARY");
        assert_eq!(e.freq, 100.0);
        assert_eq!(e.v1, 0.5);
        // TV_CHNS 4 字段
        assert_eq!(
            (e.acv.ua_idx, e.acv.ub_idx, e.acv.uc_idx, e.acv.un_idx),
            (60, 61, 62, 63)
        );
        assert_eq!((e.ta.ia_idx, e.ta.ib_idx, e.ta.ic_idx), (64, 65, 66));
        assert_eq!(e.ta.dir, 1);
        assert_eq!(e.ta_z.dir, -1);
        assert_eq!(e.oth_achns, vec![70]);
        assert_eq!(e.sta_chns, vec![102, 103]);
    }

    #[test]
    fn test_exciter_section_round_trip() {
        let section = sample_exciter_section();
        let inf = InfFile {
            sections: vec![section],
        };
        let eg = build_equipment_group(&inf);
        let rebuilt = build_exciter_section(&eg.exciters[0]);
        let eg2 = build_equipment_group(&InfFile {
            sections: vec![rebuilt],
        });
        let e1 = &eg.exciters[0];
        let e2 = &eg2.exciters[0];
        assert_eq!(e1.idx, e2.idx);
        assert_eq!(e1.name, e2.name);
        assert_eq!(e1.pwr_id, e2.pwr_id);
        assert_eq!(e1.object_type, e2.object_type);
        assert_eq!(e1.freq, e2.freq);
        assert_eq!(e1.v1, e2.v1);
        assert_eq!(e1.acv, e2.acv);
        assert_eq!(e1.ta, e2.ta);
        assert_eq!(e1.ta_z, e2.ta_z);
        assert_eq!(e1.oth_achns, e2.oth_achns);
        assert_eq!(e1.sta_chns, e2.sta_chns);
    }
}
