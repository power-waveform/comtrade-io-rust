//! DFR 二进制区解析

use crate::cfg::Config;
use crate::dat::DatFile;
use crate::error::Result;

/// DFR 二进制区
#[derive(Debug, Clone, Default)]
pub struct DfrBinary {
    /// 设备 ID（≤32 字母数字）
    pub device_id: String,
    /// 设备私有二进制头
    pub bin_header: Vec<u8>,
    /// 帧数据（模拟量 + 状态量二进制采样）
    pub frame_data: Vec<u8>,
}

impl DfrBinary {
    /// 从 `[Data]\r\n` 之后的原始字节切分
    pub fn from_raw(data: &[u8]) -> DfrBinary {
        // 设备 ID（≤32 字母数字）
        let mut device_id = String::new();
        let mut pos = 0;
        while pos < data.len() && pos < 32 {
            let c = data[pos];
            if c.is_ascii_alphanumeric() || c == b'_' {
                device_id.push(c as char);
                pos += 1;
            } else {
                break;
            }
        }

        // 设备私有头
        let header_size = super::device_header_size(&device_id).unwrap_or(0);
        let header_end = pos + header_size;
        let bin_header = if header_size > 0 && header_end <= data.len() {
            data[pos..header_end].to_vec()
        } else {
            Vec::new()
        };

        // 帧数据
        let frame_data = if header_end < data.len() {
            data[header_end..].to_vec()
        } else {
            Vec::new()
        };

        DfrBinary {
            device_id,
            bin_header,
            frame_data,
        }
    }

    /// 解析帧数据为 DatFile
    pub fn to_data(&self, cfg: &Config) -> Result<DatFile> {
        let analog_count = cfg.channels.analog;
        let status_count = cfg.channels.status;
        let status_word_count = status_count.div_ceil(16);
        let frame_size = analog_count * 2 + status_word_count * 2;

        if frame_size == 0 || self.frame_data.is_empty() {
            return Ok(DatFile::default());
        }

        let sample_count = self.frame_data.len() / frame_size;
        let end_point = cfg
            .sampling
            .segments
            .first()
            .map(|s| s.end_point)
            .unwrap_or(sample_count);
        let actual = sample_count.min(end_point);

        let mut sample_index = Vec::with_capacity(actual);
        let mut timestamp_us = Vec::with_capacity(actual);
        let mut analogs: Vec<Vec<f64>> = (0..analog_count)
            .map(|_| Vec::with_capacity(actual))
            .collect();
        let mut statuses: Vec<Vec<u8>> = (0..status_count)
            .map(|_| Vec::with_capacity(actual))
            .collect();

        let multipliers: Vec<f64> = cfg.analogs.iter().map(|a| a.multiplier).collect();
        let offsets: Vec<f64> = cfg.analogs.iter().map(|a| a.offset).collect();

        // 采样率（用于时间戳推算）
        let samp_rate = cfg
            .sampling
            .segments
            .first()
            .map(|s| s.samp_rate)
            .unwrap_or(1000.0);
        let interval_us = if samp_rate > 0.0 {
            1_000_000.0 / samp_rate
        } else {
            1000.0
        };

        for i in 0..actual {
            let offset = i * frame_size;

            // 序号（生成，DFR 无序号列）
            sample_index.push((i + 1) as i32);

            // 时间戳（按采样率等间隔推算）
            timestamp_us.push(i as f64 * interval_us);

            // 模拟量 (i16 LE)
            for (ch, analog_col) in analogs.iter_mut().enumerate().take(analog_count) {
                let ch_offset = offset + ch * 2;
                if ch_offset + 1 < self.frame_data.len() {
                    let raw = i16::from_le_bytes([
                        self.frame_data[ch_offset],
                        self.frame_data[ch_offset + 1],
                    ]) as f64;
                    let mult = multipliers.get(ch).copied().unwrap_or(1.0);
                    let off = offsets.get(ch).copied().unwrap_or(0.0);
                    analog_col.push(raw * mult + off);
                } else {
                    analog_col.push(0.0);
                }
            }

            // 状态量 (u16 LE)
            let status_offset = offset + analog_count * 2;
            for w in 0..status_word_count {
                let w_offset = status_offset + w * 2;
                if w_offset + 1 < self.frame_data.len() {
                    let word = u16::from_le_bytes([
                        self.frame_data[w_offset],
                        self.frame_data[w_offset + 1],
                    ]);
                    for bit in 0..16 {
                        let ch = w * 16 + bit;
                        if ch < status_count {
                            statuses[ch].push(((word >> bit) & 1) as u8);
                        }
                    }
                } else {
                    for status_col in statuses.iter_mut().take(status_count) {
                        status_col.push(0);
                    }
                }
            }
        }

        Ok(DatFile {
            sample_index,
            timestamp_us,
            analogs,
            statuses,
        })
    }
}
