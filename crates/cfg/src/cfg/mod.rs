//! CFG 配置文件读写模块
//!
//! 支持 COMTRADE 1991/1999 版本 CFG 文件的解析与序列化。

mod channel;
mod config;
mod data_type;
mod header;
mod reference;
mod sampling;
mod tran_side;
mod version;

pub use channel::{parse_f64_or, AnalogChannel, AnalogExt, StatusChannel, StatusExt};
pub use config::{Config, SamplingTimeQuality, TimeInfo};
pub use data_type::DataType;
pub use header::{ChannelCount, Header};
pub use reference::{is_iec61850_reference, is_reference_like_ccbm};
pub use sampling::{Sampling, Segment, DEFAULT_NOMINAL_FREQ};
pub use tran_side::TranSide;
pub use version::Version;
