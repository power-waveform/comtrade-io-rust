//! 线路设备模型

use super::{AccBran, AcvChn};

/// 线路阻抗（DMF `RX` / INF `RX=R1,X1,R0,X0`，单位 Ω/km）
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Impedance {
    /// 正序电阻（Ω/km）
    pub r1: f64,
    /// 正序电抗（Ω/km）
    pub x1: f64,
    /// 零序电阻（Ω/km）
    pub r0: f64,
    /// 零序电抗（Ω/km）
    pub x0: f64,
}

/// 线路分布电容与电导
///
/// 字段序按规范 §1.4：`CG=C1,G1,C0,G0`（正序容 / 正序导 / 零序容 / 零序导），
/// C 单位 μF/km，G 单位 S/km。
/// 注意：DMF 的 `CG` 元素属性名为 `c1/c0/g1/g0`，与 INF 的位置序不同，按属性名读取。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Capacitance {
    /// 正序电容（μF/km）
    pub c1: f64,
    /// 正序电导（S/km）
    pub g1: f64,
    /// 零序电容（μF/km）
    pub c0: f64,
    /// 零序电导（S/km）
    pub g0: f64,
}

/// 线路零序互感（DMF `MR` / INF `MRX=MR,MX`，单位 Ω/km）
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MutualInductance {
    /// 互感支路序号
    pub idx: usize,
    /// 零序互感电阻（Ω/km）
    pub mr0: f64,
    /// 零序互感电抗（Ω/km）
    pub mx0: f64,
}

/// 线路正序/零序 PX 参数（DMF `PX`）
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Px {
    /// 正序 PX 参数
    pub px: f64,
    /// 零序 PX 参数
    pub px0: f64,
}

/// 间隙零序电流通道（DMF `Igap`）
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Igap {
    /// 间隙零序电流通道号（DMF `zgap_idx`）
    pub zgap_idx: usize,
    /// 间隙零序电流备用通道号（DMF `zsgap_idx`）
    pub zsgap_idx: usize,
}

/// 线路
#[derive(Debug, Clone, Default)]
pub struct Line {
    /// 设备序号（INF `DEV_ID` / DMF `idx`）
    pub idx: usize,
    /// 设备名称（DMF `line_name` / INF `DEV_ID`）
    pub name: String,
    /// 所连母线 ID（DMF `bus_ID`）
    pub bus_id: String,
    /// 源引用（DMF `srcRef`）
    pub src_ref: String,
    /// INF `SYS_ID`：系统内编号
    pub sys_id: String,
    /// 线路类型（INF `OBJECT_TYPE`）：`LINE` / `BYPASS` / `BUS_CONNECTION` / `RAILWAY`
    pub object_type: String,
    /// 一次额定电压（kV）
    pub v_rtg: f64,
    /// 一次额定电流（A）
    pub a_rtg: f64,
    /// 二次额定电流（DMF `ARtgSnd`）
    pub a_rtg_snd: f64,
    /// 线路长度（km）
    pub line_len: f64,
    /// 电流分支数
    pub bran_num: usize,
    /// 线路 UUID（DMF `line_uuid`）
    pub line_uuid: String,
    /// DMF `remote_ID`：对端（远侧）ID
    pub remote_id: String,
    /// DMF `remote_Flag`：对端标志
    pub remote_flag: String,
    /// DMF `differential_ID`：差动 ID
    pub differential_id: String,
    /// INF `OTHER_ID`：双回线之另一回线 ID
    pub other_id: String,
    /// 并联电抗器补偿电抗（Ω）。INF `REACTOR=NO` 表示无并联电抗器 → `None`
    pub reactor: Option<f64>,
    /// 线路阻抗（RX）
    pub impedance: Impedance,
    /// 线路分布电容与电导（CG）
    pub capacitance: Capacitance,
    /// 线路零序互感（MR）
    pub mutual_inductance: MutualInductance,
    /// 线路正序/零序 PX 参数
    pub px: Px,
    /// 电流分支（DMF `ACC_Bran` / INF `TA_CHNS`、`TA_CHNS_#2`）
    pub currents: Vec<AccBran>,
    /// 电压通道组（DMF `ACVChn` / INF `TV_CHNS`、`TV_CHNS_#n`）
    pub voltages: Vec<AcvChn>,
    /// 差动电流通道（DMF `DifferentialCurrent`）
    pub differential_current: AcvChn,
    /// INF `OTH_ACHNS`：其他相关模拟量通道
    pub oth_achns: Vec<usize>,
    /// 关联模拟量通道
    pub ana_chns: Vec<usize>,
    /// 关联开关量通道
    pub sta_chns: Vec<usize>,
}
