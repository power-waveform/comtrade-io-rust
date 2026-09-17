# comtrade-io

COMTRADE（IEEE C37.111）故障录波文件解析与导出库，纯 Rust 实现。

支持 CFG / DAT / INF / DMF / CFF / DFR 格式的读写，以及在 ASCII / BINARY / BINARY32 / FLOAT32 四种 DAT 格式间互转，并可导出为 JSON / CSV，或就地编辑通道与采样数据后回写。

## 功能特性

- **多格式读写**：CFG、DAT、INF、DMF、CFF（单文件复合）全双向；DFR 只读
- **格式转换**：在 ASCII / BINARY / BINARY32 / FLOAT32 间互转，输出多文件或 CFF 单文件
- **多种导出**：JSON、CSV、CFF、多文件回写
- **波形编辑**：通道整列替换、通道增删、采样行裁剪（`DataEdit` trait）
- **零重度依赖**：不依赖 serde / chrono / quick-xml；JSON、CSV、XML、时间解析均为手写。仅 `encoding_rs` 用于 GBK 编解码
- **列式存储**：DAT 按通道列存储，对齐 DataFrame 按列访问模式
- **往返保真**：各格式 `parse → serialize → reparse` 语义等价；未知内容保留原文
- **错误即上下文**：统一 `Error` 枚举，解析错误携带行号/偏移，对外不 panic
- **多 crate 可拆用**：仓库为 Cargo workspace，可整体依赖根包 facade，也可单独依赖任一格式子 crate。发布到 crates.io 时注册名统一带 `comtrade-` 前缀（`comtrade-cfg` / `comtrade-dat` …，因短名多已被占用），但 Rust 里的 crate 引用仍是短名 `cfg` / `dat`。单独使用时在 Cargo.toml 声明 `comtrade-cfg = "0.1"`，代码中 `use cfg::…` 即可。

## 文档

- **API 参考（rustdoc）**：入口 [`docs/rustdoc/index.html`](docs/rustdoc/index.html)——包含根包 `comtrade_io` 及各子 crate（`cfg` / `dat` / `inf` / `dmf` / `cff` / `dfr` / `model` / `export` / `edit` / `recognition` / `cbase`）的全部接口。本地重建：`cargo doc --workspace --no-deps`，产物在 `target/doc`，镜像到 `docs/rustdoc`
- **设计文档**：`docs/rust重构软件设计报告.md` / `docs/rust重构需求分析报告.md`（权威规格）

## 安装

`Cargo.toml`：

```toml
[dependencies]
comtrade-io = "0.1.0"
```

> MSRV：Rust 1.75

### Feature flags

- `gbk-builtin`（默认关闭）：用内置精简 GBK 编解码替换 `encoding_rs::GBK`（`encoding_rs` 仍会编译进依赖树，只是不再被调用）。**该内置实现不完整**——GBK 中文（CJK 区间）一律解码为 `U+FFFD`，仅适合 ASCII 为主的内容；默认路径（`encoding_rs`）才是完整可用路径。

## 快速上手

```rust
use comtrade_io::{Comtrade, DataType};

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

## 格式转换与导出

加载后可转成任意 DAT 格式，输出多文件或 CFF。`save` / `write_to_dir` 由 `Export` trait 提供，需导入：

```rust
use comtrade_io::{Comtrade, DataType, Export, ExportFormat};

// 转成 FLOAT32 多文件（CFG + DAT + 可选 INF/DMF）
ct.write_to_dir("out_dir".as_ref(), "out", DataType::Float32)?;

// 等价写法
ct.save("out_dir/out".as_ref(), ExportFormat::MultiFile, DataType::Float32)?;

// 转成 BINARY32 单文件 CFF
ct.save("out.cff".as_ref(), ExportFormat::Cff, DataType::Binary32)?;

// JSON / CSV（DataType 参数无影响，传任意值即可）
ct.save("out.json".as_ref(), ExportFormat::Json, DataType::Ascii)?;
ct.save("out.csv".as_ref(), ExportFormat::Csv, DataType::Ascii)?;
```

四种 DAT 格式说明：

| 格式 | 模拟量存储 | 说明 |
|---|---|---|
| ASCII | 文本整数 | 反算 raw，3 位小数精度 |
| BINARY | i16 整数 | 反算 raw |
| BINARY32 | i32 整数 | 反算 raw |
| FLOAT32 | f32 浮点 | 按 IEEE C37.111 直存工程值，不应用 mult/offset |

## 波形编辑

编辑能力由 `DataEdit` trait 提供，只改采样数据与通道定义，不重建设备拓扑（`equipment`）：

```rust
use comtrade_io::{Comtrade, DataEdit, ChannelKind};

let mut ct = Comtrade::from_path("tests/data/binary_1999.cfg")?;

// 替换第 1 个模拟通道整列（0 基列下标，长度须与采样点数一致）
let samples: Vec<f64> = vec![0.0; ct.data.as_ref().map(|d| d.len()).unwrap_or(0)];
ct.set_analog_column(0, samples)?;

// 删除一条状态量通道，CFG 定义与计数原子同步、其余通道自动重编号
ct.remove_channel(ChannelKind::Status, 0)?;

// 裁剪行区间 [start, end)，并重算采样段
ct.crop_rows(0, 2400)?;
```

## 构建与测试

```bash
cargo build                        # 构建整个 workspace
cargo test                         # 全部测试（集成 + 单元 + 文档测试）
cargo doc --workspace --no-deps    # 本地 API 文档（target/doc）
cargo test --test convert_tests    # 单个集成测试文件
cargo test test_float32            # 按名运行单个测试
cargo run --example basic          # 运行基础用法示例
cargo run --example convert        # 运行格式转换示例
cargo run --example cff            # 运行 CFF 解析示例
```

## 示例

| 示例 | 说明 | 运行 |
|---|---|---|
| `basic` | 多文件加载、配置/数据访问、变位检测、导出 | `cargo run --example basic` |
| `convert` | 4 种 DAT 格式互转（多文件 + CFF） | `cargo run --example convert` |
| `cff` | CFF 单文件解析 | `cargo run --example cff` |

## 项目结构

仓库采用 Cargo workspace，根包 `comtrade-io` 是纯 facade（re-export 各子 crate）。
应用层可整体依赖根包，也可只依赖某个子 crate（如仅解析 CFG 用 `cfg`，仅做编辑用 `edit`）。

```
comtrade-io (根，facade)           # 只 re-export，消费方统一入口
crates/
├── cbase/        # 共享地基：error / time / encoding / equipment 拓扑
├── cfg/          # CFG 配置读写（Config / AnalogChannel / StatusChannel）
├── dat/          # DAT 数据读写（ASCII / BINARY / BINARY32 / FLOAT32，列式）
├── inf/          # INF 信息文件（INI 风格节）
├── dmf/          # DMF 设备模型（手写 XML 读写）
├── cff/          # CFF 单文件（字节级段切分）
├── dfr/          # DFR 只读（WNDR 头 + 二进制区）
├── model/        # 聚合 Comtrade + ComtradePaths / ChannelKind / ChannelView / HdrFile
├── export/       # Export trait：JSON / CSV / CFF / 多文件导出
├── edit/         # DataEdit trait：通道列替换 / 增删 / 裁剪
└── recognition/  # 通道识别、备用判定、设备归组、CFG→DMF、TOML 规则
examples/         # 用法示例
tests/            # 跨格式 / 整组加载集成测试（tests/data/ 为样本）
docs/             # API 文档（rustdoc）+ 权威设计规格（md）
```

各子 crate 依赖方向为有向无环图——格式 crate → `cbase`；`model` 依赖全部格式 crate；
`export` / `edit` 依赖 `model`；根 facade 依赖全部；`recognition` 依赖根 facade。

## 许可证

MIT