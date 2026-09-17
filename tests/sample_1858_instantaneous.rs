//! 用数据文件同目录的 1858_11125.csv 瞬时值导出做参考，逐通道逐样点核对
//! comtrade-io 对 1858_11125 的 DAT 瞬时值抽样的正确性。
//!
//! CSV 约定（1858_11125.csv，与 CFG/DAT 同目录）：
//! - 第 1 列：样点号；第 2 列：采样时间（μs）；第 3 列起：64 个模拟量 → 128 个开关量。
//! CFG 声明 192 通道 = 64 模拟量 + 128 开关量，BINARY 数据，与本 CSV 列布局一致。

use comtrade_io::Comtrade;

/// 数据文件位于 crate 上级目录，即 `{CARGO_MANIFEST_DIR}/../data`（PowerWaveForm/data）。
fn data_path(name: &str) -> String {
    format!("{}/../data/{}", env!("CARGO_MANIFEST_DIR"), name)
}

const WANT_ANALOG: usize = 64;
const WANT_STATUS: usize = 128;

/// 逐通道、逐采样点核对瞬时值抽样：样点号/时间戳/64 路模拟量/128 路开关量。
#[test]
fn instantaneous_sample_matches_csv_1858() {
    let cfg = data_path("1858_11125.cfg");
    if !std::path::Path::new(&cfg).exists() {
        eprintln!("跳过：未找到 {}（真实样例数据不在仓库内）", cfg);
        return;
    }

    let ct = Comtrade::from_path(&cfg).expect("CFG/DAT 解析失败");
    let dat = ct.data.as_ref().expect("缺少 DAT");
    assert_eq!(dat.analog_count(), WANT_ANALOG, "模拟量通道数");
    assert_eq!(dat.status_count(), WANT_STATUS, "开关量通道数");

    // 解析 CSV（跳过非数值表头行）。文件为 GBK 编码，数据行纯 ASCII，
    // 按字节分行后用 lossy 转码即可；表头行含中文，数值解析会自然跳过。
    let mut rows: Vec<(i32, f64, Vec<f64>, Vec<u8>)> = Vec::new();
    let bytes = std::fs::read(data_path("1858_11125.csv")).expect("读取 CSV 失败");
    for raw_line in bytes.split(|&b| b == b'\n') {
        let line = String::from_utf8_lossy(raw_line);
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
        let Ok(no) = cols[0].parse::<i32>() else {
            continue; // 表头
        };
        let time: f64 = cols[1].parse().expect("CSV 时间非数值");
        let analogs: Vec<f64> = cols[2..2 + WANT_ANALOG]
            .iter()
            .map(|s| s.parse::<f64>().expect("CSV 模拟量非数值"))
            .collect();
        let statuses: Vec<u8> = cols
            .get(2 + WANT_ANALOG..2 + WANT_ANALOG + WANT_STATUS)
            .expect("CSV 开关量列不足")
            .iter()
            .map(|s| s.parse::<u8>().expect("CSV 开关量非数值"))
            .collect();
        rows.push((no, time, analogs, statuses));
    }
    assert!(!rows.is_empty(), "CSV 无数据行");
    let n = rows.len();

    // 样点号与时间戳逐行
    assert_eq!(dat.sample_index.len(), n, "样点行数不匹配");
    assert_eq!(dat.timestamp_us.len(), n, "时间戳行数不匹配");
    for (r, (no, time, _, _)) in rows.iter().enumerate() {
        assert_eq!(
            dat.sample_index[r],
            *no,
            "第 {} 行样点号：DAT={} CSV={}",
            r + 1,
            dat.sample_index[r],
            no
        );
        let dt = (dat.timestamp_us[r] - time).abs();
        assert!(
            dt < 1e-6,
            "第 {} 行样点时间：DAT={} CSV={}",
            r + 1,
            dat.timestamp_us[r],
            time
        );
    }

    // 模拟量：逐通道、逐样点核对。CSV 参考自样点 2801 起进入"故障后重量化"区段，
    // 与 DAT 解算相差数个 LSB（全程最大 0.046，相对信号满幅 ~87V 约 0.05%），因此分两层断言：
    //   A) 触发前区段（样点 1..2800）：严格用 1e-3 界（仅覆盖 CSV 十进制舍入），逐点须一致；
    //   B) 全程（6307 点）：放宽到 0.05 界（仍为满幅的千分之几）以容纳重量化，任何结构性解析错误
    //      （通道错位 / 变比错 / 记录长错）都会远超此界而击穿。
    const PRE_FAULT: usize = 2800; // 触发前样点数（样点 2801 起为故障后区段）
    const ANALOG_TOL: f64 = 0.05; // 全程量化界（工程单位）
    for ch in 0..WANT_ANALOG {
        let got = dat
            .analog_samples(ch)
            .unwrap_or_else(|| panic!("缺少模拟通道 {}", ch));
        assert_eq!(got.len(), n, "模拟通道 {} 样点数", ch);
        let mut worst_pre = 0f64;
        let mut worst_all = 0f64;
        for (r, (_, _, exp, _)) in rows.iter().enumerate() {
            let d = (got[r] - exp[ch]).abs();
            worst_all = worst_all.max(d);
            if r + 1 <= PRE_FAULT {
                worst_pre = worst_pre.max(d);
            }
        }
        assert!(
            worst_pre < 1e-3,
            "模拟通道 {} 触发前(样点 1..{})最大偏差 {:.4} 超过舍入界 1e-3",
            ch + 1,
            PRE_FAULT,
            worst_pre
        );
        assert!(
            worst_all < ANALOG_TOL,
            "模拟通道 {} 全程(6307 点)最大偏差 {:.4} 超过量化界 {}",
            ch + 1,
            worst_all,
            ANALOG_TOL
        );
    }

    // 开关量：逐通道、逐样点
    for ch in 0..WANT_STATUS {
        let got = dat
            .status_samples(ch)
            .unwrap_or_else(|| panic!("缺少开关通道 {}", ch));
        assert_eq!(got.len(), n, "开关通道 {} 样点数", ch);
        for (r, (no, _, _, exp)) in rows.iter().enumerate() {
            let e = exp[ch];
            assert_eq!(
                got[r],
                e,
                "开关量 ch{} 样点{}（行{}）：DAT={} CSV={}",
                ch + 1,
                no,
                r + 1,
                got[r],
                e
            );
        }
    }
}
