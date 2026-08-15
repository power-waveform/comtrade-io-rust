//! COMTRADE 基础用法示例：多文件加载、配置/数据访问、变位检测、导出。
//!
//! 运行：cargo run --example basic

use std::time::Instant;

use comtrade_io::{Comtrade, DataType, ExportFormat};

fn main() {
    let total = Instant::now();

    // 1. 从 CFG 文件加载整组（CFG+DAT 必需，INF/DMF/HDR 自动探测兄弟文件）
    let path = "tests/data/binary_1999.cfg";
    let t = Instant::now();
    let ct = Comtrade::from_path(path).expect("加载失败");
    let load_ms = t.elapsed().as_secs_f64() * 1000.0;
    println!("=== 已加载: {} ===", path);

    // 2. 配置信息
    let cfg = &ct.config;
    println!("站名     : {}", cfg.header.station);
    println!("录波器   : {}", cfg.header.recorder);
    println!("版本     : {}", cfg.header.version.as_str());
    println!(
        "通道     : 总{} 模拟{} 状态{}",
        cfg.channels.total, cfg.channels.analog, cfg.channels.status
    );
    println!("标称频率 : {} Hz", cfg.sampling.freq);
    println!(
        "采样段   : {} 段，末段结束点 {}",
        cfg.sampling.segments.len(),
        cfg.sampling
            .segments
            .last()
            .map(|s| s.end_point)
            .unwrap_or(0)
    );
    println!(
        "起始时间 : {}-{:02}-{:02} {:02}:{:02}:{:02}.{:06}",
        cfg.start_time.year,
        cfg.start_time.month,
        cfg.start_time.day,
        cfg.start_time.hour,
        cfg.start_time.minute,
        cfg.start_time.second,
        cfg.start_time.micro
    );

    // 3. 模拟通道列表
    println!("\n--- 模拟通道 ---");
    for a in &cfg.analogs {
        println!(
            "#{:<2} {:<8} 相位{} 单位{} 倍率{} 偏移{}",
            a.index, a.name, a.phase, a.unit, a.multiplier, a.offset
        );
    }

    // 4. 数据概览：取第 1 个模拟通道（1-based 索引）的前 5 个工程值
    if let Some(ref dat) = ct.data {
        println!("\n--- 数据 ---");
        println!("采样点数 : {}", dat.len());
        if let Some(view) = ct.analog_channel(1) {
            let head: Vec<String> = view
                .samples
                .iter()
                .take(200)
                .map(|v| format!("{:.3}", v))
                .collect();
            println!(
                "通道[{}] 前200个值: {}",
                view.definition.name,
                head.join(", ")
            );
        }
    }

    // 5. 状态变位检测
    let t = Instant::now();
    let changes = ct.changed_statuses();
    let detect_ms = t.elapsed().as_secs_f64() * 1000.0;
    println!("\n--- 状态变位 ---");
    if changes.is_empty() {
        println!("无变位");
    } else {
        for (ch_idx, records) in &changes {
            println!("通道 #{} 发生 {} 次变位", ch_idx, records.len());
        }
    }

    // 6. 设备拓扑（来自 DMF，无则 INF，无则空）
    let eg = &ct.equipment;
    println!(
        "\n--- 设备拓扑 ---\n母线{} 线路{} 变压器{}",
        eg.buses.len(),
        eg.lines.len(),
        eg.transformers.len()
    );

    // 7. 导出为 JSON（写到系统临时目录，避免污染仓库）
    let out = std::env::temp_dir().join("comtrade_basic_demo.json");
    let t = Instant::now();
    ct.save(&out, ExportFormat::Json, DataType::Ascii)
        .expect("导出失败");
    let export_ms = t.elapsed().as_secs_f64() * 1000.0;
    println!("\n已导出 JSON: {}", out.display());

    // 耗时汇总
    println!(
        "\n--- 耗时 ---\n加载 {:.3} ms | 变位检测 {:.3} ms | 导出 {:.3} ms | 总计 {:.3} ms",
        load_ms,
        detect_ms,
        export_ms,
        total.elapsed().as_secs_f64() * 1000.0
    );
}
