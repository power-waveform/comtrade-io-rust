# Changelog

本文件记录 comtrade-io crate 的变更。格式参考 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，
版本号遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

解析/写出行为的语义变化（即使签名不变）视为破坏性变更，必须升主版本或次版本并在此说明迁移方式。

## [Unreleased]

## [0.0.3] - 2026-08-31

### Added

- 公开导出 `DmfAnalogChannel` 与 `DmfStatusChannel`（原先仅 `DmfFile` 可见），
  供调用方（wave-tauri 模型加载链）基于 CFG 构造最小 DMF。
  纯新增，无破坏性变更。

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

[Unreleased]: https://github.com/PowerWaveForm/comtrade-io-rust/compare/v0.0.2...HEAD
[0.0.2]: https://github.com/PowerWaveForm/comtrade-io-rust/compare/v0.0.1...v0.0.2
[0.0.1]: https://github.com/PowerWaveForm/comtrade-io-rust/releases/tag/v0.0.1
