//! 状态变位检测

use cbase::time::Timestamp;

use super::DatFile;

/// 状态变位记录
#[derive(Debug, Clone, Copy)]
pub struct StatusChange {
    /// 采样点号（1-based）
    pub sample_point: usize,
    /// 绝对时间戳
    pub timestamp: Option<Timestamp>,
    /// 状态值（0/1）
    pub state: u8,
}

impl DatFile {
    /// 变位检测：返回发生变位的状态通道索引及其变位记录
    pub fn changed_statuses(
        &self,
        start_time: Option<Timestamp>,
    ) -> Vec<(usize, Vec<StatusChange>)> {
        let mut results = Vec::new();
        for (ch_idx, status_col) in self.statuses.iter().enumerate() {
            if status_col.is_empty() {
                continue;
            }
            let initial = status_col[0];
            let mut records = vec![StatusChange {
                sample_point: 1,
                timestamp: start_time,
                state: initial,
            }];

            for i in 1..status_col.len() {
                if status_col[i] != status_col[i - 1] {
                    let ts = start_time.and_then(|st| {
                        let us = self.timestamp_us.get(i).copied().unwrap_or(0.0);
                        st.add_micros(us as i64)
                    });
                    records.push(StatusChange {
                        sample_point: i + 1,
                        timestamp: ts,
                        state: status_col[i],
                    });
                }
            }

            if records.len() > 1 {
                results.push((ch_idx + 1, records));
            }
        }
        results
    }
}
