//! 由 Config + 设备组生成 INF。

use cbase::equipment::EquipmentGroup;
use cfg::Config;

use super::builder;
use super::section::{InfFile, Section, SectionKind};

/// 由 Config + 设备组生成 INF
pub fn from_config(cfg: &Config, equipment: Option<&EquipmentGroup>) -> InfFile {
    let mut sections = Vec::new();

    // File_Description
    sections.push(builder::build_file_description(cfg));

    // 模拟通道节
    for analog in &cfg.analogs {
        sections.push(builder::build_analog_section(analog));
    }

    // 状态通道节
    for status in &cfg.statuses {
        sections.push(builder::build_status_section(status));
    }

    // 参数段
    if !cfg.analogs.is_empty() {
        let mut fields = Vec::new();
        for analog in &cfg.analogs {
            let param = builder::build_analog_parameter(analog);
            fields.push((format!("CHNL_INFO_#{}", analog.index), param));
        }
        sections.push(Section {
            area: "ZYHD".into(),
            kind: SectionKind::AnalogChannelsParameter,
            index: 0,
            fields,
            raw: String::new(),
        });
    }

    if !cfg.statuses.is_empty() {
        let mut fields = Vec::new();
        for status in &cfg.statuses {
            let param = builder::build_status_parameter(status);
            fields.push((format!("CHNL_INFO_#{}", status.index), param));
        }
        sections.push(Section {
            area: "ZYHD".into(),
            kind: SectionKind::StatusChannelsParameter,
            index: 0,
            fields,
            raw: String::new(),
        });
    }

    // 设备节
    if let Some(eg) = equipment {
        for bus in &eg.buses {
            sections.push(builder::build_bus_section(bus));
        }
        for line in &eg.lines {
            sections.push(builder::build_line_section(line));
        }
        for transformer in &eg.transformers {
            sections.push(builder::build_transformer_section(transformer));
        }
        for generator in &eg.generators {
            sections.push(builder::build_generator_section(generator));
        }
        for exciter in &eg.exciters {
            sections.push(builder::build_exciter_section(exciter));
        }
    }

    InfFile { sections }
}
