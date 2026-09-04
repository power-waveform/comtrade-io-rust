# Changelog

本文件记录 comtrade-io crate 的变更。格式参考 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，
版本号遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

解析/写出行为的语义变化（即使签名不变）视为破坏性变更，必须升主版本或次版本并在此说明迁移方式。

## [Unreleased]

### Added

- CFG/INF 中以 `PTRC$...`、`TCTR$...` 等形式出现的 IEC 61850 `ccbm`/`Monitored_Component`
  参引统一转存到通道扩展 `reference`，不再误作为被监视元件参与设备归组；CFG→DMF
  生成会保留状态量 `srcRef`。开关量设备关联增加按通道名称设备编号锚点的保守匹配，
  无法确定时保持未关联，交由模型配置页面人工设置。

- CFG/INF 通道 `ccbm`/`Monitored_Component` 兼容：对 `MUSV...$...`、`SVOUT...$...`、
  `PTRC$...`、`TCTR$...` 等 IEC 61850 源引用不再作为被监视元件参与归组，解析时转存
  到通道扩展 `reference` 字段；生成 DMF 时开关量的 `srcRef` 保留该引用。

- 仓库改为 Cargo workspace，新增 `comtrade-recognition` crate，集中提供模拟量/开关量
  通道识别、备用通道判定、母线/线路/主变基础归组和 CFG→DMF 会话模型生成；
  `wave-tauri` 改为直接依赖该 crate，不再维护识别算法副本。
- 模拟量与开关量采用独立备用规则。模拟量默认识别变比为 1、空名称、备用/模拟量/
  电压/电流及 Ua/Ub/Uc/Ia/Ib/Ic 占位名称；开关量默认识别备用/开关量/开关占位名称；
  两类均支持仅由空格和数字组成的占位名称。
- 识别规则配置缺失、损坏或版本不支持时自动回退内置默认规则；人工导入仍使用严格
  校验，避免无效文件被当作成功导入。规则格式升级到 v2，v1 配置加载时自动补齐
  分类型备用规则并升级内存格式，不覆盖已有自定义规则。

- DMF 模拟通道读取时，若交流通道省略 `au` 属性，按标准默认值补为 `1.0`；
  `bu` 和 `idx_rlt` 缺省仍分别为 `0.0` 和 `0`。显式写入的 `au`（包括 0）保持
  原值，避免覆盖用户配置。
- `DmfAnalogChannel` 新增 `idx_rlt` 关联通道字段；DMF 解析同时兼容
  `idx_rlt` 与早期 `idx_rl` 拼法，写出统一使用 `idx_rlt`，不再固定为 0。

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
