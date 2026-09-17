//! 设备模型（INF/DMF 共享）
//!
//! Bus/Line/Transformer 设备拓扑模型，被 DMF 和 INF 模块复用。
//!
//! 字段命名以 DMF XML 属性与 INF 段 Key 为准，规范依据：
//! - DMF：`docs/rust重构需求分析报告.md` §2.6
//! - INF：`docs/COMTRADE1999补充定义(090917).pdf` §1.4（线路段）/ §1.5（变压器段）

pub mod bus;
pub mod generator;
pub mod line;
pub mod transformer;

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

pub use self::bus::Bus;
pub use self::generator::{BranchNum, Exciter, Generator, SyncReactance, Ufe, UfeChns, UnChns};
pub use self::line::{Capacitance, Igap, Impedance, Line, MutualInductance, Px};
pub use self::transformer::{Transformer, TransformerWinding, WindingLocation};
