//! COMTRADE 顶层聚合模型与文件组入口

use std::path::{Path, PathBuf};

use crate::cff::CffFile;
use crate::cfg::{Config, DataType};
use crate::dat::{recalculate_segments, DatFile, StatusChange};
use crate::dfr::DfrFile;
use crate::dmf::DmfFile;
use crate::equipment::EquipmentGroup;
use crate::error::{Error, FileRole, Result};
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

    // ========== 波形数据编辑写入口 ==========
    // 这些方法只改 `self.data` 的采样值与 `self.config` 的通道定义/计数，
    // 不改设备拓扑（`equipment`）——设备组引用由调用方（wave-tauri）负责重建。

    /// 替换某模拟量通道整列工程值。
    ///
    /// `channel` 为 0 基列下标（与 [`DatFile::analog_samples`] 一致）。
    /// `values` 长度必须与现有采样点数一致，否则返回错误、不修改任何数据。
    pub fn set_analog_column(&mut self, channel: usize, values: Vec<f64>) -> Result<()> {
        let data = self
            .data
            .as_mut()
            .ok_or_else(|| Error::parse(FileRole::Dat, 0, "文件不含采样数据"))?;
        let column = data
            .analogs
            .get_mut(channel)
            .ok_or_else(|| Error::parse(FileRole::Dat, 0, format!("模拟量通道下标越界: {channel}")))?;
        if values.len() != column.len() {
            return Err(Error::parse(
                FileRole::Dat,
                0,
                format!("新列长度 {} 与采样点数 {} 不一致", values.len(), column.len()),
            ));
        }
        *column = values;
        Ok(())
    }

    /// 替换某状态量通道整列（0/1）。
    ///
    /// `channel` 为 0 基列下标；`values` 长度须与采样点数一致，且每个值必须为 0 或 1。
    pub fn set_status_column(&mut self, channel: usize, values: Vec<u8>) -> Result<()> {
        let data = self
            .data
            .as_mut()
            .ok_or_else(|| Error::parse(FileRole::Dat, 0, "文件不含采样数据"))?;
        let column = data
            .statuses
            .get_mut(channel)
            .ok_or_else(|| Error::parse(FileRole::Dat, 0, format!("状态量通道下标越界: {channel}")))?;
        if values.len() != column.len() {
            return Err(Error::parse(
                FileRole::Dat,
                0,
                format!("新列长度 {} 与采样点数 {} 不一致", values.len(), column.len()),
            ));
        }
        if values.iter().any(|&v| v > 1) {
            return Err(Error::parse(
                FileRole::Dat,
                0,
                "状态量取值只能为 0 或 1",
            ));
        }
        *column = values;
        Ok(())
    }

    /// 删除一条通道（模拟量或状态量），原子同步 CFG 定义、通道计数与数据列。
    ///
    /// `index` 为 0 基列下标；删除后同种类其余通道的 CFG 1 基 `index` 重新编号，
    /// `config.channels.total/analog/status` 与 `data` 列数保持一致。
    pub fn remove_channel(&mut self, kind: ChannelKind, index: usize) -> Result<()> {
        let data = self
            .data
            .as_mut()
            .ok_or_else(|| Error::parse(FileRole::Dat, 0, "文件不含采样数据"))?;
        match kind {
            ChannelKind::Analog => {
                if index >= self.config.analogs.len() {
                    return Err(Error::parse(
                        FileRole::Cfg,
                        0,
                        format!("模拟量通道下标越界: {index}"),
                    ));
                }
                self.config.analogs.remove(index);
                data.analogs.remove(index);
                for (i, ch) in self.config.analogs.iter_mut().enumerate() {
                    ch.index = i + 1;
                }
                self.config.channels.analog = self.config.analogs.len();
            },
            ChannelKind::Status => {
                if index >= self.config.statuses.len() {
                    return Err(Error::parse(
                        FileRole::Cfg,
                        0,
                        format!("状态量通道下标越界: {index}"),
                    ));
                }
                self.config.statuses.remove(index);
                data.statuses.remove(index);
                for (i, ch) in self.config.statuses.iter_mut().enumerate() {
                    ch.index = i + 1;
                }
                self.config.channels.status = self.config.statuses.len();
            },
        }
        self.config.channels.total = self.config.channels.analog + self.config.channels.status;
        Ok(())
    }

    /// 在某模拟量通道下标之后插入一条新通道（含初始工程值列）。
    ///
    /// `after` 为 0 基下标，插入到 `after + 1` 处；`values` 长度须与采样点数一致。
    /// 插入后全部模拟量 CFG 1 基 `index` 重新编号，计数同步更新。
    pub fn insert_analog_channel(
        &mut self,
        after: usize,
        ch: crate::cfg::AnalogChannel,
        values: Vec<f64>,
    ) -> Result<()> {
        let data = self
            .data
            .as_mut()
            .ok_or_else(|| Error::parse(FileRole::Dat, 0, "文件不含采样数据"))?;
        if after >= self.config.analogs.len() {
            return Err(Error::parse(
                FileRole::Cfg,
                0,
                format!("模拟量插入位置越界: after={after}"),
            ));
        }
        if values.len() != data.len() {
            return Err(Error::parse(
                FileRole::Dat,
                0,
                format!("新列长度 {} 与采样点数 {} 不一致", values.len(), data.len()),
            ));
        }
        let insert_at = after + 1;
        self.config.analogs.insert(insert_at, ch);
        data.analogs.insert(insert_at, values);
        for (i, ch) in self.config.analogs.iter_mut().enumerate() {
            ch.index = i + 1;
        }
        self.config.channels.analog = self.config.analogs.len();
        self.config.channels.total = self.config.channels.analog + self.config.channels.status;
        Ok(())
    }

    /// 在某状态量通道下标之后插入一条新通道（含初始状态列，值须为 0/1）。
    ///
    /// 语义与 [`Comtrade::insert_analog_channel`] 一致。
    pub fn insert_status_channel(
        &mut self,
        after: usize,
        ch: crate::cfg::StatusChannel,
        values: Vec<u8>,
    ) -> Result<()> {
        let data = self
            .data
            .as_mut()
            .ok_or_else(|| Error::parse(FileRole::Dat, 0, "文件不含采样数据"))?;
        if after >= self.config.statuses.len() {
            return Err(Error::parse(
                FileRole::Cfg,
                0,
                format!("状态量插入位置越界: after={after}"),
            ));
        }
        if values.len() != data.len() {
            return Err(Error::parse(
                FileRole::Dat,
                0,
                format!("新列长度 {} 与采样点数 {} 不一致", values.len(), data.len()),
            ));
        }
        if values.iter().any(|&v| v > 1) {
            return Err(Error::parse(
                FileRole::Dat,
                0,
                "状态量取值只能为 0 或 1",
            ));
        }
        let insert_at = after + 1;
        self.config.statuses.insert(insert_at, ch);
        data.statuses.insert(insert_at, values);
        for (i, ch) in self.config.statuses.iter_mut().enumerate() {
            ch.index = i + 1;
        }
        self.config.channels.status = self.config.statuses.len();
        self.config.channels.total = self.config.channels.analog + self.config.channels.status;
        Ok(())
    }

    /// 裁剪采样行，仅保留区间 `[start, end)`，同步截断全部数据列并修正 CFG 采样段。
    ///
    /// - 保留 `sample_index` / `timestamp_us` / 各模拟列 / 各状态列的 `[start, end)`；
    /// - `sample_index` 平移为从 1 重新编号（相对新起点），`timestamp_us` 保持绝对微秒；
    /// - 用 `recalculate_segments` 按剩余时间戳重算 `config.sampling.segments`
    ///   （`end_point` 随新采样点数更新）。
    pub fn crop_rows(&mut self, start: usize, end: usize) -> Result<()> {
        let data = self
            .data
            .as_mut()
            .ok_or_else(|| Error::parse(FileRole::Dat, 0, "文件不含采样数据"))?;
        let n = data.len();
        if start >= end || end > n {
            return Err(Error::parse(
                FileRole::Dat,
                0,
                format!("裁剪区间非法: [{start}, {end}) 超出 0..{n}"),
            ));
        }
        let keep = end - start;
        data.sample_index.drain(..start);
        data.sample_index.truncate(keep);
        data.timestamp_us.drain(..start);
        data.timestamp_us.truncate(keep);
        for col in &mut data.analogs {
            col.drain(..start);
            col.truncate(keep);
        }
        for col in &mut data.statuses {
            col.drain(..start);
            col.truncate(keep);
        }
        // 重新编号采样点号（1 基，相对新起点）
        for (i, v) in data.sample_index.iter_mut().enumerate() {
            *v = (i + 1) as i32;
        }
        // 按剩余时间戳重算采样段；不足两点时清空段（无法派生采样率）
        let nominal = self.config.sampling.freq;
        self.config.sampling.segments =
            recalculate_segments(&data.timestamp_us, nominal);
        Ok(())
    }

    /// 编码为 CFF 单文件字节流（CFG+INF+DAT，丢弃 HDR/DMF，与 `CffFile::encode` 一致）。
    pub fn to_cff_bytes(&self, dt: DataType) -> Result<Vec<u8>> {
        let data = self
            .data
            .as_ref()
            .ok_or_else(|| Error::parse(FileRole::Dat, 0, "文件不含采样数据"))?;
        Ok(CffFile::encode(&self.config, data, self.inf.as_ref(), dt))
    }

    /// 写出为 CFF 单文件。
    pub fn write_cff(&self, path: &Path, dt: DataType) -> Result<()> {
        let bytes = self.to_cff_bytes(dt)?;
        std::fs::write(path, bytes).map_err(|e| Error::Io {
            path: path.to_path_buf(),
            source: e,
        })
    }
}

/// 解析文件路径（存在则返回）
fn resolve_path(parent: &Path, stem: &str, ext: &str) -> Option<PathBuf> {
    // Windows 通常不区分大小写，但 Linux/WSL 区分。录波文件组的
    // 后缀约定本身大小写不敏感，因此在大小写敏感文件系统上也要
    // 找到 `FILE.CFG` / `file.cfg` 的兄弟文件。只比较扩展名，主
    // 文件名仍保持用户选择的 stem，避免误匹配相似文件。
    let entries = std::fs::read_dir(parent).ok();
    if let Some(entries) = entries {
        let mut candidates = entries
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .filter(|candidate| {
                candidate.file_stem().and_then(|value| value.to_str()) == Some(stem)
                    && candidate
                        .extension()
                        .and_then(|value| value.to_str())
                        .is_some_and(|value| value.eq_ignore_ascii_case(ext))
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|a, b| a.to_string_lossy().cmp(&b.to_string_lossy()));
        if let Some(path) = candidates.into_iter().next() {
            return Some(path);
        }
    }
    let path = parent.join(format!("{}.{}", stem, ext));
    path.exists().then_some(path)
}

#[cfg(test)]
mod tests {
    use super::resolve_path;

    #[test]
    fn resolve_path_accepts_case_insensitive_extension() {
        let dir = std::env::temp_dir().join(format!(
            "comtrade-resolve-case-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let cfg = dir.join("capture.CfG");
        std::fs::write(&cfg, b"test").unwrap();
        assert_eq!(resolve_path(&dir, "capture", "cfg"), Some(cfg));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
