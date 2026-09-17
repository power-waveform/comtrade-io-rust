//! 发电机 / 励磁机元素属性解析（§1.6 / §1.7）

use cbase::equipment::{
    AccBran, AcvChn, BranchNum, Exciter, Generator, SyncReactance, Ufe, UfeChns, UnChns,
};

use super::common::{attr, attr_f64, attr_str, attr_usize};

/// 解析发电机属性（§1.6）。DMF 对发电机/励磁机的元素与属性命名规范未定义，
/// 此处按与现有 Bus/Line 风格一致的属性名读取。
pub fn parse_generator_attrs(attrs: &[(&str, &str)]) -> Generator {
    Generator {
        idx: attr_usize(attrs, "idx"),
        name: attr_str(attrs, "gen_name"),
        src_ref: attr_str(attrs, "srcRef"),
        sys_id: attr_str(attrs, "sys_ID"),
        trm_id: attr_str(attrs, "trm_ID"),
        object_type: attr_str(attrs, "object_type"),
        freq: attr_f64(attrs, "freq", 0.0),
        capacity: attr_f64(attrs, "capacity", 0.0),
        factor: attr_f64(attrs, "factor", 0.0),
        v1: attr_f64(attrs, "V1", 0.0),
        branch_num: BranchNum {
            z1: attr_usize(attrs, "branch_z1"),
            z2: attr_usize(attrs, "branch_z2"),
            z3: attr_usize(attrs, "branch_z3"),
        },
        rotor_i: attr_f64(attrs, "rotor_I", 0.0),
        rotor_v2: attr_f64(attrs, "rotor_V2", 0.0),
        ufe: Ufe {
            rated: attr_f64(attrs, "ufe_rated", 0.0),
            no_load: attr_f64(attrs, "ufe_no_load", 0.0),
        },
        x: SyncReactance {
            xd: attr_f64(attrs, "xd", 0.0),
            xq: attr_f64(attrs, "xq", 0.0),
            xd_prime: attr_f64(attrs, "xd_prime", 0.0),
            xs: attr_f64(attrs, "xs", 0.0),
        },
        excitation_mode: attr(attrs, "excitation_mode")
            .and_then(|v| v.trim().parse::<i32>().ok())
            .unwrap_or(0),
        igt_dir: attr(attrs, "igt_dir")
            .and_then(|v| v.trim().parse::<i32>().ok())
            .unwrap_or(0),
        acv: AcvChn::default(),
        ta: AccBran::default(),
        ta_z1: AccBran::default(),
        ta_z2: AccBran::default(),
        ta_z3: AccBran::default(),
        ufe_chns: UfeChns {
            ufe: attr_usize(attrs, "ufe_chn"),
            pos: attr_usize(attrs, "pos_ufe_chn"),
            neg: attr_usize(attrs, "neg_ufe_chn"),
        },
        ife_chn: attr_usize(attrs, "ife_chn"),
        un_chns: UnChns {
            terminal: attr_usize(attrs, "un_terminal"),
            neutral: attr_usize(attrs, "un_neutral"),
            longitudinal: attr_usize(attrs, "un_longitudinal"),
        },
        ta_ido_chn: attr_usize(attrs, "ta_ido_chn"),
        oth_achns: Vec::new(),
        sta_chns: Vec::new(),
    }
}

/// 解析励磁机属性（§1.7）。
pub fn parse_exciter_attrs(attrs: &[(&str, &str)]) -> Exciter {
    Exciter {
        idx: attr_usize(attrs, "idx"),
        name: attr_str(attrs, "exc_name"),
        src_ref: attr_str(attrs, "srcRef"),
        sys_id: attr_str(attrs, "sys_ID"),
        pwr_id: attr_str(attrs, "pwr_ID"),
        object_type: attr_str(attrs, "object_type"),
        freq: attr_f64(attrs, "freq", 0.0),
        v1: attr_f64(attrs, "V1", 0.0),
        acv: AcvChn::default(),
        ta: AccBran::default(),
        ta_z: AccBran::default(),
        oth_achns: Vec::new(),
        sta_chns: Vec::new(),
    }
}
