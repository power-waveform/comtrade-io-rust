//! COMTRADE 波形编辑：`DataEdit` trait。
//!
//! Rust 孤儿规则不允许在外部 crate 中为 `Comtrade` 增加固有方法，
//! 因此编辑能力以 trait 形式提供（由 `model::Comtrade` 实现）。

use cbase::error::{Error, FileRole, Result};
use cfg::{AnalogChannel, StatusChannel};
use dat::recalculate_segments;
use model::{ChannelKind, Comtrade};

/// 数据编辑接口（通道列替换/增删、裁剪采样行）。
///
/// 编辑边界：只改动 `Comtrade.data` 与 `Comtrade.config`，不重建 `equipment` 拓扑。
pub trait DataEdit {
    /// 替换某模拟量通道整列（工程值）。
    ///
    /// `channel` 为 0 基列下标（与 `dat::DatFile::analog_samples` 一致）。
    /// `values` 长度必须与现有采样点数一致，否则返回错误、不修改任何数据。
    fn set_analog_column(&mut self, channel: usize, values: Vec<f64>) -> Result<()>;

    /// 替换某状态量通道整列（0/1）。
    ///
    /// `channel` 为 0 基列下标；`values` 长度须与采样点数一致，且每个值必须为 0 或 1。
    fn set_status_column(&mut self, channel: usize, values: Vec<u8>) -> Result<()>;

    /// 删除一条通道（模拟量或状态量），原子同步 CFG 定义、通道计数与数据列。
    ///
    /// `index` 为 0 基列下标；删除后同种类其余通道的 CFG 1 基 `index` 重新编号，
    /// `config.channels.total/analog/status` 与 `data` 列数保持一致。
    fn remove_channel(&mut self, kind: ChannelKind, index: usize) -> Result<()>;

    /// 在某模拟量通道下标之后插入一条新通道（含初始工程值列）。
    ///
    /// `after` 为 0 基下标，插入到 `after + 1` 处；`values` 长度须与采样点数一致。
    /// 插入后全部模拟量 CFG 1 基 `index` 重新编号，计数同步更新。
    fn insert_analog_channel(
        &mut self,
        after: usize,
        ch: AnalogChannel,
        values: Vec<f64>,
    ) -> Result<()>;

    /// 在某状态量通道下标之后插入一条新通道（含初始状态列，值须为 0/1）。
    ///
    /// 语义与 [`Comtrade::insert_analog_channel`] 一致。
    fn insert_status_channel(
        &mut self,
        after: usize,
        ch: StatusChannel,
        values: Vec<u8>,
    ) -> Result<()>;

    /// 裁剪采样行，仅保留区间 `[start, end)`，同步截断全部数据列并修正 CFG 采样段。
    ///
    /// - 保留 `sample_index` / `timestamp_us` / 各模拟列 / 各状态列的 `[start, end)`；
    /// - `sample_index` 平移为从 1 重新编号（相对新起点），`timestamp_us` 保持绝对微秒；
    /// - 用 `recalculate_segments` 按剩余时间戳重算 `config.sampling.segments`
    ///   （`end_point` 随新采样点数更新）。
    fn crop_rows(&mut self, start: usize, end: usize) -> Result<()>;
}

impl DataEdit for Comtrade {
    fn set_analog_column(&mut self, channel: usize, values: Vec<f64>) -> Result<()> {
        let data = self
            .data
            .as_mut()
            .ok_or_else(|| Error::parse(FileRole::Dat, 0, "文件不含采样数据"))?;
        let column = data.analogs.get_mut(channel).ok_or_else(|| {
            Error::parse(FileRole::Dat, 0, format!("模拟量通道下标越界: {channel}"))
        })?;
        if values.len() != column.len() {
            return Err(Error::parse(
                FileRole::Dat,
                0,
                format!(
                    "新列长度 {} 与采样点数 {} 不一致",
                    values.len(),
                    column.len()
                ),
            ));
        }
        *column = values;
        Ok(())
    }

    fn set_status_column(&mut self, channel: usize, values: Vec<u8>) -> Result<()> {
        let data = self
            .data
            .as_mut()
            .ok_or_else(|| Error::parse(FileRole::Dat, 0, "文件不含采样数据"))?;
        let column = data.statuses.get_mut(channel).ok_or_else(|| {
            Error::parse(FileRole::Dat, 0, format!("状态量通道下标越界: {channel}"))
        })?;
        if values.len() != column.len() {
            return Err(Error::parse(
                FileRole::Dat,
                0,
                format!(
                    "新列长度 {} 与采样点数 {} 不一致",
                    values.len(),
                    column.len()
                ),
            ));
        }
        if values.iter().any(|&v| v > 1) {
            return Err(Error::parse(FileRole::Dat, 0, "状态量取值只能为 0 或 1"));
        }
        *column = values;
        Ok(())
    }

    fn remove_channel(&mut self, kind: ChannelKind, index: usize) -> Result<()> {
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

    fn insert_analog_channel(
        &mut self,
        after: usize,
        ch: AnalogChannel,
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

    fn insert_status_channel(
        &mut self,
        after: usize,
        ch: StatusChannel,
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
            return Err(Error::parse(FileRole::Dat, 0, "状态量取值只能为 0 或 1"));
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

    fn crop_rows(&mut self, start: usize, end: usize) -> Result<()> {
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
        self.config.sampling.segments = recalculate_segments(&data.timestamp_us, nominal);
        Ok(())
    }
}
