//! 变压器与绕组元素属性解析

use cbase::equipment::{AcvChn, Igap, Transformer, TransformerWinding};

use super::common::{attr_f64, attr_str, attr_usize};

pub fn parse_transformer_attrs(attrs: &[(&str, &str)]) -> Transformer {
    Transformer {
        idx: attr_usize(attrs, "idx"),
        name: attr_str(attrs, "trm_name"),
        src_ref: attr_str(attrs, "srcRef"),
        sys_id: String::new(),
        object_type: String::new(),
        pwr_rtg: attr_f64(attrs, "pwrRtg", 0.0),
        winding_num: 0,
        ta_self_comp: String::new(),
        transformer_uuid: attr_str(attrs, "transformer_uuid"),
        windings: Vec::new(),
        oth_achns: Vec::new(),
        ana_chns: Vec::new(),
        sta_chns: Vec::new(),
    }
}

pub fn parse_winding_attrs(attrs: &[(&str, &str)]) -> TransformerWinding {
    TransformerWinding {
        location: attr_str(attrs, "location"),
        src_ref: attr_str(attrs, "srcRef"),
        v_rtg: attr_f64(attrs, "VRtg", 0.0),
        a_rtg: attr_f64(attrs, "ARtg", 0.0),
        bran_num: attr_usize(attrs, "bran_num"),
        bus_id: attr_str(attrs, "bus_ID"),
        // 偏离 Python 基线：wG 是绕组接线组别代号（如 y0 / yn0），
        // 原 Rust 实现声明为 f64 并 parse().unwrap_or(0.0)，使全部组别静默归零。
        wg: attr_str(attrs, "wG"),
        acv: AcvChn::default(),
        currents: Vec::new(),
        igap: Igap::default(),
        ta_zs: 0,
        ta_zs_gap: 0,
        sta_chns: Vec::new(),
    }
}
