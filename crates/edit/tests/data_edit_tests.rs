//! 波形数据编辑写入口集成测试（方案 §4.2）
//!
//! 覆盖 `DataEdit` trait 的受控写方法：列替换、通道增删、裁剪行；
//! 以及 `Comtrade` 的 CFF 写出。核心断言：编辑后 CFG 通道计数与 DAT 列数保持一致，
//! 写出→重解析不列错位。fixture 相对本 crate 目录访问根包 `tests/data/`。

use cfg::{AnalogChannel, DataType, Segment, StatusChannel};
use edit::DataEdit;
use model::{ChannelKind, Comtrade};

fn fixture(name: &str) -> Comtrade {
    Comtrade::from_path(format!("../../tests/data/{}", name)).expect("样例解析失败")
}

/// 把编辑后的 Comtrade 整组写出到临时目录再重开，返回重开的 Comtrade（验证可重解析）。
fn write_and_reopen(ct: &Comtrade, label: &str, dt: DataType) -> Comtrade {
    let dir = std::env::temp_dir().join(format!(
        "edit-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    ct.write_to_dir(&dir, "out", dt).unwrap();
    let reopened = Comtrade::from_path(dir.join("out.cfg")).unwrap();
    std::fs::remove_dir_all(dir).unwrap();
    reopened
}

#[test]
fn set_analog_column_replaces_values() {
    let mut ct = fixture("ascii_1999.cfg");
    let n = ct.data.as_ref().unwrap().len();
    let orig1 = ct.data.as_ref().unwrap().analogs[1].clone();

    let new: Vec<f64> = (0..n).map(|i| 1000.0 + i as f64).collect();
    ct.set_analog_column(0, new.clone()).unwrap();
    // 改后的列 = 新值，未改动的列保持原值
    assert_eq!(ct.data.as_ref().unwrap().analogs[0], new);
    assert_eq!(ct.data.as_ref().unwrap().analogs[1], orig1);
    // 写 ASCII → 重解析 → 工程值一致（ASCII 反算整数 raw 有量化误差，用容差）
    let reopened = write_and_reopen(&ct, "setcol", DataType::Ascii);
    let reparsed = reopened.data.as_ref().unwrap();
    assert_eq!(reparsed.len(), n);
    assert_eq!(reparsed.analog_count(), ct.config.channels.analog);
    for i in 0..n.min(10) {
        assert!(
            (reparsed.analogs[0][i] - new[i]).abs() < 0.05,
            "重解析工程值应与写入值接近（写 {:.3}，读 {:.3}）",
            new[i],
            reparsed.analogs[0][i]
        );
    }
    // 原通道 1 值未受编辑影响
    assert!(
        (reparsed.analogs[1][0] - orig1[0]).abs() < 0.05,
        "未编辑通道应保持原值"
    );
}

#[test]
fn set_analog_column_rejects_wrong_length() {
    let mut ct = fixture("ascii_1999.cfg");
    let n = ct.data.as_ref().unwrap().len();
    let before = ct.data.as_ref().unwrap().analogs[0].clone();
    let err = ct.set_analog_column(0, vec![1.0; n + 5]).unwrap_err();
    assert!(err.to_string().contains("长度"), "应报告长度不一致: {err}");
    // 失败后数据不变
    assert_eq!(ct.data.as_ref().unwrap().analogs[0], before);
}

#[test]
fn set_status_column_validates_values() {
    let mut ct = fixture("ascii_1999.cfg");
    let n = ct.data.as_ref().unwrap().len();
    // 非 0/1 值被拒绝
    let err = ct.set_status_column(0, vec![2u8; n]).unwrap_err();
    assert!(err.to_string().contains("0 或 1"), "应校验状态值: {err}");
    // 合法 0/1 成功
    let vals: Vec<u8> = (0..n).map(|i| (i % 2) as u8).collect();
    ct.set_status_column(0, vals.clone()).unwrap();
    assert_eq!(ct.data.as_ref().unwrap().statuses[0], vals);
}

#[test]
fn remove_analog_channel_syncs_counts() {
    let mut ct = fixture("ascii_1999.cfg");
    let analog_before = ct.config.channels.analog;
    let status_before = ct.config.channels.status;
    let total_before = ct.config.channels.total;
    assert!(analog_before >= 2, "样例应含至少 2 个模拟量");

    ct.remove_channel(ChannelKind::Analog, 0).unwrap();
    assert_eq!(ct.config.channels.analog, analog_before - 1);
    assert_eq!(ct.config.channels.status, status_before);
    assert_eq!(ct.config.channels.total, total_before - 1);
    assert_eq!(ct.config.analogs.len(), analog_before - 1);
    assert_eq!(ct.data.as_ref().unwrap().analogs.len(), analog_before - 1);
    // 1 基 index 连续
    for (i, ch) in ct.config.analogs.iter().enumerate() {
        assert_eq!(ch.index, i + 1);
    }
    // 重解析后通道数与 CFG 声明一致
    let reopened = write_and_reopen(&ct, "rmch", DataType::Ascii);
    assert_eq!(reopened.config.channels.analog, analog_before - 1);
    assert_eq!(
        reopened.data.as_ref().unwrap().analog_count(),
        analog_before - 1
    );
}

#[test]
fn remove_status_channel_syncs_counts() {
    let mut ct = fixture("binary_inf.cfg");
    let status_before = ct.config.channels.status;
    assert!(status_before >= 2, "样例应含至少 2 个状态量");

    ct.remove_channel(ChannelKind::Status, 0).unwrap();
    assert_eq!(ct.config.channels.status, status_before - 1);
    assert_eq!(
        ct.config.channels.total,
        status_before - 1 + ct.config.channels.analog
    );
    assert_eq!(ct.config.statuses.len(), status_before - 1);
    assert_eq!(ct.data.as_ref().unwrap().statuses.len(), status_before - 1);
    for (i, ch) in ct.config.statuses.iter().enumerate() {
        assert_eq!(ch.index, i + 1);
    }
}

#[test]
fn insert_analog_channel_syncs_counts() {
    let mut ct = fixture("ascii_1999.cfg");
    let analog_before = ct.config.channels.analog;
    let n = ct.data.as_ref().unwrap().len();

    let ch = AnalogChannel {
        index: 0, // 插入后重编号，这里随意
        name: "新通道".into(),
        phase: "A".into(),
        unit: "V".into(),
        ..Default::default()
    };
    ct.insert_analog_channel(1, ch, vec![42.0; n]).unwrap();
    assert_eq!(ct.config.channels.analog, analog_before + 1);
    assert_eq!(ct.data.as_ref().unwrap().analogs.len(), analog_before + 1);
    assert_eq!(ct.config.analogs[2].name, "新通道");
    assert_eq!(ct.data.as_ref().unwrap().analogs[2][0], 42.0);
    for (i, ch) in ct.config.analogs.iter().enumerate() {
        assert_eq!(ch.index, i + 1);
    }
    // 重解析后通道数与列数一致
    let reopened = write_and_reopen(&ct, "insch", DataType::Ascii);
    assert_eq!(reopened.config.channels.analog, analog_before + 1);
    assert_eq!(
        reopened.data.as_ref().unwrap().analog_count(),
        analog_before + 1
    );
}

#[test]
fn insert_status_channel_syncs_counts() {
    let mut ct = fixture("binary_inf.cfg");
    let status_before = ct.config.channels.status;
    let n = ct.data.as_ref().unwrap().len();

    let ch = StatusChannel {
        index: 0,
        name: "新状态".into(),
        phase: "A".into(),
        equipment: String::new(),
        contact: 0,
        ext: None,
    };
    ct.insert_status_channel(1, ch.clone(), vec![0u8; n])
        .unwrap();
    assert_eq!(ct.config.channels.status, status_before + 1);
    assert_eq!(
        ct.config.channels.total,
        ct.config.channels.analog + status_before + 1
    );
    assert_eq!(ct.data.as_ref().unwrap().statuses.len(), status_before + 1);
    for (i, ch) in ct.config.statuses.iter().enumerate() {
        assert_eq!(ch.index, i + 1);
    }
    // 非法状态值被拒绝
    let err = ct.insert_status_channel(0, ch, vec![7u8; n]).unwrap_err();
    assert!(err.to_string().contains("0 或 1"));
}

#[test]
fn crop_rows_truncates_all_columns_and_renumbers() {
    let mut ct = fixture("ascii_1999.cfg");
    let n = ct.data.as_ref().unwrap().len();
    assert!(n > 20, "样例应有足够采样点");

    ct.crop_rows(10, n - 5).unwrap();
    let data = ct.data.as_ref().unwrap();
    let expected = n - 15;
    assert_eq!(data.len(), expected);
    assert_eq!(data.sample_index.len(), expected);
    assert_eq!(data.timestamp_us.len(), expected);
    for col in &data.analogs {
        assert_eq!(col.len(), expected);
    }
    for col in &data.statuses {
        assert_eq!(col.len(), expected);
    }
    // 采样点号从 1 重新编号
    assert_eq!(data.sample_index[0], 1);
    assert_eq!(data.sample_index[expected - 1], expected as i32);
    // 末段 end_point 随新采样点数更新
    let last_end = ct
        .config
        .sampling
        .segments
        .last()
        .map(|s| s.end_point)
        .unwrap();
    assert_eq!(last_end, expected, "末段 end_point 应等于裁剪后采样点数");

    // 重解析后行数与 CFG 声明一致（fit_to_config 按 end_point 截断）
    let reopened = write_and_reopen(&ct, "crop", DataType::Ascii);
    assert_eq!(reopened.data.as_ref().unwrap().len(), expected);
}

#[test]
fn crop_rows_rejects_invalid_range() {
    let mut ct = fixture("ascii_1999.cfg");
    let n = ct.data.as_ref().unwrap().len();
    let before = ct.data.as_ref().unwrap().len();
    assert!(ct.crop_rows(0, 0).is_err(), "start==end 应拒绝");
    assert!(ct.crop_rows(n, n + 1).is_err(), "end 越界应拒绝");
    assert_eq!(ct.data.as_ref().unwrap().len(), before, "失败后不变");
}

#[test]
fn to_cff_bytes_and_write_cff_reopenable() {
    let ct = fixture("ascii_1999.cfg");
    // to_cff_bytes 与 CffFile::encode 等价
    let bytes = ct.to_cff_bytes(DataType::Ascii).unwrap();
    let direct = cff::CffFile::encode(
        &ct.config,
        ct.data.as_ref().unwrap(),
        ct.inf.as_ref(),
        DataType::Ascii,
    );
    assert_eq!(bytes, direct);

    // write_cff → 重开 → 通道数/采样点数一致
    let dir = std::env::temp_dir().join(format!(
        "cff-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("out.cff");
    ct.write_cff(&path, DataType::Ascii).unwrap();
    let reopened = Comtrade::from_path(&path).unwrap();
    assert_eq!(reopened.config.channels.analog, ct.config.channels.analog);
    assert_eq!(reopened.config.channels.status, ct.config.channels.status);
    assert_eq!(
        reopened.data.as_ref().unwrap().len(),
        ct.data.as_ref().unwrap().len()
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn cycles_per_segment_uses_rate_and_freq() {
    // 方案 §7-5：每周波点数 = 采样段 samp_rate / 额定频率
    let ct = fixture("ascii_1999.cfg");
    for seg in &ct.config.sampling.segments {
        assert!(seg.samp_rate > 0.0);
        // 用 Segment::derived 的语义验证：derived 会填 cycle_point_num = rate/freq
        let derived = Segment::derived(seg.samp_rate, 0, seg.end_point, 50.0);
        assert_eq!(derived.cycle_point_num, Some(seg.samp_rate / 50.0));
    }
}
