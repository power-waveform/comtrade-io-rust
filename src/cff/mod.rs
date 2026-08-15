//! CFF 单文件读写模块
//!
//! CFF 是 CFG+DAT+INF+HDR 的复合文件，通过字节级段标记切分。

mod section_splitter;

pub use section_splitter::CffSections;

use std::path::Path;

use crate::cfg::{Config, DataType};
use crate::dat::DatFile;
use crate::error::{Error, FileRole, Result};
use crate::inf::InfFile;

/// CFF 文件
#[derive(Debug, Clone, Default)]
pub struct CffFile {
    /// 各段内容
    pub sections: CffSections,
}

impl CffFile {
    /// 从文件读取 CFF
    pub fn from_file(path: &Path) -> Result<CffFile> {
        let bytes = std::fs::read(path).map_err(|e| Error::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
        Ok(CffFile {
            sections: section_splitter::extract_sections(&bytes),
        })
    }

    /// 从字节解析 CFF
    pub fn from_bytes(bytes: &[u8]) -> CffFile {
        CffFile {
            sections: section_splitter::extract_sections(bytes),
        }
    }

    /// CFG 段 → Config
    pub fn to_config(&self) -> Result<Config> {
        match &self.sections.cfg {
            Some(cfg_text) => Config::from_str(cfg_text),
            None => Err(Error::parse(FileRole::Cff, 0, "缺少 CFG 段")),
        }
    }

    /// DAT 段 → DatFile
    pub fn to_data(&self, config: &Config) -> Result<DatFile> {
        match config.data_type {
            DataType::Ascii => {
                let text = self
                    .sections
                    .dat
                    .as_ref()
                    .ok_or_else(|| Error::parse(FileRole::Cff, 0, "缺少 DAT 段"))?;
                DatFile::from_ascii(text, config)
            },
            _ => {
                let bytes = self
                    .sections
                    .dat_bytes
                    .as_ref()
                    .ok_or_else(|| Error::parse(FileRole::Cff, 0, "缺少 DAT 二进制段"))?;
                DatFile::from_bytes(bytes, config)
            },
        }
    }

    /// INF 段 → InfFile
    pub fn to_inf(&self) -> Option<InfFile> {
        self.sections
            .inf
            .as_ref()
            .map(|text| InfFile::from_str(text))
    }

    /// 由 Comtrade 数据编码为 CFF 字节流
    pub fn encode(cfg: &Config, dat: &DatFile, inf: Option<&InfFile>, dt: DataType) -> Vec<u8> {
        let mut buf = Vec::new();

        // 同步目标数据格式，保证 CFG 声明与 DAT 字节一致
        let mut cfg = cfg.clone();
        cfg.data_type = dt;

        // CFG 段
        buf.extend_from_slice(b"--- file type CFG ---\n");
        buf.extend_from_slice(cfg.to_string().as_bytes());
        buf.push(b'\n');

        // INF 段（可选）
        if let Some(inf_file) = inf {
            buf.extend_from_slice(b"--- file type INF ---\n");
            buf.extend_from_slice(inf_file.to_string().as_bytes());
            buf.push(b'\n');
        }

        // DAT 段
        buf.extend_from_slice(b"--- file type DAT ---\n");
        match dt {
            DataType::Ascii => {
                buf.extend_from_slice(dat.to_ascii(&cfg).as_bytes());
            },
            _ => {
                buf.extend_from_slice(&dat.to_bytes(&cfg, dt));
            },
        }

        buf
    }

    /// 写入 CFF 文件
    pub fn write_file(
        cfg: &Config,
        dat: &DatFile,
        inf: Option<&InfFile>,
        path: &Path,
        dt: DataType,
    ) -> Result<()> {
        let bytes = CffFile::encode(cfg, dat, inf, dt);
        std::fs::write(path, bytes).map_err(|e| Error::Io {
            path: path.to_path_buf(),
            source: e,
        })
    }
}
