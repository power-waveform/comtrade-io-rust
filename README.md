# comtrade-io

COMTRADE（IEEE C37.111）故障录波文件解析与导出库，纯 Rust 实现。

支持 CFG / DAT / INF / DMF / CFF / DFR 格式的读写，以及在 ASCII / BINARY / BINARY32 / FLOAT32 四种 DAT 格式间互转，并可导出为 JSON / CSV。

## 功能特性

- **多格式读写**：CFG、DAT、INF、DMF、CFF（单文件复合）全双向；DFR 只读
- **格式转换**：在 ASCII / BINARY / BINARY32 / FLOAT32 间互转，输出多文件或 CFF 单文件
- **多种导出**：JSON、CSV、CFF、多文件回写
- **零重度依赖**：不依赖 serde / chrono / quick-xml；JSON、CSV、XML、时间解析均为手写。仅 `encoding_rs` 用于 GBK 编解码
- **列式存储**：DAT 按通道列存储，对齐 DataFrame 按列访问模式
- **往返保真**：各格式 `parse → serialize → reparse` 语义等价；未知内容保留原文
- **错误即上下文**：统一 `Error` 枚举，解析错误携带行号/偏移，对外不 panic

## 文档

- **API 参考（rustdoc）**：`docs/rustdoc/comtrade_io/index.html`（本地构建：`cargo doc --open`；发布后在线版见 docs.rs）
- **设计文档**：`docs/rust重构软件设计报告.md` / `docs/rust重构需求分析报告.md`（权威规格）

## 安装

`Cargo.toml`：

```toml
[dependencies]
comtrade-io = "0.0.1"
```

> MSRV：Rust 1.75

### Feature flags

- `gbk-builtin`（默认关闭）：用内置精简 GBK 编解码替换 `encoding_rs::GBK`（`encoding_rs` 仍会编译进依赖树，只是不再被调用）。**该内置实现不完整**——GBK 中文（CJK 区间）一律解码为 `U+FFFD`，仅适合 ASCII 为主的内容；默认路径（`encoding_rs`）才是完整可用路径。

## 快速上手

```rust
use comtrade_io::{Comtrade, DataType, ExportFormat};

// 从任一成员文件加载整组（CFG+DAT 必需，INF/DMF/HDR 自动探测兄弟文件）
let ct = Comtrade::from_path("tests/data/binary_1999.cfg")?;

println!("站名: {}", ct.config.header.station);
println!("通道: 模拟{} 状态{}", ct.config.channels.analog, ct.config.channels.status);

// 取第 1 个模拟通道（1-based 索引）的工程值
if let Some(view) = ct.analog_channel(1) {
    println!("通道[{}] 前5个值: {:?}",
        view.definition.name,
        view.samples.iter().take(5).collect::<Vec<_>>());
}

// 状态变位检测
let changes = ct.changed_statuses();
```

`from_path` 按扩展名分派：`.cff` / `.dfr` 走单文件模式，其余按 `parent + stem` 定位兄弟文件（大小写风格跟随输入）。

## 格式转换

加载后可转成任意 DAT 格式，输出多文件或 CFF：

```rust
// 转成 FLOAT32 多文件（CFG + DAT + 可选 INF/DMF）
ct.write_to_dir("out_dir".as_ref(), "out", DataType::Float32)?;

// 等价写法
ct.save("out_dir/out".as_ref(), ExportFormat::MultiFile, DataType::Float32)?;

// 转成 BINARY32 单文件 CFF
ct.save("out.cff".as_ref(), ExportFormat::Cff, DataType::Binary32)?;
```

四种格式说明：

| 格式 | 模拟量存储 | 说明 |
|---|---|---|
| ASCII | 文本整数 | 反算 raw，3 位小数精度 |
| BINARY | i16 整数 | 反算 raw |
| BINARY32 | i32 整数 | 反算 raw |
| FLOAT32 | f32 浮点 | 按 IEEE C37.111 直存工程值，不应用 mult/offset |

## 导出

```rust
// JSON / CSV
ct.save("out.json".as_ref(), ExportFormat::Json, DataType::Ascii)?;
ct.save("out.csv".as_ref(), ExportFormat::Csv, DataType::Ascii)?;
```

`DataType` 参数对 JSON / CSV 导出无影响（仅决定 COMTRADE DAT 格式），传任意值即可。

## 构建与测试

```bash
cargo build                       # 构建
cargo test                        # 全部测试（集成 + 单元 + 文档测试）
cargo test --test convert_tests   # 单个集成测试文件
cargo test test_float32           # 按名运行单个测试
cargo run --example basic         # 运行基础用法示例
cargo run --example convert       # 运行格式转换示例
cargo run --example cff           # 运行 CFF 解析示例
```

## 示例

| 示例 | 说明 | 运行 |
|---|---|---|
| `basic` | 多文件加载、配置/数据访问、变位检测、导出 | `cargo run --example basic` |
| `convert` | 4 种 DAT 格式互转（多文件 + CFF） | `cargo run --example convert` |
| `cff` | CFF 单文件解析 | `cargo run --example cff` |

## 项目结构

```
src/
├── lib.rs          # 公共 API 再导出
├── comtrade.rs     # Comtrade 顶层聚合 + 文件组定位
├── cfg/            # CFG 配置读写（Config / AnalogChannel / StatusChannel）
├── dat/            # DAT 数据读写（ASCII / binary 列式存储）
├── inf/            # INF 信息文件（INI 风格节）
├── dmf/            # DMF 设备模型（手写 XML 读写）
├── cff/            # CFF 单文件（字节级段切分）
├── dfr/            # DFR 只读（WNDR 头 + 二进制区）
├── exporters/      # JSON / CSV / CFF / 多文件导出
├── encoding.rs     # UTF-8 / GBK / cp1251 编解码
├── time.rs         # COMTRADE 时间解析（微秒精度，无 chrono）
├── equipment.rs    # 设备拓扑模型（Bus/Line/Transformer）
└── error.rs        # 统一 Error / Result
tests/data/         # 测试样本（含 GBK 文件、CFF、DMF）
examples/           # 用法示例
```

## 许可证

MIT
