//! CFG 配置文件集成测试

use comtrade_io::Config;

/// 获取测试数据路径
fn data_path(name: &str) -> String {
    format!("tests/data/{}", name)
}

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
fn test_from_file_preserves_utf8_chinese() {
    let path = std::env::temp_dir().join(format!("ct_utf8_{}.cfg", std::process::id()));
    let text = "\
中文站,录波器,2013
1,1A,0D
1,母线_Ua,A,母线,V,1,0,0,-32767,32767,1,1,S
50
1
1000,1
01/01/2023,00:00:00.000000
01/01/2023,00:00:00.000000
ASCII
1
UTC,UTC
0000";
    std::fs::write(&path, text.as_bytes()).unwrap();

    let config = Config::from_file(&path).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(config.header.station, "中文站");
    assert_eq!(config.header.recorder, "录波器");
    assert_eq!(config.analog(1).unwrap().name, "母线_Ua");
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
