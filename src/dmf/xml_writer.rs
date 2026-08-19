//! DMF XML 生成器
//!
//! 元素名与属性名严格对齐样本文件（`tests/data/*.dmf`），属性顺序与样本一致，
//! 以保证 `parse → serialize → reparse` 语义等价。

use crate::dmf::DmfFile;
use crate::equipment::{AccBran, AcvChn, Igap};

/// XML 实体转义
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// 缩进（样本使用 Tab）
fn indent(level: usize) -> String {
    "\t".repeat(level)
}

/// 写出 `ACVChn` 子元素。全零时跳过，避免为未配置的设备凭空造出通道组。
fn push_acv(lines: &mut Vec<String>, level: usize, acv: &AcvChn) {
    if acv.is_empty() {
        return;
    }
    lines.push(format!(
        r#"{}<scl:ACVChn ua_idx="{}" ub_idx="{}" uc_idx="{}" un_idx="{}" ul_idx="{}"/>"#,
        indent(level),
        acv.ua_idx,
        acv.ub_idx,
        acv.uc_idx,
        acv.un_idx,
        acv.ul_idx,
    ));
}

/// 写出 `ACC_Bran` 子元素。
///
/// 偏离 Python 基线：属性名为 `bran_idx`（样本与 XSD 一致），
/// 原实现写 `idx`，导致二次解析时分支号归零。
///
/// `dir` 优先写回 `dir_raw` 原始拼写（样本中 `POS` / `pos` 大小写不一致），
/// 无原文时按归一化语义写 `POS` / `NEG`。
fn push_acc(lines: &mut Vec<String>, level: usize, bran: &AccBran) {
    let dir = if !bran.dir_raw.is_empty() {
        escape(&bran.dir_raw)
    } else if bran.dir < 0 {
        "NEG".to_string()
    } else {
        "POS".to_string()
    };
    lines.push(format!(
        r#"{}<scl:ACC_Bran bran_idx="{}" ia_idx="{}" ib_idx="{}" ic_idx="{}" in_idx="{}" dir="{}"/>"#,
        indent(level),
        bran.bran_idx,
        bran.ia_idx,
        bran.ib_idx,
        bran.ic_idx,
        bran.in_idx,
        dir,
    ));
}

fn dir_text(bran: &AccBran) -> String {
    if !bran.dir_raw.is_empty() {
        escape(&bran.dir_raw)
    } else if bran.dir < 0 {
        "NEG".to_string()
    } else {
        "POS".to_string()
    }
}

/// 写出 DL/T 553-2013 表 B.9/B.10 的三相电流通道子元素。
fn push_phase_acc(lines: &mut Vec<String>, level: usize, tag: &str, bran: &AccBran) {
    if bran.is_empty() {
        return;
    }
    lines.push(format!(
        r#"{}<scl:{} ia_idx="{}" ib_idx="{}" ic_idx="{}" dir="{}"/>"#,
        indent(level),
        tag,
        bran.ia_idx,
        bran.ib_idx,
        bran.ic_idx,
        dir_text(bran),
    ));
}

fn push_neutral_group(
    lines: &mut Vec<String>,
    level: usize,
    group_idx: usize,
    bran_num: usize,
    bran: &AccBran,
) {
    if bran_num == 0 && bran.is_empty() {
        return;
    }
    lines.push(format!(
        r#"{}<scl:NeutGroup group_idx="{}" bran_num="{}" bran_idx="{}" ia_idx="{}" ib_idx="{}" ic_idx="{}" dir="{}"/>"#,
        indent(level),
        group_idx,
        bran_num,
        bran.bran_idx,
        bran.ia_idx,
        bran.ib_idx,
        bran.ic_idx,
        dir_text(bran),
    ));
}

/// 写出 `Igap` 子元素。全零时跳过。
fn push_igap(lines: &mut Vec<String>, level: usize, igap: &Igap) {
    if igap.zgap_idx == 0 && igap.zsgap_idx == 0 {
        return;
    }
    lines.push(format!(
        r#"{}<scl:Igap zgap_idx="{}" zsgap_idx="{}"/>"#,
        indent(level),
        igap.zgap_idx,
        igap.zsgap_idx,
    ));
}

/// 写出 `AnaChn` / `StaChn` 通道引用列表
fn push_chn_refs(lines: &mut Vec<String>, level: usize, tag: &str, idxs: &[usize]) {
    for idx in idxs {
        lines.push(format!(
            r#"{}<scl:{} idx_cfg="{}"/>"#,
            indent(level),
            tag,
            idx
        ));
    }
}

/// 生成 DMF XML 文本
pub fn write_dmf(dmf: &DmfFile) -> String {
    let mut lines = Vec::new();

    // XML 声明
    lines.push(r#"<?xml version="1.0" encoding="UTF-8"?>"#.to_string());

    // 根元素
    let mut root_attrs = format!(
        r#"station_name="{}" version="{}" reference="{}" rec_dev_name="{}""#,
        escape(&dmf.station_name),
        escape(&dmf.version),
        escape(&dmf.reference),
        escape(&dmf.rec_dev_name),
    );
    if !dmf.rec_ref.is_empty() {
        root_attrs.push_str(&format!(r#" Rec_ref="{}""#, escape(&dmf.rec_ref)));
    }
    lines.push(format!(r#"<scl:ComtradeModel {}>"#, root_attrs));

    // 模拟通道
    // 偏离 Python 基线：属性名为 `idx_rlt`（样本一致），原实现写 `idx_rl`。
    // 该字段当前未建模，固定写 0。
    for ch in &dmf.analogs {
        lines.push(format!(
            r#"	<scl:AnalogChannel idx_cfg="{}" idx_org="{}" type="{}" flag="{}" p_min="{}" p_max="{}" s_min="{}" s_max="{}" freq="{}" au="{}" bu="{}" sIUnit="{}" multiplier="{}" primary="{}" secondary="{}" ps="{}" idx_rlt="0" ph="{}"/>"#,
            ch.idx_cfg,
            ch.idx_org,
            escape(&ch.ch_type),
            escape(&ch.flag),
            ch.p_min,
            ch.p_max,
            ch.s_min,
            ch.s_max,
            ch.freq,
            ch.au,
            ch.bu,
            escape(&ch.unit),
            ch.multiplier,
            ch.primary,
            ch.secondary,
            escape(&ch.ps),
            escape(&ch.ph),
        ));
    }

    // 状态通道
    for ch in &dmf.statuses {
        lines.push(format!(
            r#"	<scl:StatusChannel idx_cfg="{}" idx_org="{}" type="{}" flag="{}" contact="{}" srcRef="{}"/>"#,
            ch.idx_cfg,
            ch.idx_org,
            escape(&ch.ch_type),
            escape(&ch.flag),
            escape(&ch.contact),
            escape(&ch.src_ref),
        ));
    }

    // 母线
    //
    // 偏离 Python 基线：原实现把 `Bus` 写成自闭合标签却仍需承载 `ACVChn` 等子元素，
    // 子元素被整体丢弃。此处按有无子元素分别写自闭合 / 成对标签。
    for bus in &dmf.buses {
        let open = format!(
            r#"	<scl:Bus idx="{}" bus_name="{}" srcRef="{}" VRtg="{}" VRtgSnd="{}" VRtgSnd_Pos="{}" is_location="{}" bus_uuid="{}""#,
            bus.idx,
            escape(&bus.name),
            escape(&bus.src_ref),
            bus.v_rtg,
            bus.v_rtg_snd,
            escape(&bus.tv_pos),
            escape(&bus.is_location),
            escape(&bus.bus_uuid),
        );
        let has_children =
            !bus.acv.is_empty() || !bus.ana_chns.is_empty() || !bus.sta_chns.is_empty();
        if !has_children {
            lines.push(format!("{}/>", open));
            continue;
        }
        lines.push(format!("{}>", open));
        push_acv(&mut lines, 2, &bus.acv);
        push_chn_refs(&mut lines, 2, "AnaChn", &bus.ana_chns);
        push_chn_refs(&mut lines, 2, "StaChn", &bus.sta_chns);
        lines.push("	</scl:Bus>".to_string());
    }

    // 线路
    for line in &dmf.lines {
        let open = format!(
            r#"	<scl:Line idx="{}" line_name="{}" bus_ID="{}" srcRef="{}" VRtg="{}" ARtg="{}" ARtgSnd="{}" LinLen="{}" bran_num="{}" line_uuid="{}" remote_ID="{}" remote_Flag="{}" differential_ID="{}""#,
            line.idx,
            escape(&line.name),
            escape(&line.bus_id),
            escape(&line.src_ref),
            line.v_rtg,
            line.a_rtg,
            line.a_rtg_snd,
            line.line_len,
            line.bran_num,
            escape(&line.line_uuid),
            escape(&line.remote_id),
            escape(&line.remote_flag),
            escape(&line.differential_id),
        );
        lines.push(format!("{}>", open));

        // 子元素顺序对齐样本：RX / CG / PX / MR / ACC_Bran / ACVChn /
        // AnaChn / StaChn / DifferentialCurrent
        lines.push(format!(
            r#"		<scl:RX r1="{}" x1="{}" r0="{}" x0="{}"/>"#,
            line.impedance.r1, line.impedance.x1, line.impedance.r0, line.impedance.x0,
        ));
        // 属性名按样本为 `c1/c0/g1/g0`（与 INF 的 `CG=C1,G1,C0,G0` 位置序不同，
        // 此处按属性名写出，语义不受位置序影响）
        lines.push(format!(
            r#"		<scl:CG c1="{}" c0="{}" g1="{}" g0="{}"/>"#,
            line.capacitance.c1, line.capacitance.c0, line.capacitance.g1, line.capacitance.g0,
        ));
        lines.push(format!(
            r#"		<scl:PX px="{}" px0="{}"/>"#,
            line.px.px, line.px.px0,
        ));
        lines.push(format!(
            r#"		<scl:MR idx="{}" mr0="{}" mx0="{}"/>"#,
            line.mutual_inductance.idx, line.mutual_inductance.mr0, line.mutual_inductance.mx0,
        ));
        for bran in &line.currents {
            push_acc(&mut lines, 2, bran);
        }
        for acv in &line.voltages {
            push_acv(&mut lines, 2, acv);
        }
        push_chn_refs(&mut lines, 2, "AnaChn", &line.ana_chns);
        push_chn_refs(&mut lines, 2, "StaChn", &line.sta_chns);
        // 差动电流只有 ua/ub/uc 三相，无 un/ul
        if !line.differential_current.is_empty() {
            lines.push(format!(
                r#"		<scl:DifferentialCurrent ua_idx="{}" ub_idx="{}" uc_idx="{}"/>"#,
                line.differential_current.ua_idx,
                line.differential_current.ub_idx,
                line.differential_current.uc_idx,
            ));
        }
        lines.push("	</scl:Line>".to_string());
    }

    // 变压器
    for tr in &dmf.transformers {
        lines.push(format!(
            r#"	<scl:Transformer idx="{}" trm_name="{}" srcRef="{}" pwrRtg="{}" transformer_uuid="{}">"#,
            tr.idx,
            escape(&tr.name),
            escape(&tr.src_ref),
            tr.pwr_rtg,
            escape(&tr.transformer_uuid),
        ));
        push_chn_refs(&mut lines, 2, "AnaChn", &tr.ana_chns);
        push_chn_refs(&mut lines, 2, "StaChn", &tr.sta_chns);
        for w in &tr.windings {
            // 偏离 Python 基线：补写 `srcRef` 属性（原实现未写出，round-trip 丢失）。
            // `wG` 是绕组接线组别字符串代号（如 y0 / yn0），需转义而非数值格式化。
            let open = format!(
                r#"		<scl:TransformerWinding location="{}" srcRef="{}" VRtg="{}" ARtg="{}" bran_num="{}" bus_ID="{}" wG="{}""#,
                escape(&w.location),
                escape(&w.src_ref),
                w.v_rtg,
                w.a_rtg,
                w.bran_num,
                escape(&w.bus_id),
                escape(&w.wg),
            );
            let has_children = !w.acv.is_empty()
                || !w.currents.is_empty()
                || w.igap.zgap_idx != 0
                || w.igap.zsgap_idx != 0
                || !w.sta_chns.is_empty();
            if !has_children {
                lines.push(format!("{}/>", open));
                continue;
            }
            lines.push(format!("{}>", open));
            push_acv(&mut lines, 3, &w.acv);
            for bran in &w.currents {
                push_acc(&mut lines, 3, bran);
            }
            push_igap(&mut lines, 3, &w.igap);
            push_chn_refs(&mut lines, 3, "StaChn", &w.sta_chns);
            lines.push("		</scl:TransformerWinding>".to_string());
        }
        lines.push("	</scl:Transformer>".to_string());
    }

    // 发电机（§1.6）
    for g in &dmf.generators {
        let neut_group_num = if !g.neutral_groups.is_empty() {
            g.neutral_groups.len()
        } else {
            [g.branch_num.z1, g.branch_num.z2, g.branch_num.z3]
                .into_iter()
                .filter(|v| *v > 0)
                .count()
        };
        let open = format!(
            r#"	<scl:Generator idx="{}" gen_name="{}" srcRef="{}" sys_ID="{}" trm_SID="{}" type="{}" freq="{}" capacity="{}" factor="{}" VRtg="{}" rotor_I="{}" rotor_V2="{}" neut_group_num="{}" exciter_Mode="{}" igt_Dir="{}""#,
            g.idx,
            escape(&g.name),
            escape(&g.src_ref),
            escape(&g.sys_id),
            escape(&g.trm_id),
            escape(&g.object_type),
            g.freq,
            g.capacity,
            g.factor,
            g.v1,
            g.rotor_i,
            g.rotor_v2,
            neut_group_num,
            g.excitation_mode,
            g.igt_dir,
        );
        let has_ufe = g.ufe.rated != 0.0 || g.ufe.no_load != 0.0;
        let has_x = g.x.xd != 0.0 || g.x.xq != 0.0 || g.x.xd_prime != 0.0 || g.x.xs != 0.0;
        let has_ufe_chns = g.ufe_chns.ufe != 0 || g.ufe_chns.pos != 0 || g.ufe_chns.neg != 0;
        let has_un_chns =
            g.un_chns.terminal != 0 || g.un_chns.neutral != 0 || g.un_chns.longitudinal != 0;
        let has_children = !g.acv.is_empty()
            || !g.ta.is_empty()
            || !g.ta_z1.is_empty()
            || !g.ta_z2.is_empty()
            || !g.ta_z3.is_empty()
            || has_ufe
            || has_x
            || has_ufe_chns
            || g.ife_chn != 0
            || has_un_chns
            || g.ta_ido_chn != 0
            || !g.neutral_groups.is_empty()
            || g.branch_num.z1 != 0
            || g.branch_num.z2 != 0
            || g.branch_num.z3 != 0
            || !g.oth_achns.is_empty()
            || !g.sta_chns.is_empty();
        if !has_children {
            lines.push(format!("{}/>", open));
            continue;
        }
        lines.push(format!("{}>", open));
        if has_ufe {
            lines.push(format!(
                r#"		<scl:Ufe ufe="{}" no_load="{}"/>"#,
                g.ufe.rated, g.ufe.no_load,
            ));
        }
        if has_x {
            lines.push(format!(
                r#"		<scl:X xd="{}" xq="{}" xd1="{}" xs="{}"/>"#,
                g.x.xd, g.x.xq, g.x.xd_prime, g.x.xs,
            ));
        }
        push_acv(&mut lines, 2, &g.acv);
        push_phase_acc(&mut lines, 2, "ACCCChn", &g.ta);
        if !g.neutral_groups.is_empty() {
            for ng in &g.neutral_groups {
                push_neutral_group(&mut lines, 2, ng.group_idx, ng.bran_num, &ng.current);
            }
        } else if g.branch_num.z1 != 0 || g.branch_num.z2 != 0 || g.branch_num.z3 != 0 {
            push_neutral_group(&mut lines, 2, 1, g.branch_num.z1, &g.ta_z1);
            push_neutral_group(&mut lines, 2, 2, g.branch_num.z2, &g.ta_z2);
            push_neutral_group(&mut lines, 2, 3, g.branch_num.z3, &g.ta_z3);
        } else {
            push_phase_acc(&mut lines, 2, "ACCZChn", &g.ta_z1);
        }
        if has_ufe_chns {
            lines.push(format!(
                r#"		<scl:Ufe_Chns ufe_idx="{}" posufe_idx="{}" negufe_idx="{}"/>"#,
                g.ufe_chns.ufe, g.ufe_chns.pos, g.ufe_chns.neg,
            ));
        }
        if g.ife_chn != 0 {
            lines.push(format!(r#"		<scl:Ife_Chn ife_idx="{}"/>"#, g.ife_chn));
        }
        if g.un_chns.terminal != 0 || g.un_chns.neutral != 0 {
            lines.push(format!(
                r#"		<scl:ACV_Z0 z0_idx="{}" terminal_idx="{}"/>"#,
                g.un_chns.neutral, g.un_chns.terminal
            ));
        }
        if g.un_chns.longitudinal != 0 {
            lines.push(format!(
                r#"		<scl:ACV_ZZ0 zz0_idx="{}"/>"#,
                g.un_chns.longitudinal
            ));
        }
        if g.ta_ido_chn != 0 {
            lines.push(format!(r#"		<scl:ACC_Id0 id0_idx="{}"/>"#, g.ta_ido_chn));
        }
        push_chn_refs(&mut lines, 2, "AnaChn", &g.oth_achns);
        push_chn_refs(&mut lines, 2, "StaChn", &g.sta_chns);
        lines.push("	</scl:Generator>".to_string());
    }

    // 励磁机（§1.7）
    for e in &dmf.exciters {
        let open = format!(
            r#"	<scl:Exciter idx="{}" exc_name="{}" srcRef="{}" sys_ID="{}" gen_SID="{}" type="{}" freq="{}" VRtg="{}""#,
            e.idx,
            escape(&e.name),
            escape(&e.src_ref),
            escape(&e.sys_id),
            escape(&e.pwr_id),
            escape(&e.object_type),
            e.freq,
            e.v1,
        );
        let has_children = !e.acv.is_empty()
            || !e.ta.is_empty()
            || !e.ta_z.is_empty()
            || !e.oth_achns.is_empty()
            || !e.sta_chns.is_empty();
        if !has_children {
            lines.push(format!("{}/>", open));
            continue;
        }
        lines.push(format!("{}>", open));
        push_acv(&mut lines, 2, &e.acv);
        push_phase_acc(&mut lines, 2, "ACCChn", &e.ta);
        push_phase_acc(&mut lines, 2, "ACCZChn", &e.ta_z);
        push_chn_refs(&mut lines, 2, "AnaChn", &e.oth_achns);
        push_chn_refs(&mut lines, 2, "StaChn", &e.sta_chns);
        lines.push("	</scl:Exciter>".to_string());
    }

    lines.push("</scl:ComtradeModel>".to_string());
    lines.join("\n")
}
