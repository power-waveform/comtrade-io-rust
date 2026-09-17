//! CFG 配置文件集成测试

mod common;
use common::data_path;

use comtrade_io::Config;

/// 按 GBK 编码读取测试文件
fn read_gbk(name: &str) -> String {
    let bytes = std::fs::read(data_path(name)).unwrap();
    comtrade_io::encoding::decode_with(&bytes, comtrade_io::Encoding::Gbk)
}

#[test]
fn test_parse_binary_1999_cfg() {
    let config = Config::from_str(&read_gbk("binary_1999.cfg")).unwrap();
    assert_eq!(config.header.version, comtrade_io::Version::V1999);
    assert!(config.channels.analog > 0);
    assert!(config.channels.status > 0);
    assert_eq!(config.data_type, comtrade_io::DataType::Binary);
}

#[test]
fn test_parse_ascii_1999_cfg() {
    let config = Config::from_str(&read_gbk("ascii_1999.cfg")).unwrap();
    assert_eq!(config.header.version, comtrade_io::Version::V1999);
    assert_eq!(config.data_type, comtrade_io::DataType::Ascii);
}

#[test]
fn test_parse_ascii_1991_cfg() {
    let config = Config::from_str(&read_gbk("ascii_1991.cfg")).unwrap();
    assert_eq!(config.header.version, comtrade_io::Version::V1991);
}

#[test]
fn test_parse_binary_inf_cfg() {
    let config = Config::from_str(&read_gbk("binary_inf.cfg")).unwrap();
    assert!(config.channels.total > 0);
}

#[test]
fn test_cfg_round_trip_binary_1999() {
    let text = read_gbk("binary_1999.cfg");
    let config = Config::from_str(&text).unwrap();
    let serialized = config.to_string();
    let reparsed = Config::from_str(&serialized).unwrap();
    assert_eq!(reparsed.header.station, config.header.station);
    assert_eq!(reparsed.channels.analog, config.channels.analog);
    assert_eq!(reparsed.channels.status, config.channels.status);
    assert_eq!(reparsed.data_type, config.data_type);
    assert_eq!(reparsed.start_time, config.start_time);
}
