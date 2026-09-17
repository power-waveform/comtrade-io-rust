//! INF（信息）文件解析与序列化，含设备模型（Bus/Line/Transformer/Generator/Exciter）。

pub mod inf;

pub use crate::inf::from_config as inf_from_config;
pub use crate::inf::{InfFile, Section, SectionKind};
