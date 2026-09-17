//! 变压器设备模型

use super::line::Igap;
use super::{AccBran, AcvChn};

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
