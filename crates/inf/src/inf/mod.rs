//! INF 信息文件读写模块
//!
//! INI 风格节解析，支持 File_Description / 通道节 / 参数段 / 设备节。

pub mod builder;
pub mod equipment;
pub mod from_config;
pub mod parse;
pub mod section;
pub mod serialize;

use std::path::Path;

use cbase::encoding::{self, Encoding};
use cbase::equipment::EquipmentGroup;
use cbase::error::Result;

use crate::inf::parse::build_config;

pub use section::{InfFile, Section, SectionKind};

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
    pub fn to_config(&self, existing: Option<&cfg::Config>) -> Option<cfg::Config> {
        build_config(self, existing)
    }

    /// 构建设备组（Bus/Line/Transformer/Generator/Exciter）
    pub fn to_equipment_group(&self) -> EquipmentGroup {
        equipment::build_equipment_group(self)
    }

    /// 写入 INF 文件（默认 GBK）
    pub fn write_file(&self, path: &Path) -> Result<()> {
        encoding::write_text(path, &self.to_string(), Encoding::Gbk)
    }
}

/// 由 Config + 设备组生成 INF
pub use from_config::from_config;
