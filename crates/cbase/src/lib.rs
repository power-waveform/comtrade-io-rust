//! cbase：COMTRADE 共享地基
//!
//! 提供统一错误类型、文本编码探测/转码、轻量时间类型、设备拓扑模型。
//! 被各格式 crate 与聚合模型 crate 复用，不依赖任何业务格式。

pub mod encoding;
pub mod equipment;
pub mod error;
pub mod time;

pub use crate::error::{Error, FileRole, Result};
pub use crate::time::Timestamp;
