//! 母线设备模型

use super::AcvChn;

/// 母线
#[derive(Debug, Clone, Default)]
pub struct Bus {
    /// 设备序号（INF `DEV_ID=idx,name` / DMF `idx`）
    pub idx: usize,
    /// 设备名称（DMF `bus_name` / INF `DEV_ID`）
    pub name: String,
    /// 源引用（DMF `srcRef`，IEC 61850 参引）
    pub src_ref: String,
    /// INF `SYS_ID`：保护信息系统分配的系统内编号
    pub sys_id: String,
    /// 一次额定电压（DMF `VRtg`，kV）
    pub v_rtg: f64,
    /// 二次额定电压（DMF `VRtgSnd`，V）
    pub v_rtg_snd: f64,
    /// TV 安装位置（DMF `VRtgSnd_Pos` / INF `TV_POS`）：`BUS` = 母线侧，`LINE` = 线路侧。
    /// 存原始拼写以保证 round-trip 保真。
    pub tv_pos: String,
    /// DMF `is_location`
    pub is_location: String,
    /// 母线 UUID（DMF `bus_uuid`）
    pub bus_uuid: String,
    /// 电压通道组
    pub acv: AcvChn,
    /// 关联模拟量通道（DMF `AnaChn`）
    pub ana_chns: Vec<usize>,
    /// 关联开关量通道（DMF `StaChn` / INF `STATUS_CHNS`）
    pub sta_chns: Vec<usize>,
}
