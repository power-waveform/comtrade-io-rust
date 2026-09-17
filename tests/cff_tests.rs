//! CFF 单文件集成测试

mod common;
use common::data_path;

use comtrade_io::CffFile;

#[test]
fn test_parse_cff_2013() {
    let bytes = std::fs::read(data_path("ascii_cff_2013.cff")).unwrap();
    let cff = CffFile::from_bytes(&bytes);
    assert!(cff.sections.cfg.is_some(), "应包含 CFG 段");
    assert!(cff.sections.dat.is_some(), "应包含 DAT 段");
}

#[test]
fn test_cff_to_config() {
    let bytes = std::fs::read(data_path("ascii_cff_2013.cff")).unwrap();
    let cff = CffFile::from_bytes(&bytes);
    let config = cff.to_config().unwrap();
    assert!(!config.header.station.is_empty());
    assert!(config.channels.analog > 0);
    // 该样本头部为 `GHBZ,220kV线路故障,2013`。
    // 原实现只识别 "1999"，会把它静默判成 1991。
    assert_eq!(
        config.header.version,
        comtrade_io::Version::V2013,
        "2013 版应被正确识别，而非回落 1991"
    );
    assert!(config.header.version.is_1999_or_later());
}

#[test]
fn test_cff_version_survives_round_trip() {
    let bytes = std::fs::read(data_path("ascii_cff_2013.cff")).unwrap();
    let cff = CffFile::from_bytes(&bytes);
    let config = cff.to_config().unwrap();
    let data = cff.to_data(&config).unwrap();
    let inf = cff.to_inf();

    let written = CffFile::encode(&config, &data, inf.as_ref(), config.data_type);
    let re_config = CffFile::from_bytes(&written).to_config().unwrap();
    assert_eq!(
        re_config.header.version,
        comtrade_io::Version::V2013,
        "版本号不应在写回时塌成 1991"
    );
}

#[test]
fn test_cff_to_data() {
    let bytes = std::fs::read(data_path("ascii_cff_2013.cff")).unwrap();
    let cff = CffFile::from_bytes(&bytes);
    let config = cff.to_config().unwrap();
    let data = cff.to_data(&config).unwrap();
    assert!(!data.is_empty());
    assert_eq!(data.analog_count(), config.channels.analog);
}

#[test]
fn test_cff_round_trip() {
    let bytes = std::fs::read(data_path("ascii_cff_2013.cff")).unwrap();
    let cff = CffFile::from_bytes(&bytes);
    let config = cff.to_config().unwrap();
    let data = cff.to_data(&config).unwrap();
    let inf = cff.to_inf();

    // 写回 CFF
    let written = CffFile::encode(&config, &data, inf.as_ref(), config.data_type);

    // 重新解析
    let reparsed = CffFile::from_bytes(&written);
    let re_config = reparsed.to_config().unwrap();
    let re_data = reparsed.to_data(&re_config).unwrap();

    assert_eq!(re_config.header.station, config.header.station);
    assert_eq!(re_data.len(), data.len());
    assert_eq!(re_data.analog_count(), data.analog_count());
}

#[test]
fn test_cff_segments_cfg_required() {
    // 缺少 CFG 段时报错
    let cff = CffFile::default();
    assert!(cff.to_config().is_err());
}

#[test]
fn test_cff_chinese_channel_name() {
    // 该 UTF-8 fixture 的通道名含中文「母线」（hexdump 确认 E6AF8D=母）。
    // 若 section_splitter 仍按 GBK 解码，会得到「垣嶇厎」等乱码。
    let bytes = std::fs::read(data_path("ascii_cff_2013.cff")).unwrap();
    let cff = CffFile::from_bytes(&bytes);
    let config = cff.to_config().unwrap();
    let name = config.analog(1).unwrap().name.as_str();
    assert!(
        name.contains('母'),
        "UTF-8 通道名应含「母」，实际: {}",
        name
    );
}
