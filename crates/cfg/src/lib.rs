//! cfg：COMTRADE CFG 配置文件的解析与序列化。
//!
//! 可单独使用（仅解析 CFG），也可经根包 `comtrade-io` facade 整体使用。

pub mod cfg;

pub use cfg::{
    is_iec61850_reference, is_reference_like_ccbm, AnalogChannel, AnalogExt, ChannelCount, Config,
    DataType, Header, Sampling, SamplingTimeQuality, Segment, StatusChannel, StatusExt, TimeInfo,
    TranSide, Version, DEFAULT_NOMINAL_FREQ,
};
