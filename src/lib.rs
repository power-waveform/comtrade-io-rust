//! COMTRADE (IEEE C37.111) 故障录波文件解析与导出库
//!
//! 支持 CFG / DAT / INF / DMF / CFF / DFR 格式的读写操作。
//!
//! # 使用示例
//!
//! ```no_run
//! use comtrade_io::{Comtrade, ExportFormat, DataType};
//!
//! // 多文件组加载
//! let ct = Comtrade::from_path("tests/data/binary_1999.cfg").unwrap();
//! println!("站名: {}", ct.config.header.station);
//!
//! // 导出为 JSON
//! ct.save("output.json".as_ref(), ExportFormat::Json, DataType::Ascii).unwrap();
//! ```

#![warn(missing_docs)]

// 核心模块
pub mod encoding;
mod error;
mod time;

// 格式模块
mod cff;
mod cfg;
mod dat;
mod dfr;
mod dmf;
mod hdr;
mod inf;

// 设备模型
mod equipment;

// 导出器
mod exporters;

// 顶层聚合
mod comtrade;

// 公共 API 再导出
pub use cff::{CffFile, CffSections};
pub use cfg::{
    is_iec61850_reference, is_reference_like_ccbm, AnalogChannel, AnalogExt, ChannelCount, Config,
    DataType, Header, Sampling, SamplingTimeQuality, Segment, StatusChannel, StatusExt, TimeInfo,
    TranSide, Version, DEFAULT_NOMINAL_FREQ,
};
pub use comtrade::{ChannelView, Comtrade, ComtradePaths};
pub use dat::{recalculate_segments, DatFile, StatusChange};
pub use dfr::{DfrBinary, DfrFile, WndrSection};
pub use dmf::{DmfAnalogChannel, DmfFile, DmfStatusChannel};
pub use encoding::Encoding;
pub use equipment::{
    AccBran, AcvChn, BranchNum, Bus, Capacitance, EquipmentGroup, Exciter, Generator, Igap,
    Impedance, Line, MutualInductance, Px, SyncReactance, Transformer, TransformerWinding, Ufe,
    UfeChns, UnChns, WindingLocation,
};
pub use error::{Error, FileRole, Result};
pub use exporters::{to_csv, to_json, ExportFormat, ExportOptions};
pub use hdr::HdrFile;
pub use inf::{from_config as inf_from_config, InfFile, Section, SectionKind};
pub use time::Timestamp;

/// 库版本号（取自 Cargo.toml）
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
