# comtrade-io 开发记录

> 全部功能开发已完成并提交（`0f16cd0`）。本文件保留**当前状态**与**已完成工作索引**，
> 详细历史 Review（各轮修正的缺陷、测试明细）已清理归档。`lessons.md` 保留教训记录。

## 当前状态（2026-08-15）

- **版本**：0.0.1（首次发布前）
- **测试**：89 项全绿（47 lib + 集成 42），`scripts/verify.sh` 9 步门禁全绿
- **MSRV**：1.75 实测通过（`cargo +1.75.0 test --lib`）
- **文档**：`cargo doc --no-deps` 0 warning；`missing_docs` 全库锁住（`#![warn(missing_docs)]`）
- **发布包**：298.1KiB（exclude 剔除 tests/docs/examples/tasks/.github/scripts）
- **CI**：`.github/workflows/ci.yml`（8 job）+ `release.yml` + `dependabot.yml`

## 已完成工作索引

| 日期 | 内容 | 关键验证 |
|---|---|---|
| 08-15 | CFF 乱码修复（按段编码探测 + dat_bytes 不 trim） | `test_cff_chinese_channel_name` |
| 08-15 | 发电机 §1.6 / 励磁机 §1.7（INF + DMF 全链路，全字段） | INF/DMF round-trip 测试 |
| 08-15 | 发布打包 B1–B4（exclude/License/元数据/名称核实） | `cargo publish --dry-run` 298KiB |
| 08-15 | 修复 B5–B9（gbk-builtin/doc warning/MSRV/missing_docs/feature 矩阵） | `bash scripts/verify.sh` 全绿 |
| 08-15 | CI + 提交前流程 C1–C3 / D1–D3 | `ci.yml` + `scripts/verify.sh` |
| 08-14 | 设备模型（母线/线路/变压器，INF+DMF） | `test_to_equipment_group` 断言具体值 |
| 08-14 | 六版本号识别（1991…2017） | 版本测试 + 2013 CFF round-trip |
| 08-14 | 命名规范审查 + rustfmt 配置 + Segment 派生字段 | 78 项测试全绿 |

## 剩余外部动作（需用户操作）

- [x] 推送仓库（`git remote` 已切 SSH `git@github.com:zhangsonggui/comtrade-io-rust.git`，`master` 已推）
- [x] GitHub 配置 `CARGO_REGISTRY_TOKEN` secret + `crates-io` environment（release.yml 用，用户已配）
- [x] `cargo publish` 首次发布 **0.0.1**（2026-08-15 上线，打 tag `v0.0.1` 触发 CI 自动发布）
- [ ] 删除 `docs/` 下的 PDF（被占用无法删，关闭阅读器/IDE 后执行）

## 明确搁置 / 超出范围

- 自耦变专有 `TA_Idcw_#1..#5` —— 用户决定搁置（`WINDING_NUM=1` 按两卷变降级，代码留 TODO）
- HDR 解析、DFR 写、通道识别启发式、`CfgToEquipment` —— CLAUDE.md 标注 out of scope
- DMF `SDL_*` 元素 —— 按用户决定与 Python 基线一致地丢弃
