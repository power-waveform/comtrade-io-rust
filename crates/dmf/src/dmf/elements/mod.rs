//! DMF 元素属性解析（每类设备一个模块）

pub mod bus;
pub mod common;
pub mod generator;
pub mod line;
pub mod transformer;

pub use self::bus::parse_bus_attrs;
pub use self::common::{
    attr, attr_f64, attr_str, attr_usize, is_ac_channel_type, parse_acc_attrs, parse_acv_attrs,
    parse_analog_attrs, parse_dir, parse_status_attrs,
};
pub use self::generator::{parse_exciter_attrs, parse_generator_attrs};
pub use self::line::parse_line_attrs;
pub use self::transformer::{parse_transformer_attrs, parse_winding_attrs};
