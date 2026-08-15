//! DAT 数据文件读写模块
//!
//! 支持 ASCII / BINARY / BINARY32 / FLOAT32 格式的解析与写出。
//! 采用列式存储：每列独立 Vec，对齐 Python DataFrame 的按列访问模式。

mod ascii;
mod binary;

use std::path::Path;

use crate::cfg::{Config, DataType, DEFAULT_NOMINAL_FREQ};
use crate::encoding;
use crate::error::{Error, FileRole, Result};
use crate::time::Timestamp;

/// 状态变位记录
#[derive(Debug, Clone, Copy)]
pub struct StatusChange {
    /// 采样点号（1-based）
    pub sample_point: usize,
    /// 绝对时间戳
    pub timestamp: Option<Timestamp>,
    /// 状态值（0/1）
    pub state: u8,
}

/// 采样数据集（列式存储，长度一致）
#[derive(Debug, Clone, Default)]
pub struct DatFile {
    /// 采样序号
    pub sample_index: Vec<i32>,
    /// 时间戳（μs，已应用 timemult）
    pub timestamp_us: Vec<f64>,
    /// 模拟量工程值：外层按通道，内层按采样点
    pub analogs: Vec<Vec<f64>>,
    /// 状态量：外层按通道，内层按采样点（0/1）
    pub statuses: Vec<Vec<u8>>,
}

impl DatFile {
    /// 采样点数
    pub fn len(&self) -> usize {
        self.sample_index.len()
    }

    /// 是否无采样数据
    pub fn is_empty(&self) -> bool {
        self.sample_index.is_empty()
    }

    /// 模拟量通道数
    pub fn analog_count(&self) -> usize {
        self.analogs.len()
    }

    /// 状态量通道数
    pub fn status_count(&self) -> usize {
        self.statuses.len()
    }

    /// 从文件读取 DAT（按 cfg.data_type 分派）
    pub fn from_file(path: &Path, cfg: &Config) -> Result<DatFile> {
        let bytes = std::fs::read(path).map_err(|e| Error::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
        DatFile::from_bytes(&bytes, cfg)
    }

    /// 从字节解析 DAT（按 cfg.data_type 分派）
    pub fn from_bytes(bytes: &[u8], cfg: &Config) -> Result<DatFile> {
        match cfg.data_type {
            DataType::Ascii => {
                // 编码探测：UTF-8 → GBK → Latin-1
                let text = encoding::decode(bytes);
                DatFile::from_ascii(&text, cfg)
            },
            _ => binary::parse_binary(bytes, cfg),
        }
    }

    /// 从 ASCII 文本解析 DAT
    pub fn from_ascii(text: &str, cfg: &Config) -> Result<DatFile> {
        let mut dat = ascii::parse_ascii(text, cfg)?;
        // 形状对齐
        dat = fit_to_config(dat, cfg)?;
        // 应用 timemult
        apply_timemult(&mut dat, cfg);
        Ok(dat)
    }

    /// 写出为 ASCII 文本
    pub fn to_ascii(&self, cfg: &Config) -> String {
        ascii::write_ascii(self, cfg)
    }

    /// 写出为二进制字节
    pub fn to_bytes(&self, cfg: &Config, dt: DataType) -> Vec<u8> {
        binary::write_binary(self, cfg, dt)
    }

    /// 写入文件
    pub fn write_file(&self, path: &Path, cfg: &Config, dt: DataType) -> Result<()> {
        let bytes = match dt {
            DataType::Ascii => self.to_ascii(cfg).into_bytes(),
            _ => self.to_bytes(cfg, dt),
        };
        std::fs::write(path, bytes).map_err(|e| Error::Io {
            path: path.to_path_buf(),
            source: e,
        })
    }

    /// 获取指定模拟通道的采样数据
    pub fn analog_samples(&self, channel_index: usize) -> Option<&[f64]> {
        self.analogs.get(channel_index).map(|v| v.as_slice())
    }

    /// 获取指定状态通道的采样数据
    pub fn status_samples(&self, channel_index: usize) -> Option<&[u8]> {
        self.statuses.get(channel_index).map(|v| v.as_slice())
    }

    /// 变位检测：返回发生变位的状态通道索引及其变位记录
    pub fn changed_statuses(
        &self,
        start_time: Option<Timestamp>,
    ) -> Vec<(usize, Vec<StatusChange>)> {
        let mut results = Vec::new();
        for (ch_idx, status_col) in self.statuses.iter().enumerate() {
            if status_col.is_empty() {
                continue;
            }
            let initial = status_col[0];
            let mut records = vec![StatusChange {
                sample_point: 1,
                timestamp: start_time,
                state: initial,
            }];

            for i in 1..status_col.len() {
                if status_col[i] != status_col[i - 1] {
                    let ts = start_time.and_then(|st| {
                        let us = self.timestamp_us.get(i).copied().unwrap_or(0.0);
                        st.add_micros(us as i64)
                    });
                    records.push(StatusChange {
                        sample_point: i + 1,
                        timestamp: ts,
                        state: status_col[i],
                    });
                }
            }

            if records.len() > 1 {
                results.push((ch_idx + 1, records));
            }
        }
        results
    }
}

/// 形状对齐：按 CFG 声明的通道数与采样点数截断/补齐（对齐 Python _validate_shape）
fn fit_to_config(mut dat: DatFile, cfg: &Config) -> Result<DatFile> {
    let actual_rows = dat.len();
    let expected_rows = cfg
        .sampling
        .segments
        .last()
        .map(|s| s.end_point)
        .unwrap_or(0);

    if actual_rows > expected_rows && expected_rows > 0 {
        // 截断
        dat.sample_index.truncate(expected_rows);
        dat.timestamp_us.truncate(expected_rows);
        for col in &mut dat.analogs {
            col.truncate(expected_rows);
        }
        for col in &mut dat.statuses {
            col.truncate(expected_rows);
        }
    }

    // 校验列数
    let actual_analog = dat.analog_count();
    if actual_analog < cfg.channels.analog {
        // 模拟通道不足
        if actual_analog == 0 && cfg.channels.analog > 0 {
            return Err(Error::parse(
                FileRole::Dat,
                0,
                format!(
                    "期望最少{}列模拟量，实际{}列",
                    cfg.channels.analog + 2,
                    actual_analog + 2
                ),
            ));
        }
        // 补零列
        for _ in actual_analog..cfg.channels.analog {
            dat.analogs.push(vec![0.0; dat.len()]);
        }
    }

    let actual_status = dat.status_count();
    if actual_status < cfg.channels.status {
        // 补零列
        for _ in actual_status..cfg.channels.status {
            dat.statuses.push(vec![0u8; dat.len()]);
        }
    }

    // 截断多余列
    dat.analogs.truncate(cfg.channels.analog);
    dat.statuses.truncate(cfg.channels.status);

    Ok(dat)
}

/// 应用 timemult 时间倍乘系数
fn apply_timemult(dat: &mut DatFile, cfg: &Config) {
    let timemult = cfg.timemult;
    if (timemult - 1.0).abs() > f64::EPSILON {
        for ts in &mut dat.timestamp_us {
            *ts *= timemult;
        }
    }
}

/// 按时间戳重算采样段（5% 阈值），独立纯函数，不修改输入
///
/// `nominal_freq` 为额定频率（Hz），用于派生每段的每周波采样点数
/// `cycle_point_num = samp_rate / nominal_freq`；当其 `<= 0` 或非有限时按
/// [`DEFAULT_NOMINAL_FREQ`] 取 50Hz（对齐 Python 基线的 `freq if freq else 50`）。
/// 注意额定频率**不参与** `samp_rate` 本身的计算——采样率完全由时间戳差值决定。
pub fn recalculate_segments(timestamps_us: &[f64], nominal_freq: f64) -> Vec<crate::cfg::Segment> {
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
        segments.push(crate::cfg::Segment::derived(samp, start, end, freq));
    }

    segments
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let seg = crate::cfg::Segment::new(10_000.0, 45_600);
        assert_eq!(seg.cycle_point_num, None);
    }

    /// 样本点不足时返回空
    #[test]
    fn test_recalculate_segments_too_few_points() {
        assert!(recalculate_segments(&[], 50.0).is_empty());
        assert!(recalculate_segments(&[0.0], 50.0).is_empty());
    }
}
