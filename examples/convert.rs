//! 格式转换示例：在 ASCII/BINARY/BINARY32/FLOAT32 间互转，输出多文件与 CFF 单文件。
//!
//! 运行：cargo run --example convert

use std::time::Instant;

use comtrade_io::{Comtrade, DataType, Export, ExportFormat};

fn main() {
    let src_path = "tests/data/binary_1999.cfg";
    let ct = Comtrade::from_path(src_path).expect("加载失败");
    println!("=== 源文件: {} ===", src_path);
    println!(
        "原始格式: {}，采样点数: {}\n",
        ct.config.data_type.as_str(),
        ct.data.as_ref().map(|d| d.len()).unwrap_or(0)
    );

    let out_root = std::env::temp_dir().join("comtrade_convert_demo");
    let _ = std::fs::remove_dir_all(&out_root);
    std::fs::create_dir_all(&out_root).unwrap();

    for dt in [
        DataType::Ascii,
        DataType::Binary,
        DataType::Binary32,
        DataType::Float32,
    ] {
        println!("--- 转换为 {} ---", dt.as_str());

        // 多文件（CFG+DAT+可选 INF/DMF）
        let mf_dir = out_root.join(format!("mf_{}", dt.as_str().to_lowercase()));
        let t = Instant::now();
        ct.write_to_dir(&mf_dir, "out", dt).expect("多文件写出失败");
        println!(
            "  多文件: {} (耗时 {:.3} ms)",
            mf_dir.display(),
            t.elapsed().as_secs_f64() * 1000.0
        );

        // CFF 单文件
        let cff_path = out_root.join(format!("out_{}.cff", dt.as_str().to_lowercase()));
        let t = Instant::now();
        ct.save(&cff_path, ExportFormat::Cff, dt)
            .expect("CFF 导出失败");
        println!(
            "  CFF  : {} (耗时 {:.3} ms)",
            cff_path.display(),
            t.elapsed().as_secs_f64() * 1000.0
        );

        // 读回验证格式一致性
        let back = Comtrade::from_path(mf_dir.join("out.cfg")).expect("读回失败");
        assert_eq!(back.config.data_type, dt);
        println!(
            "  读回 : 格式 {}，采样点数 {}",
            back.config.data_type.as_str(),
            back.data.as_ref().map(|d| d.len()).unwrap_or(0)
        );
    }

    println!("\n所有转换产物位于: {}", out_root.display());
}
