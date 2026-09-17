//! INF 设备段解析：构建设备组（Bus/Line/Transformer/Generator/Exciter）。

use cbase::equipment::{
    AccBran, AcvChn, BranchNum, Bus, Capacitance, EquipmentGroup, Exciter, Generator, Igap,
    Impedance, Line, MutualInductance, Px, SyncReactance, Transformer, TransformerWinding, Ufe,
    UfeChns, UnChns, WindingLocation,
};

use super::section::{InfFile, Section, SectionKind};

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
}
