//! DAT 数据文件集成测试

use comtrade_io::{Config, DatFile, DataType};

fn data_path(name: &str) -> String {
    format!("tests/data/{}", name)
}

fn load_cfg(name: &str) -> Config {
    let bytes = std::fs::read(data_path(name)).unwrap();
    let text = comtrade_io::encoding::decode_with(&bytes, comtrade_io::Encoding::Gbk);
    Config::from_str(&text).unwrap()
}

#[test]
fn test_parse_binary_1999_dat() {
    let cfg = load_cfg("binary_1999.cfg");
    let bytes = std::fs::read(data_path("binary_1999.dat")).unwrap();
    let dat = DatFile::from_bytes(&bytes, &cfg).unwrap();
    assert!(!dat.is_empty());
    assert_eq!(dat.analog_count(), cfg.channels.analog);
    assert_eq!(dat.status_count(), cfg.channels.status);
}

#[test]
fn test_parse_ascii_1999_dat() {
    let cfg = load_cfg("ascii_1999.cfg");
    let bytes = std::fs::read(data_path("ascii_1999.dat")).unwrap();
    let dat = DatFile::from_bytes(&bytes, &cfg).unwrap();
    assert!(!dat.is_empty());
}

#[test]
fn test_parse_ascii_1991_dat() {
    let cfg = load_cfg("ascii_1991.cfg");
    let bytes = std::fs::read(data_path("ascii_1991.dat")).unwrap();
    let dat = DatFile::from_bytes(&bytes, &cfg).unwrap();
    assert!(!dat.is_empty());
}

#[test]
fn test_parse_binary_inf_dat() {
    let cfg = load_cfg("binary_inf.cfg");
    let bytes = std::fs::read(data_path("binary_inf.dat")).unwrap();
    let dat = DatFile::from_bytes(&bytes, &cfg).unwrap();
    assert!(!dat.is_empty());
}

#[test]
fn test_binary_round_trip() {
    let cfg = load_cfg("binary_1999.cfg");
    let bytes = std::fs::read(data_path("binary_1999.dat")).unwrap();
    let dat = DatFile::from_bytes(&bytes, &cfg).unwrap();

    // 写出再读回
    let written = dat.to_bytes(&cfg, DataType::Binary);
    let reparsed = DatFile::from_bytes(&written, &cfg).unwrap();

    assert_eq!(reparsed.len(), dat.len());
    assert_eq!(reparsed.analog_count(), dat.analog_count());
    assert_eq!(reparsed.status_count(), dat.status_count());
}

#[test]
fn test_ascii_round_trip() {
    let cfg = load_cfg("ascii_1999.cfg");
    let bytes = std::fs::read(data_path("ascii_1999.dat")).unwrap();
    let dat = DatFile::from_bytes(&bytes, &cfg).unwrap();

    let ascii_text = dat.to_ascii(&cfg);
    let reparsed = DatFile::from_ascii(&ascii_text, &cfg).unwrap();

    assert_eq!(reparsed.len(), dat.len());
}

#[test]
fn test_changed_statuses() {
    let cfg = load_cfg("binary_1999.cfg");
    let bytes = std::fs::read(data_path("binary_1999.dat")).unwrap();
    let dat = DatFile::from_bytes(&bytes, &cfg).unwrap();

    let changes = dat.changed_statuses(Some(cfg.start_time));
    // 变位检测结果不应 panic
    let _ = changes.len();
}

#[test]
fn test_float32_round_trip_direct_value() {
    use comtrade_io::{AnalogChannel, Segment};

    // 构造 Config：2 模拟通道，mult/offset 非 1/0（FLOAT32 应忽略它们，直存工程值）
    let mut cfg = Config::default();
    cfg.channels.total = 2;
    cfg.channels.analog = 2;
    cfg.channels.status = 0;
    let a1 = AnalogChannel {
        index: 1,
        multiplier: 0.5,
        offset: 10.0,
        ..Default::default()
    };
    let a2 = AnalogChannel {
        index: 2,
        multiplier: 2.0,
        offset: -5.0,
        ..Default::default()
    };
    cfg.analogs = vec![a1, a2];
    cfg.sampling.segments = vec![Segment::new(1000.0, 3)];
    cfg.data_type = DataType::Float32;

    // 构造 DatFile：3 个采样点的工程值
    let dat = DatFile {
        sample_index: vec![1, 2, 3],
        timestamp_us: vec![0.0, 1000.0, 2000.0],
        analogs: vec![vec![1.5, -2.25, 100.0], vec![0.0, 3.5, -7.125]],
        statuses: vec![],
    };

    // 写 FLOAT32 再读回
    let bytes = dat.to_bytes(&cfg, DataType::Float32);
    let reparsed = DatFile::from_bytes(&bytes, &cfg).unwrap();

    // FLOAT32 直存工程值，读回应与原始近似（f32 精度），不受 mult/offset 影响
    assert_eq!(reparsed.len(), 3);
    for (i, col) in dat.analogs.iter().enumerate() {
        for (j, &orig) in col.iter().enumerate() {
            let got = reparsed.analogs[i][j];
            assert!(
                (orig - got).abs() < 1e-3,
                "通道{}点{}: 期望{} 实际{}",
                i,
                j,
                orig,
                got
            );
        }
    }
}
