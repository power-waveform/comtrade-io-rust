# 补全母线 / 线路 / 变压器设备模型识别

## 问题

`src/equipment.rs` 的 `Bus` / `Line` / `Transformer` / `TransformerWinding` 只有约 30 个字段，
而需求文档 §2.6 与实际样本文件里的设备模型远多于此。DMF 与 INF 两条解析路径都不完整：

**DMF（`src/dmf/xml_reader.rs`）** 只处理 7 个元素名，其余走 `_ => {}` 丢弃：
- 子元素全丢：`ACVChn`（电压通道组）、`ACC_Bran`（电流分支）、`RX`（阻抗）、`CG`（电容电导）、
  `PX`、`MR`（互感）、`Igap`、`AnaChn` / `StaChn`（通道引用）、`DifferentialCurrent`。
  这些是"设备↔通道"的接线关系，丢掉后设备模型只剩名字和额定值，等于没有拓扑。
- 属性遗漏：`Bus.VRtgSnd` / `VRtgSnd_Pos` / `is_location`；`Line.ARtgSnd` / `remote_ID` /
  `remote_Flag` / `differential_ID`。
- `TransformerWinding.wg` 类型错误：声明为 `f64`，但样本值是 `y0` / `yn0`（绕组接线组别代号），
  `value.parse().unwrap_or(0.0)` 让所有绕组组别静默变成 `0.0`。
- 自闭合 `<scl:TransformerWinding ... />` 不产生 `End` 事件，而 `Start` 分支没有 `empty` 检查
  （`Bus`/`Line`/`Transformer` 三者都有），导致自闭合绕组被静默丢弃。

**INF（`src/inf/builder.rs:266`）** `build_equipment_group` 找的 Key 完全不对：
- 找的是 DMF 风格的 `bus_name` / `line_name` / `trm_name`，
  但 INF 实际用 `DEV_ID` / `RATED_VALUE` / `TV_RATIO` / `TV_CHNS` / `TV_POS` /
  `OBJECT_TYPE` / `LENGTH` / `RX` / `CG` / `MRX` / `TA_CHNS` / `STATUS_CHNS` /
  `CAPACITY` / `WINDING_NUM` / `H_PARAM` / `M_PARAM` / `L_PARAM` / `TA_Id_#N`。
  实测 `binary_inf.inf` 的 7 个 Bus、6 个 Line、1 个 Transformer **名字全为空、绕组数为 0**。
- `SectionKind::Other` 把节名大写化存储并原样写回，
  `[ZYHD Channels_Group_#1]` → `[ZYHD CHANNELS_GROUP_#1]`，30 个 `Channels_Group` +
  `ZprdInfo` + `ZprdTrip` 共 32 节在 round-trip 中被改名。
- 写出用 `area: "Public"`，样本用 `ZYHD`。

**现有测试为什么没发现**：`test_to_equipment_group` / `test_dmf_to_equipment_group` 只断言
"不 panic"（`let _ = eg.buses.len();`），没有校验任何字段值。

## 权威依据

`docs/COMTRADE1999补充定义(090917).pdf`（12 页，已用 PyMuPDF 抽取为
`docs/_spec_extracted.txt` 便于检索）是 INF 私有段的**规范定义**，优先级高于 Python 基线行为。
关键条款：

- **§1.4 线路段** `[ZYHD LINE_#n]`：
  - `RX=R1,X1,R0,X0`（Ω/km）、**`CG=C1,G1,C0,G0`**、`MRX=MR,MX`（每千米互感电阻/电抗）。
  - `TA_CHNS=Ia,Ib,Ic,Io,DIR`，**`DIR` 为 1 或 -1，可选，缺省为 1**；1=正方向（流向线路），-1=反方向。
  - `TA_CHNS_#2` = 第二组电流（同样带 DIR）。`TV_CHNS_#n` = 第 n 组电压，与 `TV_CHNS` 兼容。
  - `REACTOR`：并联电抗器补偿电抗(Ω)，**`NO` 表示没有**（不是数值）。
  - `OBJECT_TYPE`：`LINE` / `BYPASS` / `BUS_CONNECTION` / `RAILWAY`。
  - `OTHER_ID`（双回线的另一回 ID）、`OTH_ACHNS`（其他模拟量通道表）。
- **§1.5 变压器段** `[ZYHD TRANSFORMER_#n]`：
  - `WINDING_NUM`：**1=自耦变, 2=两卷变, 3=三卷变**。
  - `{H,M,L}_PARAM=接线方式, 一次额定电压(kV), 分支数` —— 第三字段是**分支数**，规范明确。
  - `TA_Id_#N=Ia, Ib, Ic, 极性符号(1 或 -1)`，**只有三相无 Io**；电流流入变压器为 `+1`。
    映射：**`#1/#2`→高压侧一/二分支，`#3/#4`→中压侧一/二分支，`#5/#6`→低压侧一/二分支**，
    `#7`=低压侧三分支。中压侧标注"三卷变有效"。
  - `{H,M,L}_TV_CHNS=Ua,Ub,Uc,Un`；`M_TV_CHNS` 三卷变有效。
  - 另有 `TA_Idcw_#1..#5`（自耦变分侧&零序差动）、`{H,M,L}_TA_ZS` / `_ZS_GAP`（零序/间隙零序）、
    `H_TA_BK`（后备 TA）、`{H,M,L}_STATUS_CHNS`、`Z_STATUS_CHNS`、`OTH_STATUS_CHNS`、
    `TA_SELF_COMP`（YES/NO）、`OBJECT_TYPE`（MAIN/PLANT/EXCITATION）。
- **§1.6 发电机段** `[ZYHD POWER_#n]`、**§1.7 励磁机段** `[ZYHD EXCITATION_#n]`：
  本轮**不实现**（用户只要求母线/线路/变压器），但记录在此以备后续。

### PDF 纠正了我先前的三处误判
| 项 | 我原先的假设 | 规范实际 |
|---|---|---|
| `CG` 字段序 | `c1,c0,g1,g0` | **`C1,G1,C0,G0`**（正序容/正序导/零序容/零序导交替） |
| `TA_Id_#N` 尾值 | 可能是 `in`/零序通道 | **极性符号 1/-1**，且该行只有三相 `Ia,Ib,Ic` 无 `Io` |
| `REACTOR` | 纯数值 | 可为 **`NO`** 字面量 |

## 已确认的决策

1. **`SDL_*` / `PX` / `DifferentialCurrent` / `ACI_Bran` 等未建模元素：与 Python 一致地丢弃。**
   不引入 `raw_children` 原样透传。本轮只补需求文档 §2.6 点名的元素。
2. **Python 基线的读写不对称 bug：Rust 侧修正，并在代码注释中记录偏差。**
3. **DMF 与 INF 两条路径一起修。**
4. **电流方向 `DIR` / 极性符号：`i32`，缺省 `1`，取值 `1` / `-1`。**（用户确认 + 规范一致）
5. **绕组数按 `WINDING_NUM` 语义处理：两卷变 = 高压 + 低压两侧（缺中压侧）。**（用户确认）
   即 `WINDING_NUM=2` 时建 `#1/#2`(高) 与 `#5/#6`(低)，跳过中压；`=3` 时三侧齐全。

### 需要修正并注释的 Python 偏差
| 位置 | Python 行为 | Rust 采取 |
|---|---|---|
| DMF `ACVChn` | 读 `ACVChn` 写 `ACVBranch` | 读写都用 `ACVChn` |
| DMF `ACC_Bran` | 读 `bran_idx` 写 `idx` | 读写都用 `bran_idx` |
| DMF `TransformerWinding.srcRef` | 读了但从不写出 | 写出 |
| DMF `Bus`/`Line` 开标签 | 自闭合 `/>` 却带子元素+闭合标签（XML 非法） | 不自闭合，产出合法 XML |
| INF `TA_Id_#N` | 解析器 H→#5,M→#1,L→#3；写出器 H→#1,M→#5,L→#3（自相矛盾） | 按规范 §1.5：**#1/#2→高压，#3/#4→中压，#5/#6→低压**（与样本通道名一致：ch1-3=高压侧, ch15-17=中压侧, ch29-31=低压侧） |
| INF `TA_Id_#N` 尾值 | Python `str2ids` 要求严格递增，把极性符号当通道号后丢弃 | 独立解析为 `polarity: i32`（1/-1），不混入通道表 |
| INF `TA_CHNS` 尾值 | 同上被 `str2ids` 吞掉 | 独立解析为 `dir: i32`（1/-1，缺省 1） |
| INF `CG` 字段序 | Python `parse_four_values` 按 `c1,g1,c0,g0` 读入却存成 `c1,c0,g1,g0` | 按规范 `C1,G1,C0,G0` |
| INF 绕组 `TV_CHNS` | 写裸 `TV_CHNS`，读 `H_/M_/L_TV_CHNS` | 读写都用 `H_/M_/L_TV_CHNS` 前缀 |
| INF `WINDING_NUM` | 存了但不用，恒解析 3 个绕组 | 按语义：1=自耦, 2=高+低, 3=高+中+低 |
| INF `TV_RATIO` | `"220KV/100V"`（大写 KV）误判成 0.22kV | 大小写不敏感识别 kV |
| INF `parse_number_with_unit` | 正则 `\d+\.?\d*` 丢负号，`-1(Ω)` → `1.0` | 保留负号（`REACTOR=-1` 必须为 -1） |
| INF `REACTOR` | 无 `NO` 处理 | `NO` → `None`，否则解析为 `Option<f64>` |
| INF `{H,M,L}_PARAM` 第三字段 | 解析器忽略，改由电流分支数倒推 | 按规范读为 `bran_num` |

## 任务清单

### 阶段一：扩展共享设备模型
- [x] 1. `src/equipment.rs`：新增子结构 `AcvChn`（`ua/ub/uc/un/ul_idx`）、
      `AccBran`（`bran_idx, ia/ib/ic/in_idx, dir: i32, polarity: i32`）、`Impedance`（`r1,x1,r0,x0`）、
      `Capacitance`（按规范序 `c1,g1,c0,g0`）、`MutualInductance`（`idx,mr0,mx0`）、
      `Igap`（`zgap/zsgap_idx`）。
      `dir`/`polarity` 用 `i32` 缺省 `1`（规范 §1.4/§1.5：1=正方向/流入，-1=反方向）。
      DMF 的 `dir="POS"` 与 INF 的 `DIR=1` 是两种拼写，`AccBran` 同时保留
      `dir_raw: String`（DMF 原文，round-trip 用）与 `dir: i32`（归一化语义）。
      `location` / `tv_pos` / `object_type` 用 `String` 存原始拼写（`High` vs `high` 在两样本里
      不一致，转枚举会丢原文导致 round-trip 失真）。
- [x] 2. `Bus` 增补 `v_rtg_snd, tv_pos, is_location, sys_id, acv: AcvChn,
      ana_chns: Vec<usize>, sta_chns: Vec<usize>`。
- [x] 3. `Line` 增补 `a_rtg_snd, remote_id, remote_flag, differential_id, other_id, object_type,
      sys_id, reactor: Option<f64>（NO → None）, impedance, capacitance, mutual_inductance,
      currents: Vec<AccBran>, acv, acvs: Vec<AcvChn>（TV_CHNS_#n 多组）,
      oth_achns: Vec<usize>, ana_chns, sta_chns`。
- [x] 4. `Transformer` 增补 `winding_num, object_type, sys_id, capacity, ta_self_comp,
      oth_achns, ana_chns, sta_chns`；
      `TransformerWinding.wg` **`f64` → `String`**，增补 `acv, currents: Vec<AccBran>, igap,
      bran_num（来自 PARAM 第三字段）, ta_zs, ta_zs_gap, sta_chns`。
- [x] 5. `src/lib.rs` 导出新增公共类型。

### 阶段二：DMF 读写补全
- [x] 6. `xml_reader.rs`：改成显式的容器栈，处理 `ACVChn` / `ACC_Bran` / `RX` / `CG` / `MR` /
      `Igap` / `AnaChn` / `StaChn`，按"当前容器"（Bus / Line / Winding）挂载。
- [x] 7. 修 `TransformerWinding` 自闭合丢失：`Start` 分支加 `empty` 检查并立即收尾。
- [x] 8. 补 `Bus` / `Line` 的遗漏属性；`wg` 按字符串读取；`ACC_Bran.dir` 存原文并归一化。
- [x] 9. `xml_writer.rs`：按 Python 的属性顺序写出全部新字段与子元素；
      修正 `ACVChn` 元素名、`bran_idx` 属性名、补 `TransformerWinding.srcRef`、
      去掉 `Bus`/`Line` 开标签上的非法 `/`。每处修正加 `// 偏离 Python 基线：...` 注释。

### 阶段三：INF 设备节补全（严格按 PDF §1.4 / §1.5）
- [x] 10. `src/inf/builder.rs` 新增辅助函数：
      `parse_num_with_unit`（保留负号，剥离 `V`/`(km)`/`(MVA)`/`(Ω/km)` 等单位）、
      `parse_chn_list_with_dir`（逗号分隔；前若干为通道号、可选末位为 `1`/`-1` 方向符；
      **不能像 Python 那样用"严格递增"过滤，否则会吞掉方向符和重复通道号**）、
      `split_dev_id`（首个逗号切分，前段 idx、后段 name）、
      `parse_reactor`（`NO` → `None`）。
- [x] 11. 重写 `build_equipment_group`：
      - Bus：`DEV_ID/SYS_ID/RATED_VALUE/TV_RATIO/TV_CHNS/TV_POS`
      - Line：`DEV_ID/OTHER_ID/SYS_ID/OBJECT_TYPE/LENGTH/RX/CG/MRX/REACTOR/
        TA_CHNS/TA_CHNS_#2/TV_CHNS/TV_CHNS_#n/OTH_ACHNS/STATUS_CHNS`
      - Transformer：`DEV_ID/SYS_ID/OBJECT_TYPE/CAPACITY/TA_SELF_COMP/WINDING_NUM/
        {H,M,L}_PARAM/{H,M,L}_TV_CHNS/TA_Id_#1..#7/{H,M,L}_TA_ZS/_ZS_GAP/
        {H,M,L}_STATUS_CHNS/Z_STATUS_CHNS/OTH_STATUS_CHNS/OTH_ACHNS`
      - 绕组装配按 `WINDING_NUM`：`3`→高(#1,#2)+中(#3,#4)+低(#5,#6,#7)；
        `2`→高(#1,#2)+低(#5,#6,#7)，**跳过中压**；`1`（自耦变）→按 `TA_Idcw_#n` 另行处理，
        本轮先按两卷变同构降级并留 TODO 注释。
      - 每侧只在对应 `PARAM` 或 `TA_Id` 实际存在时建绕组，避免产出全零空绕组。
- [x] 12. `build_{bus,line,transformer}_section`：按规范 Key 与值格式写出（含单位后缀与方向符），
      `area` 改 `ZYHD`。
- [x] 13. `src/inf/section.rs`：`Other(String)` 保留原始大小写，匹配时才大小写不敏感，
      修掉 32 个节的改名问题。

### 阶段四：验证
- [x] 14. `src/dmf/` 与 `src/inf/` 内联单元测试：覆盖子元素解析、自闭合绕组、`wg` 字符串、
      带单位数值（含负数 `-1(Ω)`）、`REACTOR=NO`、`DEV_ID` 切分、`TV_RATIO` 大小写、
      `TA_CHNS` 带/不带 `DIR` 两种形态、`TA_Id_#N` 极性符号、`WINDING_NUM=2` 跳中压。
- [x] 15. 重写 `tests/dmf_tests.rs::test_dmf_to_equipment_group` 与
      `tests/inf_tests.rs::test_to_equipment_group`：断言**具体字段值**，不再只断言不 panic。
      DMF 断言 `binary_1999.dmf`：2 母线 / 3 线路 / 1 变压器 2 绕组、`LinLen=23.79`、
      `ACC_Bran` 与 `ACVChn` 索引、`ascii_1999.dmf`：5 母线 / 19 线路、`RX.r1=0.0469`。
      INF 断言 `binary_inf.inf`：7 母线（名字非空、`RATED_VALUE=220000`、`TV_CHNS=[46,47,48]`）、
      6 线路（`LENGTH=22.45`、`RX=[0.071,0.44,0.351,1.324]`、`REACTOR=None`、
      `TA_CHNS` 通道 `[41,42,43]` + `dir=1`）、
      1 变压器（`CAPACITY=180`、`WINDING_NUM=3`、3 绕组、
      高压绕组电流 `[1,2,3]` 且 `polarity=1`、中压 `[15,16,17]`、低压 `[29,30,31]`）。
- [x] 16. round-trip 测试：DMF 与 INF 各断言设备字段在 `parse → serialize → reparse` 后不变，
      INF 额外断言 32 个 `Other` 节名大小写不被改动。
- [x] 17. `cargo test` 全绿 + `cargo clippy` 无警告。

---

## Review（2026-08-14 完成）

### 结果
`cargo test` 63 项全绿，`cargo clippy --lib` 无警告。

设备识别从"只有名字和节序号"变为完整拓扑：

| 样本 | 母线 | 线路 | 变压器 |
|---|---|---|---|
| `binary_inf.inf` | 7（名称/额定电压/TV 通道/TV_POS 齐全） | 6（长度/RX/CG/MRX/REACTOR/TA 方向/TV/开关量齐全） | 1（三卷变，高/中/低三侧，接线组别 `Y`/`Y12`/`D11`） |
| `binary_1999.dmf` | 2 | 3 | 1（两卷变） |
| `ascii_1999.dmf` | 5 | 19 | 1（接线组别 `y0`/`yn0`） |

### 修正的缺陷
1. **INF 读错 Key**：`build_equipment_group` 用 DMF 的 `bus_name`/`line_name`/`trm_name`
   去读 INF 段，而 INF 用 `DEV_ID=idx,name`，所有字段落空。
2. **INF 设备段几乎全部未解析**：`RATED_VALUE`/`TV_CHNS`/`TV_POS`/`LENGTH`/`RX`/`CG`/
   `MRX`/`REACTOR`/`TA_CHNS`/`STATUS_CHNS`/`WINDING_NUM`/`{H,M,L}_PARAM`/`TA_Id_#N`/
   `{H,M,L}_TV_CHNS`/`CAPACITY` 现已全部解析。
3. **`TA_Id_#N` 侧映射**：`#1/#2`→高压，`#3/#4`→中压，`#5/#6/#7`→低压，每侧支持多路分支。
4. **变压器 TA 字段数**：`TA_Id_#N` 只有 `Ia,Ib,Ic,极性符号`（4 字段，**无零序**），
   与线路 `TA_CHNS` 的 5 字段不同，用两个独立解析器（Python 基线混用同一个，
   把极性符号误读成 `in_idx`）。
5. **不再吞掉方向标志**：Python `str2ids` 的"严格递增"过滤会让 `41,42,43,0,1`
   变成 `[41,42,43]`（`0` 被跳过、尾部 `1` 因不大于 `43` 被丢），方向整体丢失。
   Rust 按原文逐项解析。
6. **`CG` 位置序**：规范 §1.4 是 `C1,G1,C0,G0`，不是 `c1,c0,g1,g0`（Python 读错，
   会把零序电容与正序电导对调）。
7. **`REACTOR=NO`/负值** → `None`；`0` 保留为有效值。
8. **DMF 子元素归属**：`ACVChn`/`ACC_Bran`/`RX` 同时出现在 Bus、Line、
   TransformerWinding 下，改用显式容器栈按当前容器分派；未建模元素压入
   `Container::Ignored`，其子元素不再被误挂到外层设备上。
9. **DMF 自闭合绕组丢失**：`<TransformerWinding />` 不产生 End 事件，原实现只在
   End 分支收尾，自闭合绕组被静默丢弃。
10. **`wG` 类型错误**：原声明为 `f64` 并 `parse().unwrap_or(0.0)`，使
    `y0`/`yn0`/`Y12`/`D11` 等接线组别全部静默归零。改为 `String`。
11. **`ACC_Bran` 属性名**：读 `bran_idx` 写 `idx`，二次解析时分支号归零。统一为 `bran_idx`。
12. **DMF 写出缺陷**：`Bus`/`Line` 开标签带非法 `/` 却仍需承载子元素（子元素被整体丢弃）、
    `idx_rlt` 误写 `idx_rl`、`TransformerWinding.srcRef` 未写出。
13. **INF 节名大写化**：`SectionKind::Other` 存 `to_uppercase()` 结果，
    序列化把 `ZprdInfo`/`Channels_Group` 等 32 个节名改写成全大写，破坏 round-trip。
14. **设备段区域名**：`build_*_section` 写 `Public`，实际应为 `ZYHD`。

每处偏离 Python 基线的修正均在代码中以 `// 偏离 Python 基线：...` 注释记录。

### 测试
- 内联单元测试 13 项：数值单位剥离（含负号）、通道列表不做递增过滤、线路/变压器
  两套 TA 解析、`DEV_ID` 切分、`REACTOR`、`CG` 位置序、`_PARAM` 字符串代号、
  绕组布局与 `TA_Id` 槽位、缺侧不造空绕组、第二路分支、DMF 容器分派、
  自闭合绕组、未建模元素子元素隔离、节名大小写。
- 集成测试改为断言具体数值（原先只断言"不 pank"，正是缺陷长期未被发现的原因）。
- 新增 DMF / INF 设备段 round-trip 测试，逐字段比对 `parse → serialize → reparse`。

### 未纳入本轮（需另行确认）
- 规范 §1.6 发电机段 `[ZYHD POWER_#n]`、§1.7 励磁机段 `[ZYHD EXCITATION_#n]`。
- 自耦变专有的 `TA_Idcw_#1..#5` 公共绕组电流通道；当前 `WINDING_NUM=1`
  按两卷变（高/低）降级处理，代码中已留 TODO。
- DMF `SDL_*` 全部丢弃（按用户决定，与 Python 基线一致）。

---

## Review：六个版本号识别（2026-08-14 完成）

### 需求
录波文件版本包含 1991 / 1999 / 2001 / 2008 / 2013 / 2017；只有当头部为空时默认 1991。

### 结果
`cargo build` 通过，`cargo test` 70 项全绿（含新增 7 项），`cargo clippy --lib` 无警告。

### 修正的缺陷
1. **`Version` 枚举只有 `V1991` / `V1999`**：`src/cfg/channel.rs` 扩展为六值枚举
   （`V2001/V2008/V2013/V2017`），新增 `standard()`（对应标准名，与 Python
   `model/type/version.py` 一致）、`parse() -> Option<Version>`（trim 后匹配，未知返回 `None`）、
   `as_str()`。
2. **头部版本只匹配 `"1999"`**：`Header::from_line` 原先 `match ver_str { "1999" => V1999, _ => V1991 }`，
   `2001/2008/2013/2017` 的文件——包括真实样本 `tests/data/ascii_cff_2013.cff`
   （头部 `GHBZ,220kV线路故障,2013`）——全被静默判成 1991。改为
   `Version::parse(...).unwrap_or_default()`，头部无版本字段或无法识别时回落 1991
   （对齐 Python `Version.from_value(x, V1991)`，用户已确认此回滚策略）。
3. **INF `Revision_Year` 同样只认 1999**：`src/inf/builder.rs::build_config` 的 `InfFile → Config`
   转换用同一修复，INF 写回时保留原版本号。
4. **新增 `is_1999_or_later()` 判据**：1991 是唯一没有 CFG 可选尾部字段
   （`timemult` / 时间码 / 采样时间品质）的版本，1999+ 布局兼容；所有按版本分支的
   解析/序列化逻辑都应判断这个方法而非 `== V1999`。已 Grep 确认生产代码无残留
   `== V1999` 比较（仅剩的 `V1991`/`V1999` 字面量是 `Config::default()` 与 DFR 转换器的
   固定默认值，均为有意为之）。CFG 写出器按 `Option` 存在性而非版本号输出可选尾部行，
   因此写出端无需改动。

### 测试
- `src/cfg/mod.rs` 内联 5 项：六版本 `parse`/`as_str` 往返、`is_1999_or_later`、
  头部六版本识别与写回、头部空/缺版本/未知版本（`2020`、`abc`）回落 1991、2013 版 CFG
  含可选尾部字段（`UTC-8,UTC-8` + `0000`）的 round-trip。
- `src/inf/builder.rs` 内联 1 项：`Revision_Year` 六版本 round-trip，`"2020"`/`""` → `V1991`。
- `tests/cff_tests.rs`：`test_cff_to_config` 断言 `ascii_cff_2013.cff` 头部 `2013` 被识别为
  `V2013`（原会被静默判成 1991）；新增 `test_cff_version_survives_round_trip` 断言版本号
  写回不塌成 1991。

---

## Review：命名规范审查 + rustfmt 配置 + Segment 派生字段（2026-08-14 完成）

### 需求
1. 审查项目中残留的 Python 命名习惯，按 Rust 风格重命名（范围：公共 API + 内部命名，
   仓库零提交未发布，破坏性重命名无成本）。
2. 创建 `rustfmt.toml`（用户给定 22 项配置）并据此格式化项目。
3. 失效项注释掉并标注 nightly 要求。
4. `nominal_freq` 参数形同废弃 —— 按额定频率补齐派生字段。
5. `count` 与 `start_point` 一并补上。

### 结果
`cargo test` **78 项全绿**（lib 39 + 集成 39），`cargo clippy --all-targets` 无警告，
`cargo fmt --check` 退出码 0。

### 一、命名规范修正
| 原名 | 新名 | 依据 |
|---|---|---|
| `get_analog` / `get_status` | `analog` / `status` | Rust API C-GETTER：访问器不带 `get_` |
| `get_analog_samples` / `get_status_samples` | `analog_samples` / `status_samples` | 同上 |
| `n_analog` / `n_status` / `n_intervals` | `analog_count` / `status_count` / `interval_count` | `n_` 是 numpy 习惯；本 crate 已有 `analog_count` 先例 |
| `DatFile::from_ascii_str` | `from_ascii` | `&str` 形参已表达 "str" |
| `CffFile::from_comtrade` | `CffFile::encode` | 关联函数无 `self`，`to_*` 会与 `DatFile::to_bytes`（有 `self`）语义冲突 |
| `validate_shape` | `fit_to_config` | 该函数**会改数据**，`validate_*` 名不副实 |
| `post_process` | `apply_timemult` | 命名"做什么"而非"流水线第几步" |
| `data_type_str` | `data_type_text` | — |
| `sec_type` / `line_str` / `type_str` / `idx_str` | `section_type` / `text` / `type_name` | — |
| `|l|` / `let n` / `idx`（局部） | `|line|` / `sample_count` / `index` | — |

顺手清掉的死代码（非命名问题）：`src/inf/builder.rs` 的 `let _name = parts[2].to_string();`
（白做一次堆分配）、`src/dat/ascii.rs` 的 `let _n_analog`、`src/cff/section_splitter.rs` 的
`let _line_start`、`src/dat/binary.rs` 的空 `if _remainder != 0 {}` 块（体内只有注释）。

**故意不改**：`idx_org` / `au` / `bu` / `tv_pos` / `v_rtg` / `pwr_rtg` / `wg` / `sta_chns` /
`samp_rate` / `tmq_code` / `timemult` / `ia_idx` / `ua_idx` —— 这些是 IEEE C37.111 / INF /
DMF-XML 的**原文字段名**，序列化时原样写回，格式保真优先于展开可读性。
`idx`（约 198 处，Rust 可接受缩写）与固有方法 `from_str` / `to_string` 亦保持不变。

### 二、rustfmt 配置：22 项里 11 项在 stable 上是失效的
`cargo fmt` 刷屏报 `can't set X, unstable features are only available in nightly channel`。
另有 `impl_empty_lines` —— rustfmt **根本没有这个选项**（报 `Unknown configuration option`）。

处理：10 项有效项保留，11 项 nightly 项**注释掉并标 `[nightly]`**（留在文件里作为文档，
但不再让配置文件谎报行为），`impl_empty_lines` 移除并留注释说明其对应选项是
`blank_lines_upper_bound`（同样是 nightly）。文件头加说明：如需启用请 `cargo +nightly fmt`。

**重要副作用**：`imports_granularity` / `group_imports` / `wrap_comments` / `comment_width` /
`normalize_comments` 均**未生效** —— 导入只排序不合并不分组，注释永不重排。已写入 CLAUDE.md
以免后人误以为这些规则在跑。

另记：本代码库 `cargo fmt` 需**跑两遍**才收敛（`tests/` 里的长 `assert_eq!` 宏），已写入 CLAUDE.md。

### 三、`nominal_freq` 死参数：根因是模型缺字段，不是参数写错了
`recalculate_segments(timestamps_us, nominal_freq)` 的第二个参数从未被读取。表面像"忘了用"，
实际根因是 **Rust 的 `Segment` 比 Python 基线窄**：Python `segment.py` 有
`start_point` / `count` / `cycle_point_num` 三个 `| None` 字段，Rust 一个都没有，
派生值无处安放，所以参数只能烂在签名里。死参数是**症状**，不是病。

修法：
- `Segment` 增补三个派生字段，全为 `Option`（CFG 不携带这些值，解析出的段一律 `None`，
  对齐 Python 的 `default=None`）。
- 拆成两个构造器：`Segment::new(samp_rate, end_point)` 走文件解析路径
  （CFG / DFR / INF 三处调用点**零改动**），`Segment::derived(samp_rate, start_point,
  end_point, nominal_freq)` 走重算路径，一次性算出三个派生值。
- `cycle_point_num = samp_rate / freq`，`freq` 非法（`<= 0` / 非有限）时取
  `DEFAULT_NOMINAL_FREQ = 50.0`，对齐 Python `freq if freq else 50`。
- `count = end_point - start_point`（`saturating_sub`），半开区间 `[start_point, end_point)`。

**一处需要向用户澄清的地方**：用户说"按额定频率校准"，但查 Python 基线后确认
**`nominal_freq` 从不校准 `samp_rate`** —— 采样率完全由时间戳差值决定，额定频率只用来
派生每周波点数。已就此矛盾询问用户，用户选择对齐 Python 语义。没有照字面实现。

CFG round-trip 未受影响：`Sampling::to_cfg_text` 只写 `samp_rate` / `end_point`，
实测真实样本输出仍为 `"50\n1\n10000,45600"`，逐字节一致。

`src/exporters/json.rs` 未同步输出三个派生字段：JSON 导出的 `Config` 只来自 CFG 解析，
派生字段恒为 `None`，输出一串 `null` 只是噪声。

### 四、测试
`recalculate_segments` 此前**零测试**，正是这个遗漏能长期存活的原因。新增 8 项内联测试：
派生 `cycle_point_num` / 派生 `start_point`+`count` / `count` 恒等于 `end_point-start_point`
（含相邻段首尾相接、无空隙无重叠、末段终止于总点数的不变式）/ 遵循传入频率（60Hz）/
非法频率回落 50Hz / 采样率切换分段 / CFG 解析段派生字段为 `None` / 点数不足返回空。

**变异验证**（把 `derived` 改回不填派生字段，确认测试真的能抓到）：
`cycle_point_num` 轮 6 项测试中 4 项 FAILED；`start_point`/`count` 轮 2 项新测试
精确 FAILED、其余 6 项不受影响。两轮均恢复修复后复跑全绿。

### 附带修正
- `tests/dat_tests.rs` 两处 clippy `field_reassign_with_default` → 结构体字面量 + `..Default::default()`。
- `Segment::end_point` 文档措辞：原写"1-based"，与相邻 `start_point` 的"0-based 半开区间"
  看似矛盾。补注说明两种表述**取值相同**（1-based 末点序号 == 0-based 半开右端点），
  对齐 CFG `endsamp` 语义。
- CLAUDE.md 三处更新：新增 `## Formatting` 章节、修正 `validate_shape`/`get_analog_samples`
  等失效引用、新增 C-GETTER 与 `Segment` 双构造器约定。

---

# 待办：发布到 crates.io + 自动化 CI + 提交前完整测试流程

> 状态：**仅记录，尚未开工**。摸底于 2026-08-14 完成，下列问题均已实测确认，非推测。

## 摸底实测结论

| 检查项 | 结果 |
|---|---|
| `cargo test`（默认 feature） | 78 全绿 |
| `cargo clippy --all-targets` | 0 警告 |
| `cargo fmt --check` | 退出码 0 |
| `cargo build --examples` | 通过 |
| `cargo doc --no-deps` | **1 个 warning**（失效 intra-doc 链接） |
| `cargo test --features gbk-builtin` | **1 项 FAILED** |
| `cargo publish --dry-run` | 打包成功但**体积超限** |
| git 提交数 | **0**（无任何提交，无 remote） |

## P0 —— 阻断发布，必须先解决

### [x] B1. 包体积 62.3MiB / 压缩后 11.4MiB，超过 crates.io 10MiB 上限
`cargo package --list` 显示 60 个文件全部入包，其中 `tests/data/` 占 62MB：

| 文件 | 大小 |
|---|---|
| `ascii_1999.dat` | 47M |
| `binary_1999.dat` | 9.8M |
| `binary_inf.dat` | 4.1M |
| `ascii_1991.dat` | 583K |
| 其余 13 个 | 合计 < 700K |

`cargo publish --dry-run` 实测输出：`Packaged 60 files, 62.3MiB (11.4MiB compressed)`。
**超上限，会被 crates.io 拒绝。**

待定决策（需用户拍板）：
- **方案 A**：`Cargo.toml` 加 `exclude = ["tests/data/*.dat", "docs/", "tasks/", ...]`，
  发布包不含大 DAT。代价：`cargo test` 在**已发布包**中跑不起来（下游 `cargo package`
  验证阶段不跑集成测试，所以不影响发布本身）。
- **方案 B**：造小型精简 fixture（截取前 N 个采样点），大文件只留本地/CI，
  发布包带小 fixture。代价：需重算所有依赖具体数值的断言。
- **方案 C**：`include = [...]` 白名单，只发 `src/` + `README` + `LICENSE` + `Cargo.toml`。
  最干净，但发布包完全无测试。

倾向 **C + 白名单**：库的使用者不需要 62MB 录波样本；CI 从 git 仓库跑测试，不从发布包跑。
另需一并 `exclude` 掉 `CLAUDE.md` / `rustfmt.toml` / `tasks/`（当前实测**都进了包**，
其中 `tasks/todo.md`、`tasks/lessons.md` 是内部工作记录，不该随库分发）。

### [x] B2. 缺 LICENSE 文件
`Cargo.toml` 声明 `license = "MIT"`，但仓库根目录**没有 LICENSE 文件**（已 `ls` 确认）。
需补 MIT 全文，作者署名待用户确认（git config 显示 `张松贵`）。

### [x] B3. `Cargo.toml` 元数据不全
`cargo publish --dry-run` 实测警告：
`manifest has no documentation, homepage or repository`。

待补字段：`repository`（**需用户提供 git 托管地址，当前 `git remote -v` 为空**）、
`documentation`（一般填 `https://docs.rs/comtrade-io`）、`homepage`、
`keywords`（如 `comtrade` / `ieee-c37-111` / `power-system` / `fault-recording`，最多 5 个）、
`categories`（如 `parser-implementations` / `science`）、`readme = "README.md"`、
`authors`。

### [x] B4. crate 名 `comtrade-io` 在 crates.io 上是否已被占用 —— **已核实，可用**
本次已通过 crates.io sparse index（`https://index.crates.io/co/mt/comtrade-io`）查询，返回 404，
确认 `comtrade-io` **可用**。同名 Python 包已存在不影响 Rust 侧占用。
注意：`comtrade`（0.2.1）和 `comtrade-rs`（0.0.0）已被占用，但与本项目名不冲突。

## P1 —— 发布前应修

### [x] B5. `gbk-builtin` feature 下 `tests/inf_tests.rs::test_to_equipment_group` 失败
实测报错：
```
assertion `left == right` failed
  left: "\u{fffd}\u{fffd}A\u{fffd}"
 right: "主变A套"
  at tests\inf_tests.rs:134:5
```
这是 CLAUDE.md 已记录的已知缺陷（内置 GBK 编解码 CJK 区间返回 `U+FFFD`）的直接后果，
不是新 bug。但**发布一个开启后必然测试失败的 feature 是有问题的**。

三个选项：
- 给该断言加 `#[cfg(not(feature = "gbk-builtin"))]`，并在 feature 文档里明确"不支持 CJK"；
- 把 `gbk-builtin` 标为实验性 / 从发布中移除，等 GBK 表补全再放出；
- 补全内置 GBK 映射表（工作量大，且 `encoding_rs` 已能胜任 —— 收益可疑）。

倾向**第一个**：feature 的定位本就是"给 ASCII 为主的场景去掉 `encoding_rs` 依赖"，
让测试如实反映这个边界，而不是假装它能处理中文。

### [x] B6. `cargo doc` 有 1 个 warning：失效 intra-doc 链接
`src/dfr/binary.rs:16` —— 注释 `/// 从 [Data]\r\n 之后的原始字节切分` 里的 `[Data]`
被 rustdoc 当成条目链接，实际是 DFR 文件里的字面标记。修法：转义成 `\[Data\]` 或改用
反引号 `` `[Data]` ``。docs.rs 会显示这个 warning，发布前清掉。

### [x] B7. MSRV 1.75 声明未经验证
`Cargo.toml` 写 `rust-version = "1.75"`，但**从未用 1.75 实际编译过**（本机是 1.97.1）。
代码里已在用 let-else（`src/inf/builder.rs:1023`、`1029`）—— let-else 是 1.65 稳定的，
这条没问题，但**其余 API 未系统核查**。CI 应加一个 MSRV job 真跑一遍，
否则这个声明就是空头承诺。

## P2 —— 质量提升，不阻断发布

### [x] B8. 232 处缺失文档（`RUSTFLAGS="-W missing_docs"` 实测）
分布：结构体字段 186、枚举变体 39、方法 4、关联函数 2、常量 1。
按文件：

| 文件 | 处数 |
|---|---|
| `src/equipment.rs` | 62 |
| `src/cfg/channel.rs` | 50 |
| `src/error.rs` | 22 |
| `src/cfg/mod.rs` | 20 |
| `src/inf/section.rs` | 16 |
| `src/comtrade.rs` | 16 |
| 其余 11 个文件 | 合计 46 |

注意：`missing_docs` 目前**未启用**（`src/lib.rs` 无 `#![warn(missing_docs)]`），
所以这 232 处不会在常规构建中报警。作为公共库，字段级文档会直接体现在 docs.rs 上。
建议分批补完后再加 `#![warn(missing_docs)]` 锁住，避免一次性加导致 232 条噪声。

### [x] B9. 未跑过 `cargo publish --dry-run` 之外的打包健全性检查
待补：`cargo package` 解包后独立编译（dry-run 已含此步且通过）、
`--no-default-features` 构建、feature 组合矩阵。

## 自动化 CI

### [x] C1. 建 `.github/workflows/ci.yml`（仓库当前**无 `.github/` 目录**）
建议 job 矩阵：

| Job | 内容 | 阻断 |
|---|---|---|
| `fmt` | `cargo fmt --check` | 是 |
| `clippy` | `cargo clippy --all-targets -- -D warnings` | 是 |
| `test` | `cargo test` on ubuntu / windows（本项目有 GBK + 换行符敏感逻辑，**双平台必须都跑**） | 是 |
| `test-features` | `cargo test --features gbk-builtin`（需先解决 B5） | 是 |
| `no-default-features` | `cargo build --no-default-features` | 是 |
| `msrv` | `cargo build` on 1.75（验证 B7） | 是 |
| `doc` | `cargo doc --no-deps` + `RUSTDOCFLAGS="-D warnings"`（需先解决 B6） | 是 |
| `package` | `cargo publish --dry-run`（守住 B1 体积回归） | 是 |

注意事项：
- 62MB fixture 会拖慢 checkout —— 考虑是否值得引入 Git LFS，或 CI 只跑轻量 fixture。
- 缓存 `~/.cargo` 与 `target/` 以控制时长。
- Windows runner 上注意 `newline_style = "Unix"` 与 git `core.autocrlf` 的交互
  （本地 `git diff` 已在刷 `LF will be replaced by CRLF` 警告，需确认 CI 上不会因此
  导致 `fmt --check` 假失败）。

### [x] C2. 建 `release.yml`：打 tag 时自动 `cargo publish`
需要 `CARGO_REGISTRY_TOKEN` secret。建议加 `environment` 保护 + 手动 approve，
避免误触发发布（发布到 crates.io **不可撤销**,只能 yank)。

### [x] C3. 加 `dependabot.yml`
只有 `encoding_rs` 一个运行时依赖，收益有限，但 GitHub Actions 的版本更新值得自动化。

## 提交前完整测试流程

### [x] D1. 定义"完整测试"清单并落地成可执行入口
当前无任何提交（`git log` 实测：`your current branch 'master' does not have any commits yet`），
首次提交前应过一遍完整门禁。建议顺序（快的先跑，快速失败）：

```
1. cargo fmt --check                              # 注意：本库需跑两遍 cargo fmt 才收敛
2. cargo clippy --all-targets -- -D warnings
3. cargo build --no-default-features
4. cargo test                                     # 78 项
5. cargo test --features gbk-builtin              # 需先解决 B5
6. cargo doc --no-deps  (RUSTDOCFLAGS="-D warnings")
7. cargo build --examples
8. cargo publish --dry-run                        # 守体积
```

### [x] D2. 选一个落地形式
- **`Makefile` / `justfile`** 里定义 `verify` 目标 —— 简单，本地 CI 共用同一份定义；
- **`cargo xtask`** —— 无需额外工具链，但要写 Rust；
- **git pre-commit hook** —— 自动，但完整跑约 15s+（集成测试有两个 7s 的用例），
  可能被 `--no-verify` 绕过。

已选 **bash 脚本 `scripts/verify.sh`**：Windows 本机 Git Bash 已有、CI ubuntu/windows runner 均带 bash，
无需装 `just`。9 步与 ci.yml 各 job 复用同一组命令，`bash scripts/verify.sh` 实测全绿。

### [x] D3. 首次提交前的额外确认
- `tasks/` 当前**未纳入 git**（`.gitignore` 没忽略它，但也没 `git add`）——
  **决定：提交**。todo.md/lessons.md 是工作流记录，Cargo.toml `exclude` 已保证不进发布包。
- `docs/` 已被 `.gitignore` 忽略，但 CLAUDE.md 称其为"authoritative spec"。
  **决定：保持忽略**（含 216KB PDF，疑有版权；两份设计报告如需入库可后续单列规则）。
- 62MB fixture（含 47MB `ascii_1999.dat`）：**决定：全部提交**（用户确认；47MB < GitHub 100MB 上限）。
  所有 fixture 当前已在 git index 中（状态 `A`），无需额外操作。

---

## Review：CFF 乱码修复 + 发电机/励磁机段 + 发布打包（2026-08-15 完成）

### 需求
1. CFF 读取后出现文件乱码 —— 修复。
2. INF 和 DMF 均增加发电机段（§1.6）、励磁机段（§1.7）识别 —— 完整实现。
3. 发布打包：examples/docs/tests 不打包（解决 B1）；增加 LICENSE（MIT，署名张松贵 sanguine）；
   补全 Cargo.toml 元数据格式（具体值随后替换）；确认 crate 名在 crates.io 唯一。

### 结果
- **CFF 乱码根因**：`section_splitter.rs` 无条件 GBK 解码，UTF-8 fixture 的中文被误读。
  修复为按段 UTF-8 优先探测、失败回退 GBK（复用 `encoding::decode`）。
  附带修复 BINARY DAT 段被 `trim_bytes` 吃掉首尾合法采样字节的独立缺陷。
- **发电机/励磁机**：`equipment.rs` 新增 `Generator`/`Exciter` 及 `BranchNum`/`Ufe`/
  `SyncReactance`/`UfeChns`/`UnChns` 子结构体；INF 层（SectionKind + 解析 + 序列化）、
  DMF 层（`<scl:Generator>`/`<scl:Exciter>` 读写）全链路打通，按规范 §1.6/§1.7 全字段实现。
- **发布打包**：`exclude` 剔除 tests/examples/docs/tasks/CLAUDE.md/rustfmt.toml，
  发布包 62.3MiB → 244.6KiB（60.6KiB 压缩）。LICENSE 已建。Cargo.toml 补全
  authors/repository(占位)/documentation/homepage/keywords/categories/readme/exclude。
  crate 名 `comtrade-io` 经 sparse index 查询确认可用（404）。

### 修正的缺陷
- `section_splitter::decode_gbk` → `decode_text`（UTF-8 探测优先）。
- `dat_bytes` 存未裁剪的 `section_bytes`，BINARY 采样字节逐字保留。
- `SectionKind` 新增 `Generator`/`Exciter`，三处映射（枚举/to_text/parse_section_header）同步。
- INF `build_equipment_group` 加两 arm；新增 `build_generator_section`/`build_exciter_section`。
- DMF `Container`/`parse_dmf`/`write_dmf`/`DmfFile` 全部扩展。

### 测试
- 新增 CFF 中文通道名断言（`test_cff_chinese_channel_name`）。
- 新增 BINARY DAT 不裁剪内联测试（`test_dat_bytes_not_trimmed`）。
- 新增 UTF-8 CFG 段不被 GBK 误读内联测试。
- 新增发电机/励磁机 INF/DMF round-trip 测试（合成 fixture）。
- 全量 `cargo test` 全绿（详见验证任务记录）。

### 未纳入本轮
- B5（gbk-builtin feature 下 INF 测试失败）—— 既有缺陷，本轮未触碰。
- B6（cargo doc warning `[Data]`）—— 后续。
- B7（MSRV 1.75 未验证）—— 后续 CI。
- B8（232 处 missing_docs）—— 后续分批补。
- 自耦变 `TA_Idcw_#1..#5` —— 用户决定搁置。
- C1/C2/C3（CI workflows）—— 后续。
- D1/D2/D3（提交前测试流程落地）—— 后续。
- `Cargo.toml` 中 `repository`/`homepage` 为占位符 `<TODO: ...>`，用户随后替换。

### 验证（2026-08-15 全绿）
- `cargo fmt`（两遍收敛）+ `cargo fmt --check` → exit 0 ✓
- `cargo clippy --all-targets -- -D warnings` → 0 警告 ✓
- `cargo test` → 89 项全通过（47 lib + 7 cfg + 5 dat + 3 cff + 8 convert + 7 dmf + 11 inf + 1 doc）✓
- `cargo build --features gbk-builtin` → 编译通过（B5 既有测试失败不变，本轮未触碰）✓
- `cargo build --examples` → 通过 ✓
- `cargo package --list --allow-dirty` → exclude 生效，tests/examples/docs/tasks/CLAUDE.md/rustfmt.toml 均不在列表 ✓
- `cargo publish --dry-run --allow-dirty` → 288.3KiB（68.4KiB 压缩），远低于 10MiB 上限 ✓

---

## Review：逐项修复发布前待办 B5–B9 + CI + 提交前流程（2026-08-15 完成）

### 需求
1. 修复 `tasks/todo.md` 记录的发布前 P1/P2 待办（B5/B6/B7/B8/B9）。
2. 落地自动化 CI（C1/C2/C3）与提交前完整测试流程（D1/D2/D3）。
3. 敲定首次提交前决策（D3：62MB fixture / tasks/ / docs/）。

### 结果
- **B6**：`[Data]` doc 注释实际 **3 处**（`dfr/binary.rs:16`、`dfr/mod.rs:3`、`dfr/mod.rs:123`，
  todo 只记了 1 处）——全部包反引号。`cargo doc --no-deps` 0 warning，
  `RUSTDOCFLAGS="-D warnings"` 通过。
- **B5**：`tests/inf_tests.rs` 中文名断言包 `#[cfg(not(feature = "gbk-builtin"))]`，
  README feature 说明补"内置 codec 不支持 CJK"。`cargo test --features gbk-builtin` 89 项全绿。
- **B9**：`encoding_rs` 非 optional，README 原措辞"不再依赖"误导——修正为"切换 codec，
  依赖树仍含 encoding_rs"。`--no-default-features`、`--no-default-features --features gbk-builtin`
  构建通过；`.github/`、`scripts/` 补进 exclude（防进发布包），package --list 复验 34 文件无混入。
- **B7**：`rustup toolchain install 1.75.0` 实测 `cargo +1.75.0 build` 与 `cargo +1.75.0 test --lib`
  （47 项）全绿。**MSRV 1.75 声明真实有效**（此前从未验证）。
  `encoding_rs 0.8.35` MSRV 兼容。
- **B8**：实测 missing_docs 分布 **239 处**（比 todo 记录的 232 略多，因新增 Generator/Exciter 字段），
  3 个并行子代理按文件分派补齐（A: equipment+cfg/channel 119、B: error+cfg/mod+inf/section 58、
  C: 其余 12 文件 54），主线程复核全库 0 warning；`src/lib.rs` 加 `#![warn(missing_docs)]` 锁住防回归。
  文档语义依据设计报告 / Python 基线 / DMF XML 属性名，未编造。
- **C1/C2/C3**：`.github/workflows/ci.yml`（fmt/clippy/test 双平台/test-features/no-default-features/
  msrv/doc/package 矩阵）、`release.yml`（tag 触发 + environment 保护 + 手动 approve）、
  `dependabot.yml`（cargo + github-actions 每周）。
- **D1/D2**：`scripts/verify.sh` 落地（9 步完整门禁，含打包内容防混入检查），`bash scripts/verify.sh` 实测全绿。
- **D3**：首次提交前决策敲定——62MB fixture **全部提交**（用户确认）、`tasks/` **提交**、
  `docs/` **保持忽略**（PDF 疑有版权）。

### 验证
- `bash scripts/verify.sh` 全 9 步通过（fmt/clippy/test/gbk-builtin/doc/examples/package 体积/打包内容）✅
- `cargo +1.75.0 test --lib` → 47 项全绿（B7）✅
- `cargo publish --dry-run` → 298.2KiB（70.9KiB 压缩）< 10MiB ✅

### 待用户处理
- `Cargo.toml`：`documentation` 现填成 GitHub 仓库地址（惯例应为 `https://docs.rs/comtrade-io`），
  `homepage` 填成了中文描述而非 URL——两处建议修正。
- 首次提交（单一 initial commit，含全部暂存 + 本轮改动）。
- `release.yml` 需在 GitHub 仓库配置 `CARGO_REGISTRY_TOKEN` secret + `crates-io` environment。
