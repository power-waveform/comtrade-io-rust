//! dat crate 集成测试：按时间戳重算采样段（`recalculate_segments`）。

use cfg::{Segment, DEFAULT_NOMINAL_FREQ};
use dat::recalculate_segments;

/// 10kHz 恒定采样：单段，samp=10000，50Hz 下每周波 200 点
#[test]
fn test_recalculate_segments_derives_cycle_point_num() {
    let ts: Vec<f64> = (0..10).map(|i| i as f64 * 100.0).collect();
    let segs = recalculate_segments(&ts, 50.0);
    assert_eq!(segs.len(), 1);
    assert_eq!(segs[0].samp_rate, 10_000.0);
    assert_eq!(segs[0].cycle_point_num, Some(200.0));
}

/// 单段应覆盖全部采样点：start_point=0、end_point=count=10
#[test]
fn test_recalculate_segments_derives_start_point_and_count() {
    let ts: Vec<f64> = (0..10).map(|i| i as f64 * 100.0).collect();
    let segs = recalculate_segments(&ts, 50.0);
    assert_eq!(segs.len(), 1);
    assert_eq!(segs[0].start_point, Some(0));
    assert_eq!(segs[0].end_point, 10);
    assert_eq!(segs[0].count, Some(10), "单段应覆盖全部 10 个采样点");
}

/// count 必须恒等于 end_point - start_point（半开区间不变式）
#[test]
fn test_recalculate_segments_count_matches_half_open_range() {
    // 前 5 点 100us（10kHz），后 5 点 500us（2kHz）
    let mut ts = vec![0.0, 100.0, 200.0, 300.0, 400.0];
    let mut t = 400.0;
    for _ in 0..5 {
        t += 500.0;
        ts.push(t);
    }
    let segs = recalculate_segments(&ts, 50.0);
    assert!(segs.len() >= 2, "应检出采样率切换");
    for seg in &segs {
        let start = seg.start_point.expect("应派生 start_point");
        let count = seg.count.expect("应派生 count");
        assert_eq!(
            count,
            seg.end_point - start,
            "count 须等于 end_point-start_point（[{}, {})）",
            start,
            seg.end_point
        );
    }
    // 分段须首尾相接且覆盖全程：前段 end == 后段 start
    assert_eq!(segs[0].start_point, Some(0), "首段应从 0 开始");
    for pair in segs.windows(2) {
        assert_eq!(
            pair[1].start_point,
            Some(pair[0].end_point),
            "相邻段须首尾相接，无空隙无重叠"
        );
    }
    assert_eq!(
        segs.last().unwrap().end_point,
        ts.len(),
        "末段应终止于总采样点数"
    );
}

/// 60Hz 额定频率：同一采样率派生出不同的每周波点数
#[test]
fn test_recalculate_segments_respects_nominal_freq() {
    let ts: Vec<f64> = (0..10).map(|i| i as f64 * 100.0).collect();
    let segs = recalculate_segments(&ts, 60.0);
    assert_eq!(segs[0].samp_rate, 10_000.0);
    let cpn = segs[0].cycle_point_num.expect("应派生每周波点数");
    assert!(
        (cpn - 10_000.0 / 60.0).abs() < 1e-9,
        "60Hz 下应为 samp/60，实际 {}",
        cpn
    );
}

/// 非法额定频率（0 / 负数 / 非有限）一律退回 50Hz，对齐 Python `freq if freq else 50`
#[test]
fn test_recalculate_segments_defaults_bad_freq_to_50() {
    let ts: Vec<f64> = (0..10).map(|i| i as f64 * 100.0).collect();
    for bad in [0.0, -50.0, f64::NAN, f64::INFINITY] {
        let segs = recalculate_segments(&ts, bad);
        assert_eq!(
            segs[0].cycle_point_num,
            Some(200.0),
            "freq={bad} 应退回 {DEFAULT_NOMINAL_FREQ}Hz"
        );
    }
}

/// 采样率切换 >5% 时应分段，且每段各自派生每周波点数
#[test]
fn test_recalculate_segments_splits_on_rate_change() {
    // 前 5 点间隔 100us（10kHz），后 5 点间隔 500us（2kHz）
    let mut ts = vec![0.0, 100.0, 200.0, 300.0, 400.0];
    let mut t = 400.0;
    for _ in 0..5 {
        t += 500.0;
        ts.push(t);
    }
    let segs = recalculate_segments(&ts, 50.0);
    assert!(segs.len() >= 2, "应检出采样率切换，实际 {} 段", segs.len());
    for seg in &segs {
        let cpn = seg.cycle_point_num.expect("每段都应派生每周波点数");
        assert!(
            (cpn - seg.samp_rate / 50.0).abs() < 1e-9,
            "每周波点数须等于 samp_rate/50"
        );
    }
}

/// 从 CFG 解析出的段不带派生值（CFG 文件本身不携带该字段）
#[test]
fn test_cfg_parsed_segment_has_no_cycle_point_num() {
    let seg = Segment::new(10_000.0, 45_600);
    assert_eq!(seg.cycle_point_num, None);
}

/// 样本点不足时返回空
#[test]
fn test_recalculate_segments_too_few_points() {
    assert!(recalculate_segments(&[], 50.0).is_empty());
    assert!(recalculate_segments(&[0.0], 50.0).is_empty());
}
