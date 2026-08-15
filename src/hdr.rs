//! HDR 自由文本文件（预留，格式不固定暂未实现读写）
//!
//! 一期只定义类型与 API 位，读写返回 `Error::UnsupportedFormat`。

use std::path::Path;

use crate::error::{Error, Result};

/// HDR 自由文本文件（预留）
#[derive(Debug, Clone, Default)]
pub struct HdrFile {
    /// 文本内容（预留字段）
    pub content: String,
}

impl HdrFile {
    /// 预留：读取 HDR 文件
    pub fn from_file(_path: &Path) -> Result<HdrFile> {
        Err(Error::UnsupportedFormat("hdr 格式不固定，暂未实现".into()))
    }

    /// 预留：写出 HDR 文件
    pub fn write_file(&self, _path: &Path) -> Result<()> {
        Err(Error::UnsupportedFormat("hdr 格式不固定，暂未实现".into()))
    }
}
