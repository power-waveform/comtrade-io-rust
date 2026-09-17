//! dat：COMTRADE DAT 数据文件读写（ASCII / BINARY / BINARY32 / FLOAT32）。
//!
//! 可单独使用（仅解析 DAT，需配合 cfg 的 Config），
//! 也可经根包 `comtrade-io` facade 整体使用。

pub mod dat;

pub use dat::{recalculate_segments, DatFile, StatusChange};
