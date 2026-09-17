//! COMTRADE 顶层聚合模型：Comtrade 文件组加载与写出。

pub mod comtrade;
pub mod hdr;

pub use crate::comtrade::{ChannelKind, ChannelView, Comtrade, ComtradePaths};
pub use crate::hdr::HdrFile;
