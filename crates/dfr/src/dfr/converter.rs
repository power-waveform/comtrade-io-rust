//! DFR WNDR → Config 转换器

use super::wndr::WndrSection;
use super::DEFAULT_END_POINT;
use cbase::Timestamp;
use cfg::{AnalogChannel, Config, DataType, Sampling, Segment, StatusChannel, TranSide, Version};

/// WNDR → Config 转换
pub fn wndr_to_config(wndr: &WndrSection, time: Timestamp) -> Config {
    let analog_count = wndr.analog_channels.len();
    let status_count = wndr.status_channels.len();

    let samp_rate = (wndr.samples_per_cycle as f64) * wndr.grid_freq;
    let end_point = if wndr.total_samples > 0 {
        wndr.total_samples
    } else {
        DEFAULT_END_POINT
    };

    let mut analogs = Vec::new();
    for ch in &wndr.analog_channels {
        let unit = convert_unit(&ch.unit, ch.ch_type);
        let primary = convert_primary(ch.rated_primary, ch.ch_type);
        let phase = convert_phase(&ch.phase);
        let min_value = -(wndr.full_scale as f64);
        let max_value = wndr.full_scale as f64;

        analogs.push(AnalogChannel {
            index: ch.idx,
            name: ch.name.clone(),
            phase,
            equipment: ch.monitor_loop.clone(),
            unit,
            multiplier: ch.conversion_factor,
            offset: 0.0,
            delay: 0.0,
            min_value,
            max_value,
            primary,
            secondary: 1.0,
            tran_side: TranSide::S,
            ext: None,
        });
    }

    let mut statuses = Vec::new();
    for ch in &wndr.status_channels {
        statuses.push(StatusChannel {
            index: ch.idx,
            name: ch.name.clone(),
            phase: String::new(),
            equipment: String::new(),
            contact: 0,
            ext: None,
        });
    }

    Config {
        header: cfg::Header {
            station: wndr.station_name.clone(),
            recorder: String::new(),
            version: Version::V1999,
        },
        channels: cfg::ChannelCount {
            total: analog_count + status_count,
            analog: analog_count,
            status: status_count,
        },
        analogs,
        statuses,
        sampling: Sampling {
            freq: wndr.grid_freq,
            segments: vec![Segment::new(samp_rate, end_point)],
        },
        start_time: time,
        trigger_time: time,
        data_type: DataType::Binary,
        timemult: 1.0,
        time_info: None,
        sampling_time_quality: None,
    }
}

/// 单位转换：ch_type≤10→A, =100→kV
fn convert_unit(unit: &str, ch_type: u32) -> String {
    if ch_type <= 10 {
        "A".to_string()
    } else if ch_type == 100 {
        "kV".to_string()
    } else {
        unit.to_string()
    }
}

/// 一次值缩放：kV 且 >10000 → /1000
fn convert_primary(primary: f64, ch_type: u32) -> f64 {
    if ch_type == 100 && primary > 10000.0 {
        primary / 1000.0
    } else {
        primary
    }
}

/// 西里尔相别映射：а/в/с → A/B/C
fn convert_phase(phase: &str) -> String {
    match phase.to_lowercase().as_str() {
        "а" | "a" => "A".to_string(),
        "в" | "b" => "B".to_string(),
        "с" | "c" => "C".to_string(),
        _ => phase.to_uppercase(),
    }
}
