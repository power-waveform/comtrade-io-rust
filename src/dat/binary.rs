//! 二进制 DAT 解析与写出（BINARY / BINARY32 / FLOAT32）

use super::DatFile;
use crate::cfg::{Config, DataType};
use crate::error::{Error, FileRole, Result};

/// 解析二进制 DAT
pub fn parse_binary(data: &[u8], cfg: &Config) -> Result<DatFile> {
    let analog_count = cfg.channels.analog;
    let status_count = cfg.channels.status;
    let is_32bit = cfg.data_type.is_32bit();
    let is_float = matches!(cfg.data_type, DataType::Float32);
    let analog_size = if is_32bit { 4 } else { 2 };
    let status_word_count = status_count.div_ceil(16);

    let record_size = 4 + 4 + analog_count * analog_size + status_word_count * 2;
    if record_size == 0 {
        return Err(Error::binary(FileRole::Dat, 0, "记录大小为0"));
    }

    // 尾部不足整记录时直接截断（不报错），实际只读取 sample_count 条
    let sample_count = data.len() / record_size;

    let expected = cfg
        .sampling
        .segments
        .last()
        .map(|s| s.end_point)
        .unwrap_or(sample_count);

    let actual_count = sample_count.min(expected);

    let mut sample_index = Vec::with_capacity(actual_count);
    let mut timestamp_us = Vec::with_capacity(actual_count);
    let mut analogs: Vec<Vec<f64>> = (0..analog_count)
        .map(|_| Vec::with_capacity(actual_count))
        .collect();
    let mut statuses: Vec<Vec<u8>> = (0..status_count)
        .map(|_| Vec::with_capacity(actual_count))
        .collect();

    let multipliers: Vec<f64> = cfg.analogs.iter().map(|a| a.multiplier).collect();
    let offsets: Vec<f64> = cfg.analogs.iter().map(|a| a.offset).collect();

    for i in 0..actual_count {
        let offset = i * record_size;

        // 序号 (i32 LE)
        let index = i32::from_le_bytes(read_4(data, offset)?);
        sample_index.push(index);

        // 时间戳 (i32 LE)
        let ts = i32::from_le_bytes(read_4(data, offset + 4)?) as f64;
        timestamp_us.push(ts);

        // 模拟量
        let analog_offset = offset + 8;
        for (ch, col) in analogs.iter_mut().enumerate().take(analog_count) {
            let ch_offset = analog_offset + ch * analog_size;
            let raw = if is_float {
                f32::from_le_bytes(read_4(data, ch_offset)?) as f64
            } else if is_32bit {
                i32::from_le_bytes(read_4(data, ch_offset)?) as f64
            } else {
                i16::from_le_bytes(read_2(data, ch_offset)?) as f64
            };
            let mult = multipliers.get(ch).copied().unwrap_or(1.0);
            let off = offsets.get(ch).copied().unwrap_or(0.0);
            let val = if is_float { raw } else { raw * mult + off };
            col.push(val);
        }

        // 状态量
        let status_offset = offset + 8 + analog_count * analog_size;
        for w in 0..status_word_count {
            let word_offset = status_offset + w * 2;
            let word = u16::from_le_bytes(read_2(data, word_offset)?);
            for bit in 0..16 {
                let ch = w * 16 + bit;
                if ch < status_count {
                    let val = ((word >> bit) & 1) as u8;
                    statuses[ch].push(val);
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

/// 写出二进制 DAT
pub fn write_binary(dat: &DatFile, cfg: &Config, dt: DataType) -> Vec<u8> {
    let analog_count = cfg.channels.analog;
    let status_count = cfg.channels.status;
    let is_32bit = dt.is_32bit();
    let is_float = matches!(dt, DataType::Float32);
    let status_word_count = status_count.div_ceil(16);
    let analog_size = if is_32bit { 4 } else { 2 };
    let record_size = 4 + 4 + analog_count * analog_size + status_word_count * 2;

    let sample_count = dat.len();
    let mut buf = Vec::with_capacity(sample_count * record_size);

    let multipliers: Vec<f64> = cfg.analogs.iter().map(|a| a.multiplier).collect();
    let offsets: Vec<f64> = cfg.analogs.iter().map(|a| a.offset).collect();

    for row in 0..sample_count {
        // 序号 (i32 LE)
        let index = dat.sample_index[row];
        buf.extend_from_slice(&index.to_le_bytes());

        // 时间戳 (i32 LE)
        let ts = dat.timestamp_us[row].round() as i32;
        buf.extend_from_slice(&ts.to_le_bytes());

        // 模拟量：FLOAT32 直存工程值，其余反算原始值
        for ch in 0..analog_count {
            let val = dat.analogs[ch][row];
            if is_float {
                buf.extend_from_slice(&(val as f32).to_le_bytes());
            } else {
                let mult = multipliers.get(ch).copied().unwrap_or(1.0);
                let off = offsets.get(ch).copied().unwrap_or(0.0);
                let raw = if mult.abs() > 1e-10 {
                    ((val - off) / mult).round()
                } else {
                    0.0
                };
                if is_32bit {
                    buf.extend_from_slice(&(raw as i32).to_le_bytes());
                } else {
                    // clamp to i16 range
                    let clamped = raw.clamp(i16::MIN as f64, i16::MAX as f64) as i16;
                    buf.extend_from_slice(&clamped.to_le_bytes());
                }
            }
        }

        // 状态量：按字打包
        for w in 0..status_word_count {
            let mut word = 0u16;
            for bit in 0..16 {
                let ch = w * 16 + bit;
                if ch < status_count {
                    let val = dat.statuses[ch].get(row).copied().unwrap_or(0);
                    if val != 0 {
                        word |= 1 << bit;
                    }
                }
            }
            buf.extend_from_slice(&word.to_le_bytes());
        }
    }

    buf
}

fn read_4(data: &[u8], offset: usize) -> Result<[u8; 4]> {
    let slice = data
        .get(offset..offset + 4)
        .ok_or_else(|| Error::binary(FileRole::Dat, offset, "数据越界"))?;
    Ok([slice[0], slice[1], slice[2], slice[3]])
}

fn read_2(data: &[u8], offset: usize) -> Result<[u8; 2]> {
    let slice = data
        .get(offset..offset + 2)
        .ok_or_else(|| Error::binary(FileRole::Dat, offset, "数据越界"))?;
    Ok([slice[0], slice[1]])
}
