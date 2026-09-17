//! INF 写侧构建器：把 Config / 设备组序列化为 INI 节（Section）。

use cbase::equipment::{
    AccBran, AcvChn, BranchNum, Bus, Exciter, Generator, Line, SyncReactance, Transformer, Ufe,
    UfeChns, UnChns,
};
use cbase::time;
use cfg::{AnalogChannel, Config, StatusChannel};

use super::section::{Section, SectionKind};

// ========== 通道与配置段的序列化 ==========

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
    use crate::inf::equipment::build_equipment_group as build_eq;
    use crate::InfFile;

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
        let eg = build_eq(&inf);
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
        let eg = build_eq(&inf);
        let rebuilt = build_generator_section(&eg.generators[0]);
        let eg2 = build_eq(&InfFile {
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
        let eg = build_eq(&inf);
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
        let eg = build_eq(&inf);
        let rebuilt = build_exciter_section(&eg.exciters[0]);
        let eg2 = build_eq(&InfFile {
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
