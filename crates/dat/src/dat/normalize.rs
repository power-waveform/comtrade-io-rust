//! DAT 形状对齐与 timemult 应用

use cbase::error::{Error, FileRole, Result};
use cfg::Config;

use super::DatFile;

/// 形状对齐：按 CFG 声明的通道数与采样点数截断/补齐（对齐 Python _validate_shape）
pub(crate) fn fit_to_config(mut dat: DatFile, cfg: &Config) -> Result<DatFile> {
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
pub(crate) fn apply_timemult(dat: &mut DatFile, cfg: &Config) {
    let timemult = cfg.timemult;
    if (timemult - 1.0).abs() > f64::EPSILON {
        for ts in &mut dat.timestamp_us {
            *ts *= timemult;
        }
    }
}
