//! DAT 数据文件读写模块
//!
//! 支持 ASCII / BINARY / BINARY32 / FLOAT32 格式的解析与写出。
//! 采用列式存储：每列独立 Vec，对齐 Python DataFrame 的按列访问模式。

mod ascii;
mod binary;
mod changes;
mod normalize;
mod recalc;

pub use changes::StatusChange;
pub use recalc::recalculate_segments;

use std::path::Path;

use cbase::encoding;
use cbase::error::{Error, Result};
use cfg::{Config, DataType};

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
        dat = normalize::fit_to_config(dat, cfg)?;
        // 应用 timemult
        normalize::apply_timemult(&mut dat, cfg);
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
}
