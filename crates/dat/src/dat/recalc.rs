//! 按时间戳重算采样段（5% 阈值），独立纯函数，不修改输入

use cfg::{Segment, DEFAULT_NOMINAL_FREQ};

/// 按时间戳重算采样段（5% 阈值），独立纯函数，不修改输入
///
/// `nominal_freq` 为额定频率（Hz），用于派生每段的每周波采样点数
/// `cycle_point_num = samp_rate / nominal_freq`；当其 `<= 0` 或非有限时按
/// [`DEFAULT_NOMINAL_FREQ`] 取 50Hz（对齐 Python 基线的 `freq if freq else 50`）。
/// 注意额定频率**不参与** `samp_rate` 本身的计算——采样率完全由时间戳差值决定。
pub fn recalculate_segments(timestamps_us: &[f64], nominal_freq: f64) -> Vec<Segment> {
    if timestamps_us.len() < 2 {
        return Vec::new();
    }

    let freq = if nominal_freq > 0.0 && nominal_freq.is_finite() {
        nominal_freq
    } else {
        DEFAULT_NOMINAL_FREQ
    };

    let time_diffs: Vec<f64> = timestamps_us.windows(2).map(|w| w[1] - w[0]).collect();

    let change_threshold = 0.05;
    let mut change_indices = Vec::new();
    for i in 1..time_diffs.len() {
        let prev = time_diffs[i - 1].abs();
        if prev < 1e-10 {
            continue;
        }
        let diff = (time_diffs[i] - time_diffs[i - 1]).abs();
        if diff / prev > change_threshold {
            change_indices.push(i + 1);
        }
    }

    let mut segment_starts = vec![0];
    segment_starts.extend(&change_indices);
    let mut segment_ends = change_indices.clone();
    segment_ends.push(timestamps_us.len());

    let mut segments = Vec::new();
    for (&start, &end) in segment_starts.iter().zip(segment_ends.iter()) {
        if start >= end || start >= time_diffs.len() {
            continue;
        }
        let diff = time_diffs[start];
        if diff <= 0.0 || !diff.is_finite() {
            continue;
        }
        let interval_count = end - start - 1;
        let samp = if interval_count > 0 {
            let avg = (timestamps_us[end - 1] - timestamps_us[start]) / interval_count as f64;
            (1_000_000.0 / avg).round()
        } else {
            (1_000_000.0 / diff).round()
        };
        // 派生 start_point / count / cycle_point_num（对齐 Python dat.py:356 的四字段输出）
        segments.push(Segment::derived(samp, start, end, freq));
    }

    segments
}
