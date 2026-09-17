//! COMTRADE 文件组写出。

use std::path::Path;

use cbase::error::{Error, FileRole, Result};
use cff::CffFile;
use cfg::DataType;

use super::Comtrade;

impl Comtrade {
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
