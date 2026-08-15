//! INF 信息文件读写模块
//!
//! INI 风格节解析，支持 File_Description / 通道节 / 参数段 / 设备节。

mod builder;
mod section;

pub use section::{InfFile, Section, SectionKind};

use std::path::Path;

use crate::cfg::Config;
use crate::encoding::{self, Encoding};
use crate::equipment::EquipmentGroup;
use crate::error::Result;

impl InfFile {
    /// 从文件读取 INF
    pub fn from_file(path: &Path) -> Result<InfFile> {
        let (text, _) = encoding::read_text_gbk(path)?;
        Ok(InfFile::from_str(&text))
    }

    /// 从字符串解析 INF
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(text: &str) -> InfFile {
        section::parse_sections(text)
    }

    /// 构建 Config（由 File_Description + 通道节）
    pub fn to_config(&self, existing: Option<&Config>) -> Option<Config> {
        builder::build_config(self, existing)
    }

    /// 构建设备组（Bus/Line/Transformer）
    pub fn to_equipment_group(&self) -> EquipmentGroup {
        builder::build_equipment_group(self)
    }

    /// 序列化为 INF 文本
    #[allow(clippy::inherent_to_string)]
    pub fn to_string(&self) -> String {
        let mut lines = Vec::new();
        for section in &self.sections {
            lines.push(section.to_text());
            lines.push(String::new());
        }
        lines.join("\n")
    }

    /// 写入 INF 文件（默认 GBK）
    pub fn write_file(&self, path: &Path) -> Result<()> {
        encoding::write_text(path, &self.to_string(), Encoding::Gbk)
    }
}

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
