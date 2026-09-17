//! DMF（设备模型）XML 格式解析与写出，可独立使用。

pub mod dmf;

pub use crate::dmf::{DmfAnalogChannel, DmfFile, DmfStatusChannel};
pub use cbase::equipment;
