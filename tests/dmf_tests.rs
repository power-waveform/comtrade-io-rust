//! DMF 设备模型文件集成测试

use comtrade_io::{DmfFile, WindingLocation};

fn data_path(name: &str) -> String {
    format!("tests/data/{}", name)
}

#[test]
fn test_parse_binary_1999_dmf() {
    let text = std::fs::read_to_string(data_path("binary_1999.dmf")).unwrap();
    let dmf = DmfFile::from_str(&text).unwrap();
    assert!(!dmf.station_name.is_empty());
    assert!(!dmf.analogs.is_empty() || !dmf.statuses.is_empty());
}

#[test]
fn test_parse_ascii_1999_dmf() {
    let text = std::fs::read_to_string(data_path("ascii_1999.dmf")).unwrap();
    let dmf = DmfFile::from_str(&text).unwrap();
    assert!(!dmf.station_name.is_empty());
}

#[test]
fn test_dmf_round_trip() {
    let text = std::fs::read_to_string(data_path("binary_1999.dmf")).unwrap();
    let dmf = DmfFile::from_str(&text).unwrap();
    let serialized = dmf.to_string();
    let reparsed = DmfFile::from_str(&serialized).unwrap();
    assert_eq!(reparsed.station_name, dmf.station_name);
    assert_eq!(reparsed.analogs.len(), dmf.analogs.len());
    assert_eq!(reparsed.statuses.len(), dmf.statuses.len());
}

#[test]
fn test_dmf_to_equipment_group() {
    let text = std::fs::read_to_string(data_path("binary_1999.dmf")).unwrap();
    let dmf = DmfFile::from_str(&text).unwrap();
    let eg = dmf.to_equipment_group();

    assert_eq!(eg.buses.len(), 2, "应识别 2 条母线");
    assert_eq!(eg.lines.len(), 3, "应识别 3 条线路");
    assert_eq!(eg.transformers.len(), 1, "应识别 1 台变压器");

    // 母线：ACVChn 子元素必须挂到所属母线上
    let b = &eg.buses[0];
    assert_eq!(b.idx, 1);
    assert_eq!(b.name, "220kV母线U");
    assert_eq!(b.tv_pos, "BUS");
    assert_eq!(
        (b.acv.ua_idx, b.acv.ub_idx, b.acv.uc_idx, b.acv.un_idx),
        (1, 2, 3, 4)
    );
    assert_eq!(
        (
            eg.buses[1].acv.ua_idx,
            eg.buses[1].acv.ub_idx,
            eg.buses[1].acv.uc_idx,
            eg.buses[1].acv.un_idx
        ),
        (5, 6, 7, 8),
        "第二条母线不应复用第一条的通道"
    );

    // 线路：RX / ACC_Bran / AnaChn / StaChn 均应解析
    let l = &eg.lines[0];
    assert_eq!(l.idx, 1);
    assert_eq!(l.name, "hh1x");
    assert_eq!(l.line_len, 23.79);
    assert_eq!(
        (
            l.impedance.r1,
            l.impedance.x1,
            l.impedance.r0,
            l.impedance.x0
        ),
        (0.0469, 0.1525, 0.1289, 0.2825)
    );
    assert_eq!(l.currents.len(), 1);
    let c = &l.currents[0];
    // `bran_idx` 属性名须读写一致，否则二次解析归零
    assert_eq!(c.bran_idx, 1);
    assert_eq!((c.ia_idx, c.ib_idx, c.ic_idx, c.in_idx), (17, 18, 19, 20));
    assert_eq!(c.dir, 1, "dir=POS 应归一化为 +1");
    assert_eq!(c.dir_raw, "POS", "原始拼写应保留");
    assert_eq!(l.ana_chns, vec![17, 18, 19, 20]);
    assert_eq!(l.sta_chns.len(), 13);

    // 变压器绕组：ACVChn / ACC_Bran 必须按所属绕组分派，不能串到别的绕组上
    let t = &eg.transformers[0];
    assert_eq!(t.name, "1号主变");
    assert_eq!(t.windings.len(), 2, "样本为两卷变，高/低两侧");
    let locs: Vec<&str> = t.windings.iter().map(|w| w.location.as_str()).collect();
    assert_eq!(locs, vec!["High", "Low"]);
    assert_eq!(
        (t.windings[0].acv.ua_idx, t.windings[0].currents[0].ia_idx),
        (1, 21),
        "高压侧电压/电流通道"
    );
    assert_eq!(
        (t.windings[1].acv.ua_idx, t.windings[1].currents[0].ia_idx),
        (5, 49),
        "低压侧电压/电流通道"
    );
}

#[test]
fn test_dmf_ascii_to_equipment_group() {
    let text = std::fs::read_to_string(data_path("ascii_1999.dmf")).unwrap();
    let dmf = DmfFile::from_str(&text).unwrap();
    let eg = dmf.to_equipment_group();

    assert_eq!(eg.buses.len(), 5);
    assert_eq!(eg.lines.len(), 19);
    assert_eq!(eg.transformers.len(), 1);

    // 该样本的母线带 VRtg / VRtgSnd / is_location
    let b = &eg.buses[0];
    assert_eq!(b.v_rtg, 220.0);
    assert_eq!(b.v_rtg_snd, 100.0);
    assert_eq!(b.is_location, "0");

    let l = &eg.lines[0];
    assert_eq!(l.idx, 5);
    assert_eq!(l.impedance.r1, 0.0469);

    // `wG` 是接线组别字符串代号（y0 / yn0），不是数值 —— 声明为 f64 会全部归零
    let t = &eg.transformers[0];
    assert_eq!(t.windings.len(), 2);
    let wgs: Vec<&str> = t.windings.iter().map(|w| w.wg.as_str()).collect();
    assert_eq!(wgs, vec!["y0", "yn0"]);
}

#[test]
fn test_dmf_equipment_round_trip() {
    for name in ["binary_1999.dmf", "ascii_1999.dmf"] {
        let text = std::fs::read_to_string(data_path(name)).unwrap();
        let dmf = DmfFile::from_str(&text).unwrap();
        let reparsed = DmfFile::from_str(&dmf.to_string()).unwrap();

        let a = dmf.to_equipment_group();
        let b = reparsed.to_equipment_group();
        assert_eq!(a.buses.len(), b.buses.len(), "{}: 母线数", name);
        assert_eq!(a.lines.len(), b.lines.len(), "{}: 线路数", name);
        assert_eq!(
            a.transformers.len(),
            b.transformers.len(),
            "{}: 变压器数",
            name
        );

        for (x, y) in a.buses.iter().zip(&b.buses) {
            assert_eq!(
                (x.idx, &x.name, &x.tv_pos),
                (y.idx, &y.name, &y.tv_pos),
                "{}",
                name
            );
            assert_eq!(x.acv, y.acv, "{}: 母线电压通道组", name);
        }
        for (x, y) in a.lines.iter().zip(&b.lines) {
            assert_eq!(
                (x.idx, &x.name, x.line_len),
                (y.idx, &y.name, y.line_len),
                "{}",
                name
            );
            assert_eq!(x.impedance, y.impedance, "{}: 线路阻抗", name);
            assert_eq!(x.capacitance, y.capacitance, "{}: 线路电容", name);
            assert_eq!(x.currents, y.currents, "{}: 线路电流分支", name);
            assert_eq!(x.ana_chns, y.ana_chns, "{}: 模拟量通道", name);
            assert_eq!(x.sta_chns, y.sta_chns, "{}: 开关量通道", name);
        }
        for (x, y) in a.transformers.iter().zip(&b.transformers) {
            assert_eq!(x.windings.len(), y.windings.len(), "{}: 绕组数", name);
            for (wx, wy) in x.windings.iter().zip(&y.windings) {
                assert_eq!(
                    (&wx.location, &wx.wg),
                    (&wy.location, &wy.wg),
                    "{}: 绕组位置与接线组别",
                    name
                );
                assert_eq!(wx.acv, wy.acv, "{}: 绕组电压通道组", name);
                assert_eq!(wx.currents, wy.currents, "{}: 绕组电流分支", name);
            }
        }
    }
}

#[test]
fn test_dmf_generator_exciter_round_trip() {
    // 样本 DMF 无 Generator/Exciter 元素，用合成 XML 验证 §1.6/§1.7 全链路 round-trip。
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

    let dmf1 = DmfFile::from_str(xml).unwrap();
    let text = dmf1.to_string();
    let dmf2 = DmfFile::from_str(&text).unwrap();

    let a = dmf1.to_equipment_group();
    let b = dmf2.to_equipment_group();
    assert_eq!(a.generators.len(), 1);
    assert_eq!(b.generators.len(), 1);
    assert_eq!(a.exciters.len(), 1);
    assert_eq!(b.exciters.len(), 1);

    let g1 = &a.generators[0];
    let g2 = &b.generators[0];
    assert_eq!((g1.idx, &g1.name), (g2.idx, &g2.name));
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

    let e1 = &a.exciters[0];
    let e2 = &b.exciters[0];
    assert_eq!((e1.idx, &e1.name), (e2.idx, &e2.name));
    assert_eq!(e1.pwr_id, e2.pwr_id);
    assert_eq!(e1.object_type, e2.object_type);
    assert_eq!(e1.freq, e2.freq);
    assert_eq!(e1.v1, e2.v1);
    assert_eq!(e1.acv, e2.acv);
    assert_eq!(e1.ta, e2.ta);
    assert_eq!(e1.ta_z, e2.ta_z);
}

#[test]
fn test_dmf_dl_t553_standard_elements() {
    // 依据 DL/T 553-2013 附录 B：标准 DMF 使用 Rec_ref、Common 绕组、
    // Generator/Exciter 的标准属性名及子元素。
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<scl:ComtradeModel station_name="S" version="1.0" reference="1" rec_dev_name="R" Rec_ref="IED/RDRE1">
    <scl:AnalogChannel idx_cfg="1" idx_org="11" type="D" flag="AMP" p_min="-5" p_max="5" s_min="-1" s_max="1" freq="0" au="2" bu="3" sIUnit="A" multiplier="k" primary="1000" secondary="1" ps="P"/>
    <scl:Transformer idx="3" trm_name="AT1" srcRef="T/ref" pwrRtg="240">
        <scl:TransformerWinding location="Common" srcRef="CW/ref" VRtg="110" ARtg="1200" bran_num="1" bus_ID="9" wG="yn0">
            <scl:ACC_Bran bran_idx="1" ia_idx="21" ib_idx="22" ic_idx="23" dir="POS"/>
        </scl:TransformerWinding>
    </scl:Transformer>
    <scl:Generator idx="5" gen_name="G1" srcRef="G/ref" sys_ID="SYS-G" trm_SID="3,AT1" type="STEAM_TURBINE" freq="50" capacity="300" factor="0.85" VRtg="20" rotor_I="1800" rotor_V2="75" neut_group_num="2" exciter_Mode="2" igt_Dir="1">
        <scl:Ufe ufe="100" no_load="50"/>
        <scl:X xd="1.8" xq="1.7" xd1="0.3" xs="0.1"/>
        <scl:ACCCChn ia_idx="31" ib_idx="32" ic_idx="33" dir="POS"/>
        <scl:ACCZChn ia_idx="34" ib_idx="35" ic_idx="36" dir="NEG"/>
        <scl:NeutGroup group_idx="2" bran_num="3" ia_idx="37" ib_idx="38" ic_idx="39" dir="POS"/>
        <scl:Ufe_Chns ufe_idx="40" posufe_idx="41" negufe_idx="42"/>
        <scl:Ife_Chn ife_idx="43"/>
        <scl:ACV_Z0 z0_idx="44" terminal_idx="45"/>
        <scl:ACV_ZZ0 zz0_idx="46"/>
        <scl:ACC_Id0 id0_idx="47"/>
    </scl:Generator>
    <scl:Exciter idx="6" exc_name="E1" srcRef="E/ref" sys_ID="SYS-E" gen_SID="5,G1" type="PRIMARY" freq="100" VRtg="0.5">
        <scl:ACVChn ua_idx="51" ub_idx="52" uc_idx="53"/>
        <scl:ACCChn ia_idx="54" ib_idx="55" ic_idx="56" dir="POS"/>
        <scl:ACCZChn ia_idx="57" ib_idx="58" ic_idx="59" dir="NEG"/>
    </scl:Exciter>
</scl:ComtradeModel>"#;

    let dmf = DmfFile::from_str(xml).unwrap();
    assert_eq!(dmf.rec_ref, "IED/RDRE1");
    assert_eq!(dmf.analogs[0].p_min, -5.0);
    assert_eq!(dmf.analogs[0].p_max, 5.0);
    assert_eq!(dmf.analogs[0].s_min, -1.0);
    assert_eq!(dmf.analogs[0].s_max, 1.0);

    let common = &dmf.transformers[0].windings[0];
    assert_eq!(common.location_kind(), Some(WindingLocation::Common));
    assert_eq!(common.location, "Common");
    assert_eq!(common.currents[0].ia_idx, 21);

    let g = &dmf.generators[0];
    assert_eq!(g.trm_id, "3,AT1");
    assert_eq!(g.object_type, "STEAM_TURBINE");
    assert_eq!(g.v1, 20.0);
    assert_eq!((g.ufe.rated, g.ufe.no_load), (100.0, 50.0));
    assert_eq!((g.x.xd, g.x.xq, g.x.xd_prime, g.x.xs), (1.8, 1.7, 0.3, 0.1));
    assert_eq!((g.ta.ia_idx, g.ta.dir), (31, 1));
    assert_eq!((g.ta_z1.ia_idx, g.ta_z1.dir), (34, -1));
    assert_eq!(g.neutral_groups.len(), 1);
    assert_eq!(g.neutral_groups[0].group_idx, 2);
    assert_eq!(g.neutral_groups[0].bran_num, 3);
    assert_eq!(g.branch_num.z2, 3);
    assert_eq!(
        (g.ufe_chns.ufe, g.ufe_chns.pos, g.ufe_chns.neg),
        (40, 41, 42)
    );
    assert_eq!(g.ife_chn, 43);
    assert_eq!(
        (
            g.un_chns.terminal,
            g.un_chns.neutral,
            g.un_chns.longitudinal
        ),
        (45, 44, 46)
    );
    assert_eq!(g.ta_ido_chn, 47);

    let e = &dmf.exciters[0];
    assert_eq!(e.pwr_id, "5,G1");
    assert_eq!(e.object_type, "PRIMARY");
    assert_eq!(e.v1, 0.5);
    assert_eq!((e.ta.ia_idx, e.ta_z.ia_idx), (54, 57));

    let text = dmf.to_string();
    assert!(text.contains(r#"Rec_ref="IED/RDRE1""#));
    assert!(text.contains("<scl:ACCCChn"));
    assert!(text.contains("<scl:NeutGroup"));
    assert!(text.contains(r#"location="Common""#));
    assert!(text.contains(r#"gen_SID="5,G1""#));

    let reparsed = DmfFile::from_str(&text).unwrap();
    assert_eq!(reparsed.rec_ref, dmf.rec_ref);
    assert_eq!(
        reparsed.transformers[0].windings[0].location_kind(),
        Some(WindingLocation::Common)
    );
    assert_eq!(
        reparsed.generators[0].neutral_groups,
        dmf.generators[0].neutral_groups
    );
    assert_eq!(reparsed.exciters[0].ta_z, dmf.exciters[0].ta_z);
}
