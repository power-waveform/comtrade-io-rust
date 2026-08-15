//! COMTRADE 顶层聚合模型与文件组入口

use std::path::{Path, PathBuf};

use crate::cff::CffFile;
use crate::cfg::{Config, DataType};
use crate::dat::{DatFile, StatusChange};
use crate::dfr::DfrFile;
use crate::dmf::DmfFile;
use crate::equipment::EquipmentGroup;
use crate::error::{Error, Result};
use crate::hdr::HdrFile;
use crate::inf::InfFile;

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
    pub definition: &'a crate::cfg::AnalogChannel,
    /// 该通道的采样数据（只读引用）
    pub samples: &'a [f64],
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
    /// 从任一成员文件路径加载整组（CFG+DAT 必需）
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Comtrade> {
        let path = path.as_ref();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        // 单文件模式
        if ext == "cff" {
            return Comtrade::from_cff(path);
        }
        if ext == "dfr" {
            return Comtrade::from_dfr(path);
        }

        // 多文件模式：定位兄弟文件
        let parent = path.parent().unwrap_or(Path::new("."));
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let is_upper = path
            .extension()
            .map(|e| e.to_str().unwrap_or("").chars().all(|c| c.is_uppercase()))
            .unwrap_or(false);

        let ext_style = |suffix: &str| -> String {
            if is_upper {
                suffix.to_uppercase()
            } else {
                suffix.to_string()
            }
        };

        let paths = ComtradePaths {
            dir: parent.to_path_buf(),
            stem: stem.to_string(),
            cfg: resolve_path(parent, stem, &ext_style("cfg")),
            dat: resolve_path(parent, stem, &ext_style("dat")),
            hdr: resolve_path(parent, stem, &ext_style("hdr")),
            inf: resolve_path(parent, stem, &ext_style("inf")),
            dmf: resolve_path(parent, stem, &ext_style("dmf")),
        };

        // CFG 必需
        let cfg_path = paths.cfg.as_ref().ok_or_else(|| {
            Error::NotFound(parent.join(format!("{}.{}", stem, ext_style("cfg"))))
        })?;
        let config = Config::from_file(cfg_path)?;

        // DAT 必需
        let dat_path = paths.dat.as_ref().ok_or_else(|| {
            Error::NotFound(parent.join(format!("{}.{}", stem, ext_style("dat"))))
        })?;
        let data = DatFile::from_file(dat_path, &config)?;

        // INF 可选
        let inf = paths.inf.as_ref().and_then(|p| InfFile::from_file(p).ok());

        // DMF 可选
        let dmf = paths.dmf.as_ref().and_then(|p| DmfFile::from_file(p).ok());

        // HDR 仅记录路径
        let hdr = None;

        // 设备组：DMF 优先，否则 INF
        let equipment = if let Some(ref dmf) = dmf {
            dmf.to_equipment_group()
        } else if let Some(ref inf) = inf {
            inf.to_equipment_group()
        } else {
            EquipmentGroup::default()
        };

        Ok(Comtrade {
            config,
            data: Some(data),
            hdr,
            inf,
            dmf,
            equipment,
            paths,
        })
    }

    /// 从 CFF 单文件加载
    pub fn from_cff<P: AsRef<Path>>(path: P) -> Result<Comtrade> {
        let path = path.as_ref();
        let cff = CffFile::from_file(path)?;
        let config = cff.to_config()?;
        let data = cff.to_data(&config)?;
        let inf = cff.to_inf();
        let hdr = cff
            .sections
            .hdr
            .as_ref()
            .map(|s| HdrFile { content: s.clone() });

        let equipment = if let Some(ref inf) = inf {
            inf.to_equipment_group()
        } else {
            EquipmentGroup::default()
        };

        let parent = path.parent().unwrap_or(Path::new("."));
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");

        Ok(Comtrade {
            config,
            data: Some(data),
            hdr,
            inf,
            dmf: None,
            equipment,
            paths: ComtradePaths {
                dir: parent.to_path_buf(),
                stem: stem.to_string(),
                ..Default::default()
            },
        })
    }

    /// 从 DFR 单文件加载
    pub fn from_dfr<P: AsRef<Path>>(path: P) -> Result<Comtrade> {
        let path = path.as_ref();
        let dfr = DfrFile::from_file(path)?;
        let time = dfr.file_time;
        let config = dfr.to_config(time);
        let data = dfr.to_data(&config)?;

        let parent = path.parent().unwrap_or(Path::new("."));
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");

        Ok(Comtrade {
            config,
            data: Some(data),
            hdr: None,
            inf: None,
            dmf: None,
            equipment: EquipmentGroup::default(),
            paths: ComtradePaths {
                dir: parent.to_path_buf(),
                stem: stem.to_string(),
                ..Default::default()
            },
        })
    }

    /// 仅加载配置（不读 DAT）
    pub fn load_config<P: AsRef<Path>>(path: P) -> Result<Config> {
        Config::from_file(path.as_ref())
    }

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

    /// 整组写出到目录
    pub fn write_to_dir(&self, dir: &Path, stem: &str, dt: DataType) -> Result<()> {
        std::fs::create_dir_all(dir).map_err(|e| Error::Io {
            path: dir.to_path_buf(),
            source: e,
        })?;

        // 同步目标数据格式，保证 CFG 声明与 DAT 字节一致
        let mut cfg = self.config.clone();
        cfg.data_type = dt;

        // CFG
        let cfg_path = dir.join(format!("{}.cfg", stem));
        cfg.write_file(&cfg_path)?;

        // DAT
        if let Some(ref dat) = self.data {
            let dat_path = dir.join(format!("{}.dat", stem));
            dat.write_file(&dat_path, &cfg, dt)?;
        }

        // INF（可选）
        if let Some(ref inf) = self.inf {
            let inf_path = dir.join(format!("{}.inf", stem));
            inf.write_file(&inf_path)?;
        }

        // DMF（可选）
        if let Some(ref dmf) = self.dmf {
            let dmf_path = dir.join(format!("{}.dmf", stem));
            dmf.write_file(&dmf_path)?;
        }

        Ok(())
    }
}

/// 解析文件路径（存在则返回）
fn resolve_path(parent: &Path, stem: &str, ext: &str) -> Option<PathBuf> {
    let path = parent.join(format!("{}.{}", stem, ext));
    if path.exists() {
        Some(path)
    } else {
        None
    }
}
