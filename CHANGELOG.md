# Changelog

本文件记录 comtrade-io crate 的变更。格式参考 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，
版本号遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

解析/写出行为的语义变化（即使签名不变）视为破坏性变更，必须升主版本或次版本并在此说明迁移方式。

## [Unreleased]

（待定）

## [0.1.0] - 2026-09-17

首个成功发布到 crates.io 的版本。上一个打 tag 的 `0.0.3` 仅为内部开发里程碑，从未对外发布（发布流程当时有缺陷）；本版本在保留其全部内容的基础上，修复发布并统一对外版本号。

### Fixed

- 修复 crates.io 自动发布失败：多包 workspace 下根包 `cargo publish` 被拒，因全部子 crate 依赖仅声明 `path` 而无 `version`（cargo 报
  `all dependencies must have a version requirement specified when publishing`）。为每个子 crate 依赖补 `version`，与 `path` 并存——本地构建走 `path`，发布时由 cargo 移除 `path` 改用 crates.io 上的对应版本解析。
- `release` workflow 由单次 `cargo publish` 改为按依赖拓扑序逐个 `cargo publish -p` 发布全部 12 个 crate：
  `cbase → cfg → dat → dmf → inf → cff → dfr → model → export → edit → comtrade-io → recognition`。
  父 crate 只在被其依赖的子 crate 上线后才发布，避免解析到 crates.io 上的缺失版本。
- 版本号由 `0.0.3` 统一升为 `0.1.0`（全 workspace 各 crate 与 `recognition` 对齐），作为首个正式对外发布版本；每位使用方以
  `comtrade-io = "0.1"`（或其任意子 crate 名）从 crates.io 拉取。

## [0.0.3] - 2026-09-17

### Changed

- **仓库重构为多 crate Cargo workspace**：CFG / DAT / INF / DMF / CFF / DFR / 导出 / 编辑
  拆分为 `crates/` 下的独立 crate（`cbase` / `cfg` / `dat` / `dmf` / `inf` / `cff` / `dfr` /
  `model` / `export` / `edit` / `recognition`）。根包 `comtrade-io` 降为纯 facade，
  re-export 全部子 crate。应用层既可整体依赖根包，也可单独依赖某一子 crate
  （如仅用 `cfg`、仅用 `dat`）。各子 crate 依赖方向为有向无环图，无环形依赖。
- 子 crate 命名去掉 `comtrade-` 前缀（如 `comtrade-cfg` → `cfg`）；`comtrade-recognition`
  更名 `recognition`。对外根包接口名不变，应用层无需改动。
- 版本统一为 `0.0.3`（`recognition` 由 `0.1.0` 对齐）。
- **导出与编辑能力 trait 化**：`save` / `save_with` / `to_csv` / `to_json` 改为
  `Export` trait（`export` crate）为 `Comtrade` 提供；`set_analog_column` /
  `set_status_column` / `remove_channel` / `insert_analog_channel` /
  `insert_status_channel` / `crop_rows` 改为 `DataEdit` trait（`edit` crate）。
  消费方（wave-tauri `data_edit.rs`）导入 `comtrade_io::{Export, DataEdit}` 即可。
- **大文件按功能拆分**：`cfg` 抽出 `version` / `data_type` / `tran_side` / `reference`
  独立模块，`channel.rs` 只保留通道模型；`inf` / `dmf` / `dat` / `time` / `equipment`
  等也按职责拆分，避免单文件数百上千行。全部逻辑原样搬迁，无解析/序列化语义差异。
- **内联测试拆分**：cfg / dat 等 crate 的 `#[cfg(test)]` 测试移到各自 `tests/`
  集成测试目录，生产文件不再混测试代码。测试数量与通过率不变。

### Added

- 波形数据编辑写入口：`DataEdit` trait 提供 `set_analog_column` / `set_status_column`
  （按列替换工程值/状态值，校验长度与取值）、`remove_channel` / `insert_analog_channel` /
  `insert_status_channel`（增删通道时原子同步 CFG 通道定义、计数与 DAT 列并重编号 1 基
  index）、`crop_rows`（保留 `[start,end)` 行区间并重算采样段）。`Comtrade` 另提供
  `to_cff_bytes` / `write_cff`（CFF 单文件便捷写出，封装 `CffFile::encode`）。
  新增 `ChannelKind` 枚举。编辑只改采样数据与通道定义，不重建设备拓扑（`equipment`），
  设备组引用由调用方负责。为 wave-tauri 波形数据编辑功能（方案 §4）提供底层写能力。
- 公开导出 `DmfAnalogChannel` 与 `DmfStatusChannel`（原先仅 `DmfFile` 可见），
  供调用方（wave-tauri 模型加载链）基于 CFG 构造最小 DMF。
- CFG/INF 通道 `ccbm` / `Monitored_Component` 兼容：对 `MUSV...$...`、
  `SVOUT...$...`、`PTRC$...`、`TCTR$...` 等 IEC 61850 源引用不再作为被监视元件参与
  设备归组，解析时转存到通道扩展 `reference` 字段；生成 DMF 时开关量的 `srcRef`
  保留该引用。
- 仓库改为 Cargo workspace，新增 `recognition` crate，集中提供模拟量/开关量
  通道识别、备用通道判定、母线/线路/主变基础归组和 CFG→DMF 会话模型生成；
  `wave-tauri` 改为直接依赖该 crate，不再维护识别算法副本。
- 模拟量与开关量采用独立备用规则。模拟量默认识别变比为 1、空名称、备用/模拟量/
  电压/电流及 Ua/Ub/Uc/Ia/Ib/Ic 占位名称；开关量默认识别备用/开关量/开关占位名称；
  两类均支持仅由空格和数字组成的占位名称。
- 识别规则配置缺失、损坏或版本不支持时自动回退内置默认规则；人工导入仍使用严格
  校验，避免无效文件被当作成功导入。规则格式升级到 v2，v1 配置加载时自动补齐
  分类型备用规则并升级内存格式，不覆盖已有自定义规则。
- DMF 模拟通道读取时，若交流通道省略 `au` 属性，按标准默认值补为 `1.0`；
  `bu` 和 `idx_rlt` 缺省仍分别为 `0.0` 和 `0`。显式写入的 `au`（包括 0）保持原值。
  `DmfAnalogChannel` 新增 `idx_rlt` 关联通道字段；DMF 解析兼容 `idx_rlt` 与早期
  `idx_rl` 拼法，写出统一使用 `idx_rlt`，不再固定为 0。

## [0.0.2] - 2026-08-31

### Changed

- 公开 API 命名规范化：`DatFile::get_analog_samples` / `get_status_samples`
  去掉 `get_` 前缀，改为 `analog_samples` / `status_samples`（签名不变）。
  调用方（wave-tauri、wave-server）已同步迁移。
- `Line.bus_id` 等设备模型字段维持 `String` 类型（与 Python 版的
  `bus_index: int` 不同，属于已知模型差异，见 wave-tauri 任务记录）。

### Known Issues

- 设备模型弱于 Python 版 comtrade-io：缺 `ACVBranch` / `ACCBranch` /
  `Impedance` / `Capacitance` / `MutualInductance` / `WindGroup` / `Igap`，
  模型编辑器所需可编辑字段待后续版本补充。

## [0.0.1] - 2026-08

### Added

- CFG / DAT / INF / DMF / CFF（单文件复合）全双向读写；DFR 只读。
- DAT 四种数据格式（ASCII / BINARY / BINARY32 / FLOAT32）解析与互转。
- `Comtrade::from_path` 按扩展名自动分派：`.cff` / `.dfr` 单文件模式，
  其余按 `parent + stem` 定位兄弟文件（INF/DMF/HDR 自动探测）。
- 列式存储（DAT 按通道列），`analog_channel` / `changed_statuses` 等
  按列访问 API。
- 往返保真：各格式 `parse → serialize → reparse` 语义等价，未知内容保留原文。
- 统一 `Error` 枚举，解析错误携带行号/偏移，对外不 panic。
- JSON / CSV 导出（手写实现，不依赖 serde / chrono）。
- GBK 编解码（`encoding_rs`）；可选 `gbk-builtin` feature 提供不完整的
  精简内置实现（CJK 区间解码为 `U+FFFD`，仅供实验）。

[Unreleased]: https://github.com/PowerWaveForm/comtrade-io-rust/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/PowerWaveForm/comtrade-io-rust/compare/v0.0.3...v0.1.0
[0.0.3]: https://github.com/PowerWaveForm/comtrade-io-rust/compare/v0.0.2...v0.0.3
[0.0.2]: https://github.com/PowerWaveForm/comtrade-io-rust/compare/v0.0.1...v0.0.2
[0.0.1]: https://github.com/PowerWaveForm/comtrade-io-rust/releases/tag/v0.0.1