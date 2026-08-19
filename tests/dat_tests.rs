//! DAT 数据文件集成测试

use comtrade_io::{Config, DatFile, DataType, Segment};

fn data_path(name: &str) -> String {
    format!("tests/data/{}", name)
}

fn load_cfg(name: &str) -> Config {
    let bytes = std::fs::read(data_path(name)).unwrap();
    let text = comtrade_io::encoding::decode_with(&bytes, comtrade_io::Encoding::Gbk);
    Config::from_str(&text).unwrap()
}

fn strict_cfg(data_type: DataType, analog: usize, status: usize, rows: usize) -> Config {
    let mut cfg = Config::default();
    cfg.channels.total = analog + status;
    cfg.channels.analog = analog;
    cfg.channels.status = status;
    cfg.data_type = data_type;
    cfg.sampling.segments = vec![Segment::new(1000.0, rows)];
    cfg
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
    let written = dat.to_bytes(&cfg, DataType::Binary).unwrap();
    let reparsed = DatFile::from_bytes(&written, &cfg).unwrap();

    assert_eq!(reparsed.len(), dat.len());
    assert_eq!(reparsed.analog_count(), dat.analog_count());
    assert_eq!(reparsed.status_count(), dat.status_count());
}

#[test]
fn test_binary_dat_rejects_partial_record() {
    let cfg = strict_cfg(DataType::Binary, 1, 0, 1);
    let bytes = vec![0u8; 9]; // record size is 10 bytes: index + timestamp + one i16 analog
    let err = DatFile::from_bytes(&bytes, &cfg).expect_err("半条记录不应被截断接受");
    assert!(
        err.to_string().contains("整数倍"),
        "错误应说明记录长度不完整，实际: {}",
        err
    );
}

#[test]
fn test_binary_dat_rejects_sample_count_mismatch() {
    let cfg = strict_cfg(DataType::Binary, 1, 0, 2);
    let bytes = vec![0u8; 10]; // one full record, CFG expects two samples
    let err = DatFile::from_bytes(&bytes, &cfg).expect_err("样本数不匹配不应被 min 截断");
    assert!(
        err.to_string().contains("采样点数"),
        "错误应说明采样点数不一致，实际: {}",
        err
    );
}

#[test]
fn test_ascii_round_trip() {
    let cfg = load_cfg("ascii_1999.cfg");
    let bytes = std::fs::read(data_path("ascii_1999.dat")).unwrap();
    let dat = DatFile::from_bytes(&bytes, &cfg).unwrap();

    let ascii_text = dat.to_ascii(&cfg).unwrap();
    let reparsed = DatFile::from_ascii(&ascii_text, &cfg).unwrap();

    assert_eq!(reparsed.len(), dat.len());
}

#[test]
fn test_ascii_dat_rejects_missing_columns() {
    let cfg = strict_cfg(DataType::Ascii, 1, 1, 1);
    let err = DatFile::from_ascii("1,0,123", &cfg).expect_err("缺失状态列不应补 0");
    assert!(
        err.to_string().contains("列数"),
        "错误应说明列数不一致，实际: {}",
        err
    );
}

#[test]
fn test_ascii_dat_rejects_extra_rows() {
    let cfg = strict_cfg(DataType::Ascii, 1, 0, 1);
    let err = DatFile::from_ascii("1,0,123\n2,1000,456", &cfg).expect_err("多余采样行不应被截断");
    assert!(
        err.to_string().contains("超过 CFG"),
        "错误应说明采样点数超过 CFG 声明，实际: {}",
        err
    );
}

#[test]
fn test_ascii_dat_rejects_bad_numeric_fields() {
    let cfg = strict_cfg(DataType::Ascii, 1, 1, 1);
    let err = DatFile::from_ascii("1,0,bad,0", &cfg).expect_err("非法模拟值不应变成 0");
    assert!(
        err.to_string().contains("模拟量"),
        "错误应指向模拟量字段，实际: {}",
        err
    );
}

#[test]
fn test_ascii_dat_rejects_invalid_status_value() {
    let cfg = strict_cfg(DataType::Ascii, 1, 1, 1);
    let err = DatFile::from_ascii("1,0,123,2", &cfg).expect_err("状态量只能为 0/1");
    assert!(
        err.to_string().contains("只能为 0 或 1"),
        "错误应说明状态量取值非法，实际: {}",
        err
    );
}

#[test]
fn test_to_ascii_rejects_mismatched_column_shape() {
    let cfg = strict_cfg(DataType::Ascii, 1, 0, 2);
    let dat = DatFile {
        sample_index: vec![1, 2],
        timestamp_us: vec![0.0, 1000.0],
        analogs: vec![vec![123.0]],
        statuses: vec![],
    };

    let err = dat
        .to_ascii(&cfg)
        .expect_err("写出前应校验列长度而非 panic");
    assert!(
        err.to_string().contains("模拟通道长度"),
        "错误应说明模拟通道长度不一致，实际: {}",
        err
    );
}

#[test]
fn test_to_bytes_rejects_missing_configured_channel() {
    let cfg = strict_cfg(DataType::Binary, 1, 0, 1);
    let dat = DatFile {
        sample_index: vec![1],
        timestamp_us: vec![0.0],
        analogs: vec![],
        statuses: vec![],
    };

    let err = dat
        .to_bytes(&cfg, DataType::Binary)
        .expect_err("缺少 CFG 声明通道时不应 panic");
    assert!(
        err.to_string().contains("模拟通道数"),
        "错误应说明模拟通道数不一致，实际: {}",
        err
    );
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
    let bytes = dat.to_bytes(&cfg, DataType::Float32).unwrap();
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
