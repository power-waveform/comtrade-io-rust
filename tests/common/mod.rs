//! 集成测试共享辅助：测试数据路径（`tests/data/`）。
//!
//! 各测试文件以 `mod common;` 引入，合并原先各自复制的 `data_path`。
//! 注意：`tests/common/` 子目录不会被 cargo 当作独立测试目标（仅顶层
//! `tests/*.rs` 会被当作集成测试），故此处可作为纯共享模块。

/// 获取 `tests/data/` 下的测试数据路径。
pub fn data_path(name: &str) -> String {
    format!("tests/data/{}", name)
}
