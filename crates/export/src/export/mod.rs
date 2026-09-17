//! 导出器模块

mod csv;
mod json;

pub use csv::to_csv;
pub use json::to_json;

use std::path::Path;

use cbase::encoding::{self, Encoding};
use cbase::error::{Error, Result};
use cff::CffFile;
use cfg::DataType;
use model::Comtrade;

/// 导出格式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    /// 多文件导出（CFG+DAT+INF+DMF）
    MultiFile,
    /// CFF 复合文件
    Cff,
    /// JSON 文件
    Json,
    /// CSV 文件
    Csv,
}

impl ExportFormat {
    /// 从字符串解析（忽略大小写）
    pub fn parse(s: &str) -> Option<ExportFormat> {
        match s.to_lowercase().as_str() {
            "multi_file" | "multifile" => Some(ExportFormat::MultiFile),
            "cff" => Some(ExportFormat::Cff),
            "json" => Some(ExportFormat::Json),
            "csv" => Some(ExportFormat::Csv),
            _ => None,
        }
    }

    /// 目标文件后缀
    pub fn suffix(&self) -> &'static str {
        match self {
            ExportFormat::MultiFile => ".cfg",
            ExportFormat::Cff => ".cff",
            ExportFormat::Json => ".json",
            ExportFormat::Csv => ".csv",
        }
    }
}

/// 导出选项
#[derive(Debug, Clone)]
pub struct ExportOptions {
    /// 目标 DAT 数据类型
    pub data_type: DataType,
    /// JSON 缩进宽度（None 表示不格式化）
    pub json_indent: Option<usize>,
    /// 是否输出 CSV 表头
    pub csv_headers: bool,
}

impl Default for ExportOptions {
    fn default() -> Self {
        ExportOptions {
            data_type: DataType::Binary,
            json_indent: Some(2),
            csv_headers: true,
        }
    }
}

/// 统一导出入口（由原来的 `impl Comtrade { save/save_with }` 转为 trait 以解耦成环）
pub trait Export {
    /// 统一导出入口
    fn save(&self, path: &Path, format: ExportFormat, dt: DataType) -> Result<()>;

    /// 带选项的导出
    fn save_with(&self, path: &Path, format: ExportFormat, opts: &ExportOptions) -> Result<()>;
}

impl Export for Comtrade {
    fn save(&self, path: &Path, format: ExportFormat, dt: DataType) -> Result<()> {
        let opts = ExportOptions {
            data_type: dt,
            ..Default::default()
        };
        self.save_with(path, format, &opts)
    }

    fn save_with(&self, path: &Path, format: ExportFormat, opts: &ExportOptions) -> Result<()> {
        let path = ensure_suffix(path, format.suffix());

        match format {
            ExportFormat::Json => {
                let text = json::to_json(self, opts.json_indent);
                encoding::write_text(&path, &text, Encoding::Utf8)
            },
            ExportFormat::Csv => {
                let text = csv::to_csv(self, opts.csv_headers);
                encoding::write_text(&path, &text, Encoding::Utf8)
            },
            ExportFormat::Cff => {
                if let Some(ref dat) = self.data {
                    CffFile::write_file(&self.config, dat, self.inf.as_ref(), &path, opts.data_type)
                } else {
                    Err(Error::UnsupportedFormat("无 DAT 数据，无法导出 CFF".into()))
                }
            },
            ExportFormat::MultiFile => {
                // 多文件导出：以 path 为目录
                let stem = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("output")
                    .to_string();
                let dir: std::path::PathBuf = if path.is_dir() {
                    path
                } else {
                    path.parent().unwrap_or(Path::new(".")).to_path_buf()
                };
                self.write_to_dir(&dir, &stem, opts.data_type)
            },
        }
    }
}

/// 确保路径有正确的后缀
fn ensure_suffix(path: &Path, suffix: &str) -> std::path::PathBuf {
    if path.extension().map(|e| e.to_str().unwrap_or("")) == Some(suffix.trim_start_matches('.')) {
        path.to_path_buf()
    } else {
        let mut p = path.to_path_buf();
        p.set_extension(&suffix[1..]); // 去掉开头的 '.'
        p
    }
}
