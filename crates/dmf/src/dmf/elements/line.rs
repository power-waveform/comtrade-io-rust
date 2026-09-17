//! 线路元素属性解析

use cbase::equipment::{AcvChn, Capacitance, Impedance, Line, MutualInductance, Px};

use super::common::{attr_f64, attr_str, attr_usize};

pub fn parse_line_attrs(attrs: &[(&str, &str)]) -> Line {
    Line {
        idx: attr_usize(attrs, "idx"),
        name: attr_str(attrs, "line_name"),
        bus_id: attr_str(attrs, "bus_ID"),
        src_ref: attr_str(attrs, "srcRef"),
        sys_id: String::new(),
        object_type: String::new(),
        v_rtg: attr_f64(attrs, "VRtg", 0.0),
        a_rtg: attr_f64(attrs, "ARtg", 0.0),
        a_rtg_snd: attr_f64(attrs, "ARtgSnd", 0.0),
        line_len: attr_f64(attrs, "LinLen", 0.0),
        bran_num: attr_usize(attrs, "bran_num"),
        line_uuid: attr_str(attrs, "line_uuid"),
        remote_id: attr_str(attrs, "remote_ID"),
        remote_flag: attr_str(attrs, "remote_Flag"),
        differential_id: attr_str(attrs, "differential_ID"),
        other_id: String::new(),
        reactor: None,
        impedance: Impedance::default(),
        capacitance: Capacitance::default(),
        mutual_inductance: MutualInductance::default(),
        px: Px::default(),
        currents: Vec::new(),
        voltages: Vec::new(),
        differential_current: AcvChn::default(),
        oth_achns: Vec::new(),
        ana_chns: Vec::new(),
        sta_chns: Vec::new(),
    }
}
