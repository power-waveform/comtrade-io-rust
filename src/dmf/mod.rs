//! DMF 文件读写模块
//!
//! XML 格式的 COMTRADE 设备模型文件，命名空间前缀无关。

mod xml_reader;
mod xml_writer;

use std::path::Path;

use crate::encoding;
use crate::equipment::{
    Bus, EquipmentGroup, Exciter, Generator, Line, Transformer, TransformerWinding,
};
use crate::error::{Error, Result};

/// DMF 模拟通道
#[derive(Debug, Clone, Default)]
pub struct DmfAnalogChannel {
    /// CFG 通道索引
    pub idx_cfg: usize,
    /// 原始通道索引
    pub idx_org: usize,
    /// 通道类型（如 AC 电压/电流）
    pub ch_type: String,
    /// 通道标志
    pub flag: String,
    /// 通道一次侧量程最小值（仅直流类型有效，DL/T 553-2013 表 B.3）
    pub p_min: f64,
    /// 通道一次侧量程最大值（仅直流类型有效，DL/T 553-2013 表 B.3）
    pub p_max: f64,
    /// 通道二次侧量程最小值（仅直流类型有效，DL/T 553-2013 表 B.3）
    pub s_min: f64,
    /// 通道二次侧量程最大值（仅直流类型有效，DL/T 553-2013 表 B.3）
    pub s_max: f64,
    /// 额定频率（Hz）
    pub freq: f64,
    /// 幅值系数 a
    pub au: f64,
    /// 幅值系数 b
    pub bu: f64,
    /// 物理单位
    pub unit: String,
    /// 倍率
    pub multiplier: f64,
    /// 一次值
    pub primary: f64,
    /// 二次值
    pub secondary: f64,
    /// 一次/二次标志
    pub ps: String,
    /// 相别（如 A/B/C）
    pub ph: String,
}

/// DMF 状态通道
#[derive(Debug, Clone, Default)]
pub struct DmfStatusChannel {
    /// CFG 通道索引
    pub idx_cfg: usize,
    /// 原始通道索引
    pub idx_org: usize,
    /// 通道类型
    pub ch_type: String,
    /// 通道标志
    pub flag: String,
    /// 触点状态说明
    pub contact: String,
    /// 源引用
    pub src_ref: String,
}

/// DMF 文件模型
#[derive(Debug, Clone, Default)]
pub struct DmfFile {
    /// 厂站名称
    pub station_name: String,
    /// 文件版本
    pub version: String,
    /// 参考编号
    pub reference: String,
    /// 录波设备名称
    pub rec_dev_name: String,
    /// 录波装置 RDRE 参引（DL/T 553-2013 表 B.2 `Rec_ref`，可选）
    pub rec_ref: String,
    /// 模拟通道列表
    pub analogs: Vec<DmfAnalogChannel>,
    /// 状态通道列表
    pub statuses: Vec<DmfStatusChannel>,
    /// 母线列表
    pub buses: Vec<Bus>,
    /// 线路列表
    pub lines: Vec<Line>,
    /// 变压器列表
    pub transformers: Vec<Transformer>,
    /// 发电机（§1.6）
    pub generators: Vec<Generator>,
    /// 励磁机（§1.7）
    pub exciters: Vec<Exciter>,
}

impl DmfFile {
    /// 从字符串解析 DMF（XML）
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(text: &str) -> Result<DmfFile> {
        let bytes = text.as_bytes();
        xml_reader::parse_dmf(bytes)
    }

    /// 从文件读取 DMF
    pub fn from_file(path: &Path) -> Result<DmfFile> {
        let bytes = std::fs::read(path).map_err(|e| Error::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
        xml_reader::parse_dmf(&bytes)
    }

    /// 序列化为 DMF XML 文本
    #[allow(clippy::inherent_to_string)]
    pub fn to_string(&self) -> String {
        xml_writer::write_dmf(self)
    }

    /// 写入 DMF 文件（UTF-8）
    pub fn write_file(&self, path: &Path) -> Result<()> {
        encoding::write_text(path, &self.to_string(), encoding::Encoding::Utf8)
    }

    /// 转换为设备组
    pub fn to_equipment_group(&self) -> EquipmentGroup {
        EquipmentGroup {
            buses: self.buses.clone(),
            lines: self.lines.clone(),
            transformers: self.transformers.clone(),
            generators: self.generators.clone(),
            exciters: self.exciters.clone(),
        }
    }
}
