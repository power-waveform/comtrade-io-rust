//! 发电机 / 励磁机设备模型（规范 §1.6 / §1.7）

use super::{AccBran, AcvChn};

/// 发电机中性点分支数（§1.6 `BRANCH_NUM=z1, z2, z3`）
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BranchNum {
    /// 中性点第一分支组分支数
    pub z1: usize,
    /// 中性点第二分支组分支数
    pub z2: usize,
    /// 中性点第三分支组分支数
    pub z3: usize,
}

/// 发电机励磁电压（§1.6 `Ufe=额定励磁电压, 额定空载励磁电压`，单位 V）
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Ufe {
    /// 额定励磁电压
    pub rated: f64,
    /// 额定空载励磁电压
    pub no_load: f64,
}

/// 发电机同步电抗（§1.6 `X=Xd, Xq, Xd', Xs`，标么值）
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SyncReactance {
    /// 纵轴同步电抗
    pub xd: f64,
    /// 交轴同步电抗
    pub xq: f64,
    /// 暂态纵轴同步电抗
    pub xd_prime: f64,
    /// 系统联系电抗
    pub xs: f64,
}

/// 励磁电压通道（§1.6 `Ufe_CHNS=Ufe, +Ufe, -Ufe`）
#[derive(Debug, Clone, Default, PartialEq)]
pub struct UfeChns {
    /// 励磁电压通道
    pub ufe: usize,
    /// 正对地励磁电压通道
    pub pos: usize,
    /// 负对地励磁电压通道
    pub neg: usize,
}

/// 机端零序电压通道（§1.6/§1.7 `UN_CHNS=机端零序, 中性点零序, 纵向零序`）
#[derive(Debug, Clone, Default, PartialEq)]
pub struct UnChns {
    /// 机端零序电压通道
    pub terminal: usize,
    /// 中性点零序电压通道
    pub neutral: usize,
    /// 纵向零序电压通道
    pub longitudinal: usize,
}

/// 发电机（规范 §1.6 `[ZYHD POWER_#n]`）
#[derive(Debug, Clone, Default)]
pub struct Generator {
    /// 设备序号（INF `DEV_ID`）
    pub idx: usize,
    /// 设备名称（INF `DEV_ID`）
    pub name: String,
    /// 源引用（DMF `srcRef`）
    pub src_ref: String,
    /// INF `SYS_ID`
    pub sys_id: String,
    /// INF `TRM_ID=idx,name`，相关主变。原文存储以保 round-trip。
    pub trm_id: String,
    /// 发电机类型（INF `OBJECT_TYPE`）：`STEAM_TURBINE` = 气轮机，`WATER_TURBINE` = 水轮机
    pub object_type: String,
    /// 额定频率（Hz）
    pub freq: f64,
    /// 额定容量（MW）。注意与变压器 `CAPACITY`（MVA）单位不同。
    pub capacity: f64,
    /// 功率因数
    pub factor: f64,
    /// 一次额定电压（kV）
    pub v1: f64,
    /// 中性点分支数（3 元组）
    pub branch_num: BranchNum,
    /// 转子电流额定值（A）
    pub rotor_i: f64,
    /// 转子分流器二次额定值（mV）
    pub rotor_v2: f64,
    /// 励磁电压（2 元组，V）
    pub ufe: Ufe,
    /// 同步电抗（4 元组，标么值）
    pub x: SyncReactance,
    /// 励磁方式（`0`=励磁变，`1`=励磁机，`2`=励磁变+励磁机）
    pub excitation_mode: i32,
    /// 机端电流方向（`0`=流出发电机，`1`=流入发电机）
    pub igt_dir: i32,
    /// 机端电压 TV 通道（`TV_CHNS=Ua, Ub, Uc`，**仅 3 字段**，`un_idx` 恒 0）
    pub acv: AcvChn,
    /// 机端电流 TA 通道（`TA_CHNS=Ia, Ib, Ic, 极性`）
    pub ta: AccBran,
    /// 中性点第一分支组 TA（`TA_Z1_CHNS`）
    pub ta_z1: AccBran,
    /// 中性点第二分支组 TA（`TA_Z2_CHNS`）
    pub ta_z2: AccBran,
    /// 中性点第三分支组 TA（`TA_Z3_CHNS`）
    pub ta_z3: AccBran,
    /// 励磁电压通道（3 元组）
    pub ufe_chns: UfeChns,
    /// 励磁电流通道（`Ife_CHN=Ife`）
    pub ife_chn: usize,
    /// 机端零序电压通道（3 元组）
    pub un_chns: UnChns,
    /// 零序横差 TA 通道（`TA_Ido_CHN=Ido`）
    pub ta_ido_chn: usize,
    /// 其他相关模拟量通道（`OTH_ACHNS`）
    pub oth_achns: Vec<usize>,
    /// 开关量通道（`STATUS_CHNS`）
    pub sta_chns: Vec<usize>,
}

/// 励磁机（规范 §1.7 `[ZYHD EXCITATION_#n]`）
#[derive(Debug, Clone, Default)]
pub struct Exciter {
    /// 设备序号（INF `DEV_ID`）
    pub idx: usize,
    /// 设备名称（INF `DEV_ID`）
    pub name: String,
    /// 源引用（DMF `srcRef`）
    pub src_ref: String,
    /// INF `SYS_ID`
    pub sys_id: String,
    /// INF `PWR_ID=idx,name`，相关发电机。原文存储以保 round-trip。
    pub pwr_id: String,
    /// 励磁机类型（INF `OBJECT_TYPE`）：`PRIMARY` = 主励磁机，`SLAVE` = 副励磁机
    pub object_type: String,
    /// 额定频率（Hz）
    pub freq: f64,
    /// 一次额定电压（kV）
    pub v1: f64,
    /// 机端电压 TV 通道（`TV_CHNS=Ua, Ub, Uc, Un`，**4 字段**）
    pub acv: AcvChn,
    /// 机端 TA 通道（`TA_CHNS=Ia, Ib, Ic, 极性`）
    pub ta: AccBran,
    /// 中性点 TA 通道（`TA_Z_CHNS=Ia, Ib, Ic, 极性`）
    pub ta_z: AccBran,
    /// 其他相关模拟量通道（`OTH_ACHNS`）
    pub oth_achns: Vec<usize>,
    /// 开关量通道（`STATUS_CHNS`）
    pub sta_chns: Vec<usize>,
}
