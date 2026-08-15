//! 跨格式转换集成测试：4 种 DAT 格式互转（多文件）+ CFF 跨格式往返。

use std::path::PathBuf;

use comtrade_io::{Comtrade, DataType, ExportFormat};

fn data_path(name: &str) -> String {
    format!("tests/data/{}", name)
}

/// 临时目录（已创建），测试结束由调用方清理。
fn tmp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ct_conv_{}_{}", std::process::id(), name));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

fn load_source() -> Comtrade {
    Comtrade::from_path(data_path("binary_1999.cfg")).expect("加载源文件失败")
}

/// 首通道前 N 个工程值
fn first_channel_head(ct: &Comtrade, n: usize) -> Vec<f64> {
    ct.analog_channel(1)
        .map(|v| v.samples.iter().take(n).copied().collect())
        .unwrap_or_default()
}

#[test]
fn test_convert_multifile_all_formats() {
    let src = load_source();
    let expected_len = src.data.as_ref().map(|d| d.len()).unwrap_or(0);
    let expected_analog = src.config.channels.analog;
    let expected_status = src.config.channels.status;
    let src_head = first_channel_head(&src, 5);

    for dt in [
        DataType::Ascii,
        DataType::Binary,
        DataType::Binary32,
        DataType::Float32,
    ] {
        let dir = tmp_dir(&format!("mf_{:?}", dt));
        src.write_to_dir(&dir, "out", dt).expect("写出失败");

        // 读回
        let ct = Comtrade::from_path(dir.join("out.cfg")).expect("读回失败");

        // 回归 Bug 1：CFG 的 data_type 必须与目标格式一致
        assert_eq!(ct.config.data_type, dt, "data_type 不一致");
        assert_eq!(
            ct.data.as_ref().map(|d| d.len()).unwrap_or(0),
            expected_len,
            "{:?} 采样点数不一致",
            dt
        );
        assert_eq!(ct.config.channels.analog, expected_analog);
        assert_eq!(ct.config.channels.status, expected_status);

        // 首通道前 5 值近似相等
        // FLOAT32 直存工程值（f32 精度，1e-3）；其余经 raw 取整反算，误差在 mult 量级（1e-2）
        let head = first_channel_head(&ct, 5);
        assert_eq!(head.len(), src_head.len(), "{:?} 首通道长度", dt);
        let tol = if matches!(dt, DataType::Float32) {
            1e-3
        } else {
            1e-2
        };
        for (i, (a, b)) in src_head.iter().zip(head.iter()).enumerate() {
            assert!(
                (a - b).abs() < tol,
                "{:?} 首通道点{}: 期望{} 实际{}",
                dt,
                i,
                a,
                b
            );
        }

        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn test_convert_cff_cross_format() {
    let src = load_source();
    let expected_len = src.data.as_ref().map(|d| d.len()).unwrap_or(0);
    let src_head = first_channel_head(&src, 5);

    // Binary 源 -> FLOAT32 -> CFF 单文件 -> 读回
    let dir = tmp_dir("cff");
    let cff_path = dir.join("out.cff");
    src.save(&cff_path, ExportFormat::Cff, DataType::Float32)
        .expect("CFF 导出失败");

    let ct = Comtrade::from_path(&cff_path).expect("CFF 读回失败");
    assert_eq!(ct.config.data_type, DataType::Float32);
    assert_eq!(ct.data.as_ref().map(|d| d.len()).unwrap_or(0), expected_len);

    let head = first_channel_head(&ct, 5);
    for (i, (a, b)) in src_head.iter().zip(head.iter()).enumerate() {
        assert!(
            (a - b).abs() < 1e-3,
            "CFF FLOAT32 首通道点{}: 期望{} 实际{}",
            i,
            a,
            b
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_convert_multifile_to_cff() {
    // 多文件导出（MultiFile）也走 write_to_dir，验证 save 入口
    let src = load_source();
    let dir = tmp_dir("save_mf");
    let stem_path = dir.join("out"); // save 以此为目录/stem
    src.save(&stem_path, ExportFormat::MultiFile, DataType::Binary32)
        .expect("MultiFile 导出失败");

    let ct = Comtrade::from_path(dir.join("out.cfg")).expect("读回失败");
    assert_eq!(ct.config.data_type, DataType::Binary32);
    assert!(ct.data.as_ref().map(|d| d.len()).unwrap_or(0) > 0);

    let _ = std::fs::remove_dir_all(&dir);
}
