//! cfg crate 集成测试：CFG 配置解析 / 序列化 / 版本识别 / IEC 61850 参引兼容。

use cfg::{is_iec61850_reference, is_reference_like_ccbm, Config, DataType, Header, Version};

#[test]
fn test_parse_basic_cfg() {
    let cfg_text = "\
        STATION,RECORDER,1999\n\
        12,6A,6D\n\
        1,IA, A, ,A,1.0,0.0,0.0,-32767,32767,1.0,1.0,S\n\
        2,IB, B, ,A,1.0,0.0,0.0,-32767,32767,1.0,1.0,S\n\
        3,IC, C, ,A,1.0,0.0,0.0,-32767,32767,1.0,1.0,S\n\
        4,UA, A, ,V,1.0,0.0,0.0,-32767,32767,1.0,1.0,S\n\
        5,UB, B, ,V,1.0,0.0,0.0,-32767,32767,1.0,1.0,S\n\
        6,UC, C, ,V,1.0,0.0,0.0,-32767,32767,1.0,1.0,S\n\
        1,D1, , ,0\n\
        2,D2, , ,0\n\
        3,D3, , ,0\n\
        4,D4, , ,0\n\
        5,D5, , ,0\n\
        6,D6, , ,0\n\
        50.0\n\
        1\n\
        1200,1200\n\
        01/15/2023,10:30:45.123456\n\
        01/15/2023,10:30:45.123456\n\
        ASCII\n\
        1.0";
    let config = Config::from_str(cfg_text).unwrap();
    assert_eq!(config.header.station, "STATION");
    assert_eq!(config.channels.analog, 6);
    assert_eq!(config.channels.status, 6);
    assert_eq!(config.analogs.len(), 6);
    assert_eq!(config.statuses.len(), 6);
    assert_eq!(config.data_type, DataType::Ascii);
}

#[test]
fn ccbm_reference_compatibility_moves_illegal_values_to_reference() {
    let cfg_text = "\
        STATION,RECORDER,1999\n\
        2,1A,1D\n\
        1,Ua,A,MUSV$TVTR1$MX$Vol,V,1,0,0,-1,1,1,1,S\n\
        1,Trip,,SVOUTMUSV$TCTR1$MX$AmpR,0\n\
        50\n\
        1\n\
        100,100\n\
        01/01/2020,00:00:00.000000\n\
        01/01/2020,00:00:00.000000\n\
        ASCII\n\
        1";
    let cfg = Config::from_str(cfg_text).unwrap();
    assert!(cfg.analogs[0].equipment.is_empty());
    assert_eq!(
        cfg.analogs[0]
            .ext
            .as_ref()
            .and_then(|e| e.reference.as_deref()),
        Some("MUSV$TVTR1$MX$Vol")
    );
    assert!(cfg.statuses[0].equipment.is_empty());
    assert_eq!(
        cfg.statuses[0]
            .ext
            .as_ref()
            .and_then(|e| e.reference.as_deref()),
        Some("SVOUTMUSV$TCTR1$MX$AmpR")
    );
}

#[test]
fn ccbm_reference_detection_does_not_match_normal_equipment() {
    assert!(is_reference_like_ccbm("MUSV$TVTR1$MX$Vol"));
    assert!(is_reference_like_ccbm("svoutmusv$tctr1$mx$ampr"));
    assert!(!is_reference_like_ccbm("线路1"));
    assert!(!is_reference_like_ccbm("MUSV设备"));
}

#[test]
fn iec61850_ccbm_is_kept_as_reference_and_not_equipment() {
    assert!(is_iec61850_reference("PTRC$ST$Tr$general"));
    assert!(is_iec61850_reference("TCTR$MX$Amp$"));
    assert!(!is_iec61850_reference("线路$1"));
    let cfg_text = "\
        STATION,RECORDER,1999\n\
        1,0A,1D\n\
        1,Trip,,PTRC$ST$Tr$general,0\n\
        50\n\
        1\n\
        100,100\n\
        01/01/2020,00:00:00.000000\n\
        01/01/2020,00:00:00.000000\n\
        ASCII\n\
        1";
    let cfg = Config::from_str(cfg_text).unwrap();
    assert!(cfg.statuses[0].equipment.is_empty());
    assert_eq!(
        cfg.statuses[0]
            .ext
            .as_ref()
            .and_then(|ext| ext.reference.as_deref()),
        Some("PTRC$ST$Tr$general")
    );
}

#[test]
fn test_round_trip() {
    let original = "\
        STATION,RECORDER,1999\n\
        12,6A,6D\n\
        1,IA, A, ,A,1.0,0.0,0.0,-32767,32767,1.0,1.0,S\n\
        2,IB, B, ,A,1.0,0.0,0.0,-32767,32767,1.0,1.0,S\n\
        3,IC, C, ,A,1.0,0.0,0.0,-32767,32767,1.0,1.0,S\n\
        4,UA, A, ,V,1.0,0.0,0.0,-32767,32767,1.0,1.0,S\n\
        5,UB, B, ,V,1.0,0.0,0.0,-32767,32767,1.0,1.0,S\n\
        6,UC, C, ,V,1.0,0.0,0.0,-32767,32767,1.0,1.0,S\n\
        1,D1, , ,0\n\
        2,D2, , ,0\n\
        3,D3, , ,0\n\
        4,D4, , ,0\n\
        5,D5, , ,0\n\
        6,D6, , ,0\n\
        50\n\
        1\n\
        1200,1200\n\
        01/15/2023,10:30:45.123456\n\
        01/15/2023,10:30:45.123456\n\
        ASCII\n\
        1";
    let config = Config::from_str(original).unwrap();
    let serialized = config.to_string();
    let reparsed = Config::from_str(&serialized).unwrap();
    assert_eq!(reparsed.header.station, config.header.station);
    assert_eq!(reparsed.channels.analog, config.channels.analog);
    assert_eq!(reparsed.analogs.len(), config.analogs.len());
    assert_eq!(reparsed.start_time, config.start_time);
}

#[test]
fn test_empty_first_line_keeps_row_alignment() {
    // 回归：设备在首行输出空行时（部分南瑞文件），过滤空行会把
    // 通道数行误判为文件头导致整体错位。空行必须保留以维持行号。
    let cfg_text = "\
        \n\
        2,1A,1D\n\
        1,Ia,a,0,A,1.0,0.0,0.0,-16384,16384\n\
        1,D1, , ,0\n\
        50\n\
        1\n\
        1000,317\n\
        01/15/2016,10:30:45.123456\n\
        01/15/2016,10:30:45.123456\n\
        BINARY\n\
        1.0";
    let config = Config::from_str(cfg_text).unwrap();
    assert_eq!(config.channels.analog, 1);
    assert_eq!(config.channels.status, 1);
    assert_eq!(config.analogs.len(), 1);
    assert_eq!(config.sampling.freq, 50.0);
    // 空首行被当作空 header（station 为空），但后续行必须不错位
    assert_eq!(config.start_time.year, 2016);
}

#[test]
fn test_version_parse_all_six() {
    for (s, v) in [
        ("1991", Version::V1991),
        ("1999", Version::V1999),
        ("2001", Version::V2001),
        ("2008", Version::V2008),
        ("2013", Version::V2013),
        ("2017", Version::V2017),
    ] {
        assert_eq!(Version::parse(s), Some(v), "版本 {}", s);
        assert_eq!(v.as_str(), s, "as_str 应往返一致");
        assert!(!v.standard().is_empty());
    }
    // 前后空白应容忍
    assert_eq!(Version::parse(" 2013 "), Some(Version::V2013));
    // 未知版本号返回 None，由调用方决定回落
    assert_eq!(Version::parse("2020"), None);
    assert_eq!(Version::parse("abc"), None);
    assert_eq!(Version::parse(""), None);
}

#[test]
fn test_version_is_1999_or_later() {
    // 只有 1991 没有 CFG 可选尾部字段
    assert!(!Version::V1991.is_1999_or_later());
    for v in [
        Version::V1999,
        Version::V2001,
        Version::V2008,
        Version::V2013,
        Version::V2017,
    ] {
        assert!(v.is_1999_or_later(), "{} 应视为 1999 及以后", v.as_str());
    }
    assert_eq!(Version::default(), Version::V1991);
}

#[test]
fn test_header_parses_all_versions() {
    // 关键回归：原实现只匹配 "1999"，2001/2008/2013/2017 会被静默判成 1991
    for s in ["1991", "1999", "2001", "2008", "2013", "2017"] {
        let h = Header::from_line(&format!("ST,REC,{}", s));
        assert_eq!(h.station, "ST");
        assert_eq!(h.recorder, "REC");
        assert_eq!(h.version.as_str(), s, "头部版本 {} 应被识别", s);
        // 序列化必须写回原版本号，不能塌成 1991
        assert_eq!(h.to_line(), format!("ST,REC,{}", s));
    }
}

#[test]
fn test_header_defaults_to_1991_when_version_absent() {
    // 头部为空 / 缺版本字段 → 1991
    assert_eq!(Header::from_line("").version, Version::V1991);
    assert_eq!(Header::from_line("ST").version, Version::V1991);
    assert_eq!(Header::from_line("ST,REC").version, Version::V1991);
    assert_eq!(Header::from_line("ST,REC,").version, Version::V1991);
    // 无法识别的版本号同样回落 1991（对齐 Python）
    assert_eq!(Header::from_line("ST,REC,2020").version, Version::V1991);
    assert_eq!(Header::from_line("ST,REC,abc").version, Version::V1991);
    // 回落不应影响站名/设备名
    let h = Header::from_line("ST,REC,2020");
    assert_eq!((h.station.as_str(), h.recorder.as_str()), ("ST", "REC"));
}

#[test]
fn test_cfg_2013_keeps_version_through_round_trip() {
    // 2013 版 CFG：可选尾部字段与 1999 布局兼容，不应因版本号而丢失
    let text = "\
        GHBZ,220kV线路故障,2013\n\
        2,1A,1D\n\
        1,IA, A, ,A,1.0,0.0,0.0,-32767,32767,1.0,1.0,S\n\
        1,D1, , ,0\n\
        50\n\
        1\n\
        1200,1200\n\
        01/15/2023,10:30:45.123456\n\
        01/15/2023,10:30:45.123456\n\
        ASCII\n\
        1\n\
        UTC-8,UTC-8\n\
        0000";
    let cfg = Config::from_str(text).unwrap();
    assert_eq!(cfg.header.version, Version::V2013);
    assert!(cfg.header.version.is_1999_or_later());
    assert!(cfg.time_info.is_some(), "2013 版的时间码行应被解析");
    assert!(cfg.sampling_time_quality.is_some());

    let reparsed = Config::from_str(&cfg.to_string()).unwrap();
    assert_eq!(reparsed.header.version, Version::V2013, "版本号须往返保真");
    assert_eq!(reparsed.timemult, cfg.timemult);
    assert_eq!(
        reparsed.time_info.as_ref().map(|t| t.time_code.clone()),
        cfg.time_info.as_ref().map(|t| t.time_code.clone())
    );
}
