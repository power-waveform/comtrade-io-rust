//! COMTRADE 顶层聚合模型：结构定义与数据访问。

mod load;
mod write;

use std::path::PathBuf;

use cbase::equipment::EquipmentGroup;
use cfg::Config;
use dat::{DatFile, StatusChange};
use dmf::DmfFile;
use inf::InfFile;

use crate::hdr::HdrFile;

/// 文件组路径信息（存在性已知）
#[derive(Debug, Clone, Default)]
pub struct ComtradePaths {
    /// 文件所在目录
    pub dir: PathBuf,
    /// 主文件名（不含扩展名）
    pub stem: String,
    /// CFG 文件路径（存在则 Some）
    pub cfg: Option<PathBuf>,
    /// DAT 文件路径（存在则 Some）
    pub dat: Option<PathBuf>,
    /// HDR 文件路径（存在则 Some）
    pub hdr: Option<PathBuf>,
    /// INF 文件路径（存在则 Some）
    pub inf: Option<PathBuf>,
    /// DMF 文件路径（存在则 Some）
    pub dmf: Option<PathBuf>,
}

/// 通道数据视图（零拷贝）
#[derive(Debug, Clone)]
pub struct ChannelView<'a> {
    /// 通道定义（只读引用）
    pub definition: &'a cfg::AnalogChannel,
    /// 该通道的采样数据（只读引用）
    pub samples: &'a [f64],
}

/// 编辑操作中标识通道种类
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelKind {
    /// 模拟量通道
    Analog,
    /// 状态量通道
    Status,
}

/// COMTRADE 数据模型
#[derive(Debug, Clone, Default)]
pub struct Comtrade {
    /// CFG 配置（必需）
    pub config: Config,
    /// DAT 采样数据（必需）
    pub data: Option<DatFile>,
    /// HDR 头文件（仅记录路径，未解析）
    pub hdr: Option<HdrFile>,
    /// INF 信息文件（可选）
    pub inf: Option<InfFile>,
    /// DMF 设备模型文件（可选）
    pub dmf: Option<DmfFile>,
    /// 设备拓扑（由 DMF 优先，否则 INF 构建）
    pub equipment: EquipmentGroup,
    /// 文件组路径信息
    pub paths: ComtradePaths,
}

impl Comtrade {
    /// 数据访问：获取模拟通道
    pub fn analog_channel(&self, index: usize) -> Option<ChannelView<'_>> {
        let def = self.config.analog(index)?;
        let data = self.data.as_ref()?;
        let samples = data.analog_samples(index - 1)?;
        Some(ChannelView {
            definition: def,
            samples,
        })
    }

    /// 变位检测
    pub fn changed_statuses(&self) -> Vec<(usize, Vec<StatusChange>)> {
        if let Some(ref dat) = self.data {
            dat.changed_statuses(Some(self.config.start_time))
        } else {
            Vec::new()
        }
    }
}
