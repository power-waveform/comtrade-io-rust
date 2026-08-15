//! 设备模型（INF/DMF 共享）
//!
//! Bus/Line/Transformer 设备拓扑模型，被 DMF 和 INF 模块复用。
//!
//! 字段命名以 DMF XML 属性与 INF 段 Key 为准，规范依据：
//! - DMF：`docs/rust重构需求分析报告.md` §2.6
//! - INF：`docs/COMTRADE1999补充定义(090917).pdf` §1.4（线路段）/ §1.5（变压器段）

/// 设备组
#[derive(Debug, Clone, Default)]
pub struct EquipmentGroup {
    /// 母线列表
    pub buses: Vec<Bus>,
    /// 线路列表
    pub lines: Vec<Line>,
    /// 变压器列表
    pub transformers: Vec<Transformer>,
    /// 发电机（规范 §1.6 `[ZYHD POWER_#n]`）
    pub generators: Vec<Generator>,
    /// 励磁机（规范 §1.7 `[ZYHD EXCITATION_#n]`）
    pub exciters: Vec<Exciter>,
}

/// 交流电压通道组（DMF `ACVChn` / INF `TV_CHNS`）
///
/// 通道号为 1 基（对应 CFG 通道编号），`0` 表示未配置。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AcvChn {
    /// A 相电压通道号（对应 CFG 模拟量通道编号）
    pub ua_idx: usize,
    /// B 相电压通道号
    pub ub_idx: usize,
    /// C 相电压通道号
    pub uc_idx: usize,
    /// N 相（零序）电压通道号
    pub un_idx: usize,
    /// L 相（线电压）通道号
    pub ul_idx: usize,
}

impl AcvChn {
    /// 是否全部通道均未配置
    pub fn is_empty(&self) -> bool {
        self.ua_idx == 0
            && self.ub_idx == 0
            && self.uc_idx == 0
            && self.un_idx == 0
            && self.ul_idx == 0
    }
}

/// 交流电流分支（DMF `ACC_Bran` / INF `TA_CHNS` `TA_Id_#N`）
///
/// `dir` 语义按规范 §1.4：`1` = 正方向（流向线路），`-1` = 反方向，缺省 `1`。
/// 变压器侧（§1.5）该值为极性符号：`1` = 电流流入变压器，`-1` = 流出。
///
/// `dir_raw` 保留 DMF 原始拼写（样本中 `POS` / `pos` 大小写不一致），
/// 以便 round-trip 时按原文写回；`dir` 是归一化后的语义值。
#[derive(Debug, Clone, PartialEq)]
pub struct AccBran {
    /// 分支序号（DMF `bran_idx`，从 1 起）
    pub bran_idx: usize,
    /// A 相电流通道号
    pub ia_idx: usize,
    /// B 相电流通道号
    pub ib_idx: usize,
    /// C 相电流通道号
    pub ic_idx: usize,
    /// N 相（零序）电流通道号
    pub in_idx: usize,
    /// 电流方向语义值（`1` = 正方向，`-1` = 反方向）
    pub dir: i32,
    /// 方向原始拼写（DMF `dir` 属性原文，如 `POS`/`pos`）
    pub dir_raw: String,
}

impl Default for AccBran {
    fn default() -> Self {
        AccBran {
            bran_idx: 0,
            ia_idx: 0,
            ib_idx: 0,
            ic_idx: 0,
            in_idx: 0,
            dir: 1,
            dir_raw: String::new(),
        }
    }
}

impl AccBran {
    /// 是否全部通道均未配置
    pub fn is_empty(&self) -> bool {
        self.ia_idx == 0 && self.ib_idx == 0 && self.ic_idx == 0 && self.in_idx == 0
    }
}

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

/// 变压器
#[derive(Debug, Clone, Default)]
pub struct Transformer {
    /// 设备序号（INF `DEV_ID` / DMF `idx`）
    pub idx: usize,
    /// 设备名称（DMF `trm_name` / INF `DEV_ID`）
    pub name: String,
    /// 源引用（DMF `srcRef`）
    pub src_ref: String,
    /// INF `SYS_ID`：系统内编号
    pub sys_id: String,
    /// 变压器类型（INF `OBJECT_TYPE`）：`MAIN` = 主变，`PLANT` = 厂用变，`EXCITATION` = 励磁变
    pub object_type: String,
    /// 额定容量（DMF `pwrRtg` / INF `CAPACITY`，MVA）
    pub pwr_rtg: f64,
    /// 绕组数（INF `WINDING_NUM`）：`1` = 自耦变，`2` = 两卷变，`3` = 三卷变
    pub winding_num: usize,
    /// INF `TA_SELF_COMP`：TA 相位是否自补偿（`YES` / `NO`）
    pub ta_self_comp: String,
    /// 变压器 UUID（DMF `transformer_uuid`）
    pub transformer_uuid: String,
    /// 各绕组（高压/中压/低压）
    pub windings: Vec<TransformerWinding>,
    /// 其他相关模拟量通道
    pub oth_achns: Vec<usize>,
    /// 关联模拟量通道
    pub ana_chns: Vec<usize>,
    /// 关联开关量通道
    pub sta_chns: Vec<usize>,
}

/// 变压器绕组位置
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindingLocation {
    /// 高压侧
    High,
    /// 中压侧
    Medium,
    /// 低压侧
    Low,
}

impl WindingLocation {
    /// INF 段 Key 前缀（`H_` / `M_` / `L_`）
    pub fn inf_prefix(&self) -> &'static str {
        match self {
            WindingLocation::High => "H",
            WindingLocation::Medium => "M",
            WindingLocation::Low => "L",
        }
    }

    /// 该侧的 `TA_Id_#N` 分支序号。
    ///
    /// 依据规范 §1.5：`#1/#2` = 高压侧一/二分支，`#3/#4` = 中压侧一/二分支，
    /// `#5/#6/#7` = 低压侧一/二/三分支。
    pub fn ta_id_indices(&self) -> &'static [usize] {
        match self {
            WindingLocation::High => &[1, 2],
            WindingLocation::Medium => &[3, 4],
            WindingLocation::Low => &[5, 6, 7],
        }
    }

    /// DMF `location` 属性的规范拼写
    pub fn as_str(&self) -> &'static str {
        match self {
            WindingLocation::High => "High",
            WindingLocation::Medium => "Medium",
            WindingLocation::Low => "Low",
        }
    }

    /// 从 DMF `location` 属性解析（大小写不敏感）
    pub fn parse(s: &str) -> Option<WindingLocation> {
        match s.trim().to_ascii_lowercase().as_str() {
            "high" | "h" => Some(WindingLocation::High),
            "medium" | "middle" | "m" => Some(WindingLocation::Medium),
            "low" | "l" => Some(WindingLocation::Low),
            _ => None,
        }
    }
}

/// 变压器绕组
#[derive(Debug, Clone, Default)]
pub struct TransformerWinding {
    /// 绕组位置原始拼写（DMF `location`，样本中 `High` / `high` 不一致，存原文保 round-trip）
    pub location: String,
    /// 源引用（DMF `srcRef`）
    pub src_ref: String,
    /// 一次额定电压（kV）
    pub v_rtg: f64,
    /// 一次额定电流（A）
    pub a_rtg: f64,
    /// 电流分支数（DMF `bran_num` / INF `{H,M,L}_PARAM` 第三字段）
    pub bran_num: usize,
    /// 所连母线 ID（DMF `bus_ID`）
    pub bus_id: String,
    /// 绕组接线组别（DMF `wG` / INF `{H,M,L}_PARAM` 第一字段），如 `y0` / `yn0` / `Y12` / `D11`。
    ///
    /// 注意：此字段是**字符串代号**，不是数值。
    pub wg: String,
    /// 电压通道组（DMF `ACVChn` / INF `{H,M,L}_TV_CHNS`）
    pub acv: AcvChn,
    /// 电流分支（DMF `ACC_Bran` / INF `TA_Id_#N`）
    pub currents: Vec<AccBran>,
    /// 间隙零序电流通道（DMF `Igap`）
    pub igap: Igap,
    /// 零序 TA 通道（INF `{H,M,L}_TA_ZS`）
    pub ta_zs: usize,
    /// 间隙零序 TA 通道（INF `{H,M,L}_TA_ZS_GAP`）
    pub ta_zs_gap: usize,
    /// 该侧关联开关量通道（INF `{H,M,L}_STATUS_CHNS`）
    pub sta_chns: Vec<usize>,
}

impl TransformerWinding {
    /// 解析 `location` 为枚举
    pub fn location_kind(&self) -> Option<WindingLocation> {
        WindingLocation::parse(&self.location)
    }
}

// ========== 发电机 / 励磁机（规范 §1.6 / §1.7） ==========

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
