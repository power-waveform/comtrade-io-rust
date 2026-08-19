//! INF 信息文件集成测试

use comtrade_io::{InfFile, SectionKind};

fn data_path(name: &str) -> String {
    format!("tests/data/{}", name)
}

#[test]
fn test_parse_binary_inf() {
    let bytes = std::fs::read(data_path("binary_inf.inf")).unwrap();
    let text = comtrade_io::encoding::decode_with(&bytes, comtrade_io::Encoding::Gbk);
    let inf = InfFile::from_str(&text);
    assert!(!inf.sections.is_empty(), "INF 应包含节");
}

#[test]
fn test_has_file_description() {
    let bytes = std::fs::read(data_path("binary_inf.inf")).unwrap();
    let text = comtrade_io::encoding::decode_with(&bytes, comtrade_io::Encoding::Gbk);
    let inf = InfFile::from_str(&text);
    let fd = inf.file_description();
    assert!(fd.is_some(), "应包含 File_Description 节");
    let fd = fd.unwrap();
    assert!(fd.get("Station_Name").is_some());
}

#[test]
fn test_from_file_preserves_utf8_chinese() {
    let path = std::env::temp_dir().join(format!("ct_utf8_{}.inf", std::process::id()));
    let text = "\
[Public File_Description]
Station_Name=中文站
Recording_Device_ID=录波器
Revision_Year=2013
Total_Channels=0
Analog_Channels=0
Status_Channels=0
Frequency=50
Nrates=1
Samp=1000
Endsamp=1
Start_Time=01/01/2023,00:00:00.000000
Trigger_Time=01/01/2023,00:00:00.000000
File_Type=ASCII
Timemult=1
";
    std::fs::write(&path, text.as_bytes()).unwrap();

    let inf = InfFile::from_file(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    let fd = inf.file_description().unwrap();

    assert_eq!(fd.get("Station_Name"), Some("中文站"));
    assert_eq!(fd.get("Recording_Device_ID"), Some("录波器"));
}

#[test]
fn test_has_analog_channels() {
    let bytes = std::fs::read(data_path("binary_inf.inf")).unwrap();
    let text = comtrade_io::encoding::decode_with(&bytes, comtrade_io::Encoding::Gbk);
    let inf = InfFile::from_str(&text);
    let analog_sections = inf.sections_of(&SectionKind::AnalogChannel);
    assert!(!analog_sections.is_empty(), "应包含模拟通道节");
}

#[test]
fn test_has_status_channels() {
    let bytes = std::fs::read(data_path("binary_inf.inf")).unwrap();
    let text = comtrade_io::encoding::decode_with(&bytes, comtrade_io::Encoding::Gbk);
    let inf = InfFile::from_str(&text);
    let status_sections = inf.sections_of(&SectionKind::StatusChannel);
    assert!(!status_sections.is_empty(), "应包含状态通道节");
}

#[test]
fn test_to_config() {
    let bytes = std::fs::read(data_path("binary_inf.inf")).unwrap();
    let text = comtrade_io::encoding::decode_with(&bytes, comtrade_io::Encoding::Gbk);
    let inf = InfFile::from_str(&text);
    let config = inf.to_config(None);
    assert!(config.is_some(), "应能从 INF 构建 Config");
    let config = config.unwrap();
    assert!(!config.header.station.is_empty());
    assert!(config.channels.analog > 0);
    assert!(config.channels.status > 0);
}

#[test]
fn test_to_equipment_group() {
    let bytes = std::fs::read(data_path("binary_inf.inf")).unwrap();
    let text = comtrade_io::encoding::decode_with(&bytes, comtrade_io::Encoding::Gbk);
    let inf = InfFile::from_str(&text);
    let eg = inf.to_equipment_group();

    assert_eq!(eg.buses.len(), 7, "应识别 7 条母线");
    assert_eq!(eg.lines.len(), 6, "应识别 6 条线路");
    assert_eq!(eg.transformers.len(), 1, "应识别 1 台变压器");

    // 母线：idx / name 来自 `DEV_ID=idx,name`，不是节序号
    let b = &eg.buses[0];
    assert_eq!(b.idx, 7, "母线 idx 应取自 DEV_ID 而非节序号 1");
    assert!(
        b.name.contains("602XX1"),
        "母线名应解析出来，实际 {:?}",
        b.name
    );
    assert_eq!(b.v_rtg, 220.0, "RATED_VALUE=220000V 应换算为 220 kV");
    assert_eq!(b.tv_pos, "Line");
    assert_eq!(
        (b.acv.ua_idx, b.acv.ub_idx, b.acv.uc_idx, b.acv.un_idx),
        (46, 47, 48, 0)
    );
    // Bus_#6 无 TV_POS 项，应为空而非误填
    assert_eq!(eg.buses[5].tv_pos, "");
    assert_eq!(
        (
            eg.buses[5].acv.ua_idx,
            eg.buses[5].acv.ub_idx,
            eg.buses[5].acv.uc_idx
        ),
        (136, 138, 140)
    );
    assert!(
        eg.buses.iter().all(|b| !b.name.is_empty()),
        "母线名不应为空"
    );

    // 线路
    let l = &eg.lines[0];
    assert_eq!(l.idx, 14);
    assert_eq!(l.object_type, "LINE");
    assert_eq!(l.line_len, 22.45, "LENGTH=22.45(km) 应剥离单位");
    assert_eq!(
        (
            l.impedance.r1,
            l.impedance.x1,
            l.impedance.r0,
            l.impedance.x0
        ),
        (0.071, 0.44, 0.351, 1.324),
        "RX 四字段按 R1,X1,R0,X0 序解析"
    );
    // `REACTOR=-1(Ω)` 表示无并联电抗器
    assert_eq!(l.reactor, None);
    // `TA_CHNS=41, 42, 43, 0, 1` —— 尾项 1 是方向标志，不能被当作通道号，
    // 通道 0 也不能被"严格递增"过滤吃掉
    assert_eq!(l.currents.len(), 1);
    let c = &l.currents[0];
    assert_eq!((c.ia_idx, c.ib_idx, c.ic_idx, c.in_idx), (41, 42, 43, 0));
    assert_eq!(c.dir, 1, "缺省/显式 1 均为正方向");
    assert_eq!(l.voltages.len(), 1);
    assert_eq!((l.voltages[0].ua_idx, l.voltages[0].uc_idx), (46, 48));
    assert_eq!(l.sta_chns.len(), 17, "STATUS_CHNS 应解析全部 17 个通道");

    // 母联：OBJECT_TYPE=BUS_CONNECTION，REACTOR=0(Ω) 是有效的 0 值
    let bc = &eg.lines[5];
    assert_eq!(bc.object_type, "BUS_CONNECTION");
    assert_eq!(bc.reactor, Some(0.0));

    // 变压器：三卷变，三侧齐全
    let t = &eg.transformers[0];
    assert_eq!(t.idx, 20);
    // gbk-builtin 内置 codec 不支持 CJK（GBK 中文解出 U+FFFD），中文名断言仅在默认 encoding_rs 路径生效
    #[cfg(not(feature = "gbk-builtin"))]
    assert_eq!(t.name, "主变A套");
    assert_eq!(t.object_type, "MAIN");
    assert_eq!(t.pwr_rtg, 180.0, "CAPACITY=180(MVA)");
    assert_eq!(t.winding_num, 3);
    assert_eq!(t.sta_chns.len(), 20);
    assert_eq!(t.windings.len(), 3, "WINDING_NUM=3 应构建高/中/低三侧");

    // 绕组接线组别是字符串代号，不是数值（旧实现声明为 f64 会全部归零）
    let wgs: Vec<&str> = t.windings.iter().map(|w| w.wg.as_str()).collect();
    assert_eq!(wgs, vec!["Y", "Y12", "D11"]);
    let locs: Vec<&str> = t.windings.iter().map(|w| w.location.as_str()).collect();
    assert_eq!(locs, vec!["High", "Medium", "Low"]);
    let vs: Vec<f64> = t.windings.iter().map(|w| w.v_rtg).collect();
    assert_eq!(vs, vec![220.0, 110.0, 10.0]);

    // TA_Id_#1→高压侧、#3→中压侧、#5→低压侧；每条只有三相 + 极性符号（无 Io）
    let expect = [(1usize, 2usize, 3usize), (15, 16, 17), (29, 30, 31)];
    for (w, (ia, ib, ic)) in t.windings.iter().zip(expect) {
        assert_eq!(w.currents.len(), 1, "{} 侧应有一路分支电流", w.location);
        let c = &w.currents[0];
        assert_eq!(
            (c.ia_idx, c.ib_idx, c.ic_idx),
            (ia, ib, ic),
            "{} 侧电流通道",
            w.location
        );
        assert_eq!(c.in_idx, 0, "变压器 TA_Id 无零序通道，第 4 项是极性符号");
        assert_eq!(c.dir, 1, "{} 侧极性符号", w.location);
    }

    // 各侧电压通道组
    let acvs: Vec<(usize, usize, usize)> = t
        .windings
        .iter()
        .map(|w| (w.acv.ua_idx, w.acv.ub_idx, w.acv.uc_idx))
        .collect();
    assert_eq!(acvs, vec![(7, 8, 9), (21, 22, 23), (35, 36, 37)]);
}

#[test]
fn test_inf_round_trip() {
    let bytes = std::fs::read(data_path("binary_inf.inf")).unwrap();
    let text = comtrade_io::encoding::decode_with(&bytes, comtrade_io::Encoding::Gbk);
    let inf = InfFile::from_str(&text);
    let serialized = inf.to_string();
    let reparsed = InfFile::from_str(&serialized);
    assert_eq!(reparsed.sections.len(), inf.sections.len());

    // 未建模节名必须保留原始大小写（`ZprdInfo` 不能写成 `ZPRDINFO`）
    let names = |f: &InfFile| -> Vec<String> {
        f.sections
            .iter()
            .filter_map(|s| match &s.kind {
                SectionKind::Other(n) => Some(n.clone()),
                _ => None,
            })
            .collect()
    };
    let before = names(&inf);
    assert!(!before.is_empty(), "样本应含未建模节");
    assert_eq!(names(&reparsed), before, "未建模节名应逐字保留");
    assert!(
        before.iter().any(|n| n == "ZprdInfo"),
        "应保留混合大小写节名 ZprdInfo，实际 {:?}",
        before
    );
}

#[test]
fn test_equipment_section_round_trip() {
    let bytes = std::fs::read(data_path("binary_inf.inf")).unwrap();
    let text = comtrade_io::encoding::decode_with(&bytes, comtrade_io::Encoding::Gbk);
    let eg = InfFile::from_str(&text).to_equipment_group();

    // 设备段 → INF 文本 → 重新解析，语义应等价
    let inf = InfFile::from_str(&text);
    let cfg = inf.to_config(None).expect("样本应能构建 Config");
    let rebuilt = comtrade_io::inf_from_config(&cfg, Some(&eg));
    let eg2 = InfFile::from_str(&rebuilt.to_string()).to_equipment_group();

    assert_eq!(eg2.buses.len(), eg.buses.len());
    assert_eq!(eg2.lines.len(), eg.lines.len());
    assert_eq!(eg2.transformers.len(), eg.transformers.len());

    for (a, b) in eg.buses.iter().zip(&eg2.buses) {
        assert_eq!(
            (a.idx, &a.name, a.v_rtg, &a.tv_pos),
            (b.idx, &b.name, b.v_rtg, &b.tv_pos)
        );
        assert_eq!(a.acv, b.acv);
        assert_eq!(a.sta_chns, b.sta_chns);
    }
    for (a, b) in eg.lines.iter().zip(&eg2.lines) {
        assert_eq!(
            (a.idx, &a.name, &a.object_type),
            (b.idx, &b.name, &b.object_type)
        );
        assert_eq!(a.line_len, b.line_len);
        assert_eq!(a.impedance, b.impedance);
        assert_eq!(a.capacitance, b.capacitance, "CG 位置序须往返一致");
        assert_eq!(a.mutual_inductance, b.mutual_inductance);
        assert_eq!(a.reactor, b.reactor, "REACTOR=NO 须往返为 None");
        assert_eq!(a.currents, b.currents, "TA_CHNS 方向标志须保留");
        assert_eq!(a.voltages, b.voltages);
        assert_eq!(a.sta_chns, b.sta_chns);
    }
    for (a, b) in eg.transformers.iter().zip(&eg2.transformers) {
        assert_eq!(
            (a.idx, &a.name, a.pwr_rtg, a.winding_num),
            (b.idx, &b.name, b.pwr_rtg, b.winding_num)
        );
        assert_eq!(a.windings.len(), b.windings.len(), "绕组数须往返一致");
        for (wa, wb) in a.windings.iter().zip(&b.windings) {
            assert_eq!(
                (&wa.location, &wa.wg, wa.v_rtg),
                (&wb.location, &wb.wg, wb.v_rtg)
            );
            assert_eq!(wa.acv, wb.acv);
            assert_eq!(wa.currents, wb.currents, "TA_Id 槽位须映射回同一侧");
        }
    }
}

#[test]
fn test_from_config() {
    use comtrade_io::Config;
    let bytes = std::fs::read(data_path("binary_1999.cfg")).unwrap();
    let cfg_text = comtrade_io::encoding::decode_with(&bytes, comtrade_io::Encoding::Gbk);
    let config = Config::from_str(&cfg_text).unwrap();
    let inf = comtrade_io::inf_from_config(&config, None);
    assert!(!inf.sections.is_empty());
    // 应包含 File_Description
    assert!(inf.file_description().is_some());
}

#[test]
fn test_section_get_case_insensitive() {
    let bytes = std::fs::read(data_path("binary_inf.inf")).unwrap();
    let text = comtrade_io::encoding::decode_with(&bytes, comtrade_io::Encoding::Gbk);
    let inf = InfFile::from_str(&text);
    if let Some(fd) = inf.file_description() {
        let upper = fd.get("STATION_NAME");
        let lower = fd.get("station_name");
        assert_eq!(upper, lower, "Section::get 应大小写不敏感");
    }
}

#[test]
fn test_generator_exciter_section_round_trip() {
    // 样本文件无 POWER/EXCITATION 段，用合成 INF 文本验证 §1.6/§1.7 全链路 round-trip。
    // 路径：parse → InfFile::to_string (Section::to_text) → reparse，逐字段比对。
    let inf_text = r"[ZYHD Power_#1]
DEV_ID=5,1号发电机
SYS_ID=
TRM_ID=20,主变A套
OBJECT_TYPE=STEAM_TURBINE
FREQ=50(Hz)
CAPACITY=300(MW)
FACTOR=0.85
V1=20(KV)
BRANCH_NUM=2, 2, 0
Rotor_I=1800(A)
Rotor_V2=75(mV)
Ufe=100, 50
X=1.8, 1.7, 0.3, 0.1
EXCITATION_MODE=0
IGT_DIR=1
TV_CHNS=40, 41, 42
TA_CHNS=43, 44, 45, 1
TA_Z1_CHNS=46, 47, 48, -1
Ufe_CHNS=49, 50, 51
Ife_CHN=52
UN_CHNS=53, 54, 55
TA_Ido_CHN=56
OTH_ACHNS=57, 58
STATUS_CHNS=100, 101

[ZYHD Excitation_#1]
DEV_ID=6,1号励磁机
SYS_ID=
PWR_ID=5,1号发电机
OBJECT_TYPE=PRIMARY
FREQ=100(Hz)
V1=0.5(KV)
TV_CHNS=60, 61, 62, 63
TA_CHNS=64, 65, 66, 1
TA_Z_CHNS=67, 68, 69, -1
OTH_ACHNS=70
STATUS_CHNS=102, 103
";

    let inf = InfFile::from_str(inf_text);
    let eg = inf.to_equipment_group();
    assert_eq!(eg.generators.len(), 1);
    assert_eq!(eg.exciters.len(), 1);

    // 序列化 → 重新解析，逐字段比对
    let rebuilt_text = inf.to_string();
    let eg2 = InfFile::from_str(&rebuilt_text).to_equipment_group();

    assert_eq!(eg2.generators.len(), 1, "发电机段 round-trip 后数量不变");
    assert_eq!(eg2.exciters.len(), 1, "励磁机段 round-trip 后数量不变");

    let g1 = &eg.generators[0];
    let g2 = &eg2.generators[0];
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

    let e1 = &eg.exciters[0];
    let e2 = &eg2.exciters[0];
    assert_eq!((e1.idx, &e1.name), (e2.idx, &e2.name));
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
