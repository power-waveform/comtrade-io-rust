//! COMTRADE (IEEE C37.111) 故障录波文件解析与导出库
//!
//! 根包作为 **facade**，把多 crate 工作区（`crates/`）的全部公共 API 统一 re-export，
//! 保持兼容的单一入口 `comtrade_io`；同时各子 crate 也可单独依赖使用。
//!
//! - 格式解析：`cfg` / `dat` / `inf` / `dmf` /
//!   `cff` / `dfr`（DFR 只读）
//! - 基础类型：`cbase`（Error / Timestamp / Encoding / equipment）
//! - 聚合模型与加载：`model`（`Comtrade`）
//! - 导出：`export`（`Export` trait）
//! - 编辑：`edit`（`DataEdit` trait）
//!
//! # 使用示例
//!
//! ```no_run
//! use comtrade_io::{Comtrade, DataType, Export, ExportFormat};
//!
//! // 多文件组加载
//! let ct = Comtrade::from_path("tests/data/binary_1999.cfg").unwrap();
//! println!("站名: {}", ct.config.header.station);
//!
//! // 导出为 JSON
//! ct.save("output.json".as_ref(), ExportFormat::Json, DataType::Ascii).unwrap();
//! ```

#![warn(missing_docs)]

// ---- 基础类型（cbase）----
pub use cbase::encoding;
pub use cbase::encoding::Encoding;
pub use cbase::equipment::{
    AccBran, AcvChn, BranchNum, Bus, Capacitance, EquipmentGroup, Exciter, Generator, Igap,
    Impedance, Line, MutualInductance, Px, SyncReactance, Transformer, TransformerWinding, Ufe,
    UfeChns, UnChns, WindingLocation,
};
pub use cbase::{Error, FileRole, Result, Timestamp};

// ---- 格式模块 ----
pub use cff::{CffFile, CffSections};
pub use cfg::{
    is_iec61850_reference, is_reference_like_ccbm, AnalogChannel, AnalogExt, ChannelCount, Config,
    DataType, Header, Sampling, SamplingTimeQuality, Segment, StatusChannel, StatusExt, TimeInfo,
    TranSide, Version, DEFAULT_NOMINAL_FREQ,
};
pub use dat::{recalculate_segments, DatFile, StatusChange};
pub use dfr::{DfrBinary, DfrFile, WndrSection};
pub use dmf::{DmfAnalogChannel, DmfFile, DmfStatusChannel};
pub use inf::inf_from_config;
pub use inf::{InfFile, Section, SectionKind};

// ---- 聚合模型 ----
pub use model::{ChannelKind, ChannelView, Comtrade, ComtradePaths, HdrFile};

// ---- 导出与编辑 ----
pub use edit::DataEdit;
pub use export::{to_csv, to_json, Export, ExportFormat, ExportOptions};

/// 库版本号（取自 Cargo.toml）
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
