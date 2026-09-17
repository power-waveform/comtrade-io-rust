//! INF → Config 构建（由 File_Description + 通道节 + 参数段还原 CFG）。

use cbase::time;
use cfg::{
    is_iec61850_reference, AnalogChannel, AnalogExt, ChannelCount, Config, DataType, Header,
    Sampling, Segment, StatusChannel, StatusExt, TranSide, Version,
};

use super::section::{InfFile, SectionKind};

/// 由 INF 节构建 Config
pub fn build_config(inf: &InfFile, _existing: Option<&Config>) -> Option<Config> {
    let fd = inf.file_description()?;

    // 头部
    let station = fd.get("Station_Name").unwrap_or("").to_string();
    let recorder = fd.get("Recording_Device_ID").unwrap_or("").to_string();
    let rev_year = fd.get("Revision_Year").unwrap_or("1991");
    // 同 CFG 头部：六个版本号全部识别，无法识别时回落 1991。
    // 原实现只匹配 "1999"，2001/2008/2013/2017 会被静默判成 1991。
    let version = Version::parse(rev_year).unwrap_or_default();

    // 通道数
    let total = fd
        .get("Total_Channel_Count")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let analog = fd
        .get("Analog_Channel_Count")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let status = fd
        .get("Status_Channel_Count")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    // 采样
    let freq = fd
        .get("Line_Frequency")
        .and_then(|s| s.parse().ok())
        .unwrap_or(50.0);
    let sr_count = fd
        .get("Sample_Rate_Count")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let mut segments = Vec::new();
    for i in 1..=sr_count {
        let rate_key = format!("Sample_Rate_#{}", i);
        let end_key = format!("End_Sample_Rate_#{}", i);
        let rate = fd
            .get(&rate_key)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);
        let end = fd.get(&end_key).and_then(|s| s.parse().ok()).unwrap_or(0);
        if rate > 0.0 {
            segments.push(Segment::new(rate, end));
        }
    }

    // 时间
    let start_time = fd
        .get("File_Start_Time")
        .and_then(|s| time::parse(s).ok())
        .unwrap_or_else(|| time::parse("01/01/2000,00:00:00.000000").unwrap());
    let trigger_time = fd
        .get("Trigger_Time")
        .and_then(|s| time::parse(s).ok())
        .unwrap_or(start_time);

    // 数据格式
    let file_type = fd.get("File_Type").unwrap_or("ASCII");
    let data_type = DataType::parse(file_type).unwrap_or(DataType::Ascii);

    // timemult
    let timemult = fd
        .get("Time_Multiplier")
        .and_then(|s| s.parse().ok())
        .unwrap_or(1.0);

    let mut config = Config {
        header: Header {
            station,
            recorder,
            version,
        },
        channels: ChannelCount {
            total,
            analog,
            status,
        },
        analogs: Vec::new(),
        statuses: Vec::new(),
        sampling: Sampling { freq, segments },
        start_time,
        trigger_time,
        data_type,
        timemult,
        time_info: None,
        sampling_time_quality: None,
    };

    // 构建通道
    build_channels_from_inf(inf, &mut config);

    // 合并参数段
    apply_parameters(inf, &mut config);

    Some(config)
}

fn build_channels_from_inf(inf: &InfFile, config: &mut Config) {
    // 模拟通道
    let analog_sections = inf.sections_of(&SectionKind::AnalogChannel);
    for section in &analog_sections {
        let index = section.index;
        let name = section.get("Channel_ID").unwrap_or("").to_string();
        let phase = section.get("Phase_ID").unwrap_or("").to_string();
        let monitored_component = section.get("Monitored_Component").unwrap_or("").trim();
        let (equipment, reference) = if is_iec61850_reference(monitored_component) {
            (String::new(), Some(monitored_component.to_string()))
        } else {
            (monitored_component.to_string(), None)
        };
        let unit = section.get("Channel_Units").unwrap_or("").to_string();
        let multiplier = section
            .get("Channel_Multiplier")
            .and_then(|s| s.parse().ok())
            .unwrap_or(1.0);
        let offset = section
            .get("Channel_Offset")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);
        let delay = section
            .get("Channel_Skew")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);
        let min_value = section
            .get("Range_Minimum_Limit_Value")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);
        let max_value = section
            .get("Range_Maximum_Limit_Value")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);
        let primary = section
            .get("Channel_Ratio_Primary")
            .and_then(|s| s.parse().ok())
            .unwrap_or(1.0);
        let secondary = section
            .get("Channel_Ratio_Secondary")
            .and_then(|s| s.parse().ok())
            .unwrap_or(1.0);
        let ps = section.get("Data_Primary_Secondary").unwrap_or("S");
        let tran_side = TranSide::parse(ps);

        config.analogs.push(AnalogChannel {
            index,
            name,
            phase,
            equipment,
            unit,
            multiplier,
            offset,
            delay,
            min_value,
            max_value,
            primary,
            secondary,
            tran_side,
            ext: reference.map(|reference| AnalogExt {
                reference: Some(reference),
                ..Default::default()
            }),
        });
    }

    // 状态通道
    let status_sections = inf.sections_of(&SectionKind::StatusChannel);
    for section in &status_sections {
        let index = section.index;
        let name = section.get("Channel_ID").unwrap_or("").to_string();
        let phase = section.get("Phase_ID").unwrap_or("").to_string();
        let monitored_component = section.get("Monitored_Component").unwrap_or("").trim();
        let (equipment, reference) = if is_iec61850_reference(monitored_component) {
            (String::new(), Some(monitored_component.to_string()))
        } else {
            (monitored_component.to_string(), None)
        };
        let contact = section
            .get("Normal_State")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        config.statuses.push(StatusChannel {
            index,
            name,
            phase,
            equipment,
            contact,
            ext: reference.map(|reference| StatusExt {
                reference: Some(reference),
                ..Default::default()
            }),
        });
    }
}

fn apply_parameters(inf: &InfFile, config: &mut Config) {
    // 模拟量参数段
    let param_sections = inf.sections_of(&SectionKind::AnalogChannelsParameter);
    for section in &param_sections {
        for (_key, value) in &section.fields {
            let parts: Vec<&str> = value.split(',').map(|s| s.trim()).collect();
            if parts.len() < 11 {
                continue;
            }
            let idx_cfg: usize = parts[0].parse().unwrap_or(0);
            let idx_org: usize = parts[1].parse().unwrap_or(0);
            // parts[2] 为通道名，CFG 中已有权威值，此处忽略
            let flag = parts[3].to_string();
            let freq_val: f64 = parts[4].parse().unwrap_or(50.0);
            let primary: f64 = parts[5].parse().unwrap_or(1.0);
            let secondary: f64 = parts[7].parse().unwrap_or(1.0);
            let au: f64 = parts[9].parse().unwrap_or(0.0);
            let bu: f64 = parts[10].parse().unwrap_or(0.0);

            if let Some(ch) = config.analogs.iter_mut().find(|a| a.index == idx_cfg) {
                let reference = ch.ext.as_ref().and_then(|e| e.reference.clone());
                ch.ext = Some(AnalogExt {
                    idx_org: Some(idx_org),
                    freq: Some(freq_val),
                    au: Some(au),
                    bu: Some(bu),
                    channel_type: None,
                    flag: Some(flag),
                    reference,
                });
                ch.primary = primary;
                ch.secondary = secondary;
            }
        }
    }

    // 状态量参数段
    let status_param_sections = inf.sections_of(&SectionKind::StatusChannelsParameter);
    for section in &status_param_sections {
        for (_key, value) in &section.fields {
            let parts: Vec<&str> = value.split(',').map(|s| s.trim()).collect();
            if parts.len() < 6 {
                continue;
            }
            let idx_cfg: usize = parts[0].parse().unwrap_or(0);
            let idx_org: usize = parts[1].parse().unwrap_or(0);
            let channel_type = parts[3].to_string();
            let flag = parts[4].to_string();
            let equipment_no = parts[5].to_string();

            if let Some(ch) = config.statuses.iter_mut().find(|s| s.index == idx_cfg) {
                let reference = ch.ext.as_ref().and_then(|e| e.reference.clone());
                ch.ext = Some(StatusExt {
                    idx_org: Some(idx_org),
                    channel_type: if channel_type.is_empty() {
                        None
                    } else {
                        Some(channel_type)
                    },
                    flag: if flag.is_empty() { None } else { Some(flag) },
                    contact: None,
                    reference,
                    equipment_no: if equipment_no.is_empty() {
                        None
                    } else {
                        Some(equipment_no)
                    },
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inf::section::Section;

    #[test]
    fn test_revision_year_all_versions() {
        // INF `Revision_Year` 与 CFG 头部同样支持六个版本号。
        // 原实现只匹配 "1999"，2001/2008/2013/2017 会被静默判成 1991。
        let build = |year: &str| -> Version {
            let inf = InfFile {
                sections: vec![Section {
                    area: "Public".into(),
                    kind: SectionKind::FileDescription,
                    index: 0,
                    fields: vec![
                        ("Station_Name".into(), "ST".into()),
                        ("Recording_Device_ID".into(), "REC".into()),
                        ("Revision_Year".into(), year.into()),
                    ],
                    raw: String::new(),
                }],
            };
            build_config(&inf, None).unwrap().header.version
        };

        for s in ["1991", "1999", "2001", "2008", "2013", "2017"] {
            assert_eq!(build(s).as_str(), s, "Revision_Year={} 应被识别", s);
        }
        // 无法识别时回落 1991
        assert_eq!(build("2020"), Version::V1991);
        assert_eq!(build(""), Version::V1991);
    }
}
