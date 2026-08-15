//! CFF 单文件解析示例：CFF 是 CFG+DAT+INF+HDR 的复合文件。
//!
//! 运行：cargo run --example cff

use std::time::Instant;

use comtrade_io::CffFile;

fn main() {
    let path = "tests/data/ascii_cff_2013.cff";

    // 方式一：直接用 CffFile 按段切分
    let bytes = std::fs::read(path).expect("读取失败");
    let t = Instant::now();
    let cff = CffFile::from_bytes(&bytes);
    println!("段切分耗时: {:.3} ms", t.elapsed().as_secs_f64() * 1000.0);

    println!("=== CFF 文件: {} ===", path);
    println!("含 CFG 段: {}", cff.sections.cfg.is_some());
    println!("含 DAT 段: {}", cff.sections.dat.is_some());
    println!("含 INF 段: {}", cff.sections.inf.is_some());

    let t = Instant::now();
    let config = cff.to_config().expect("解析 CFG 失败");
    let data = cff.to_data(&config).expect("解析 DAT 失败");
    println!(
        "\n解析(CFG+DAT)耗时: {:.3} ms",
        t.elapsed().as_secs_f64() * 1000.0
    );
    println!(
        "站名     : {}\n通道     : 总{} 模拟{} 状态{}",
        config.header.station,
        config.channels.total,
        config.channels.analog,
        config.channels.status
    );
    println!("采样点数 : {}", data.len());

    // 方式二：用顶层 Comtrade::from_path 走 .cff 单文件模式（推荐）
    let t = Instant::now();
    let ct = comtrade_io::Comtrade::from_path(path).expect("加载失败");
    println!(
        "\n（经 Comtrade::from_path）站名: {}，采样点数: {}，耗时: {:.3} ms",
        ct.config.header.station,
        ct.data.as_ref().map(|d| d.len()).unwrap_or(0),
        t.elapsed().as_secs_f64() * 1000.0
    );
}
