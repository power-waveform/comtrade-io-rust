//! 母线元素属性解析

use cbase::equipment::{AcvChn, Bus};

use super::common::{attr_f64, attr_str, attr_usize};

pub fn parse_bus_attrs(attrs: &[(&str, &str)]) -> Bus {
    Bus {
        idx: attr_usize(attrs, "idx"),
        name: attr_str(attrs, "bus_name"),
        src_ref: attr_str(attrs, "srcRef"),
        sys_id: String::new(),
        v_rtg: attr_f64(attrs, "VRtg", 0.0),
        v_rtg_snd: attr_f64(attrs, "VRtgSnd", 0.0),
        tv_pos: attr_str(attrs, "VRtgSnd_Pos"),
        is_location: attr_str(attrs, "is_location"),
        bus_uuid: attr_str(attrs, "bus_uuid"),
        acv: AcvChn::default(),
        ana_chns: Vec::new(),
        sta_chns: Vec::new(),
    }
}
