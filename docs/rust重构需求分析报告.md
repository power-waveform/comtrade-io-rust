# comtrade-io Rust 重构 —— 需求分析报告

- 版本：v1.1（2026-08-14 修订：CFF/DFR 纳入一期，新增 JSON/CSV 导出，HDR 改为预留）
- 日期：2026-08-14
- 基线：`comtrade-io`（Python，当前仓库 `develop` 分支，commit `8523ca1` 之后状态）
- 目标产物：`comtrade-io-rust`（Rust 库，crate 名建议 `comtrade-io`）

---

## 1. 项目背景与目标

### 1.1 背景

现有 Python 库 `comtrade-io` 实现了 COMTRADE（IEEE C37.111）故障录波文件族的解析与导出：

- 多文件格式：`*.cfg` / `*.dat` / `*.hdr` / `*.inf` / `*.dmf`
- 单文件格式：`*.cff`（CFG+DAT+INF+HDR 复合）、`*.dfr`
- 导出：JSON、CSV、CFF、多文件回写

Python 实现依赖 pandas/numpy/pydantic/loguru/encoding 探测，启动与解析开销大，且难以嵌入高性能服务、边缘网关与其他语言运行时。

### 1.2 重构目标

1. **用 Rust 重新实现 CFG / DAT / INF / DMF 四种多文件的读写，以及 CFF / DFR 两种单文件格式的读取**（本次范围，见 §3.1）。
2. **支持导出 JSON 与 CSV**（含 CFF 单文件导出），对齐 Python `save_comtrade(format=...)` 能力。
3. 提供稳定、符合 Rust 习惯的公共 API，供其他模块（上层分析、可视化、转换服务）以 crate 依赖方式使用。
4. **不是逐行翻译**：利用 Rust 的所有权、类型系统、`Result` 错误传播、零拷贝解析、迭代器组合等特性重新设计内部结构。
5. **尽量避免第三方库**：CFG/INF/DAT/CFF/DFR 解析与 JSON/CSV 生成全部手写；仅在"手写成本显著高于收益且风险大"的点位允许极少量依赖（决策见设计报告 §14）。
6. 模块划分合理：格式解析、数据模型、编码、时间、错误、IO、导出分层清晰，单一职责。

### 1.3 非目标（Out of Scope）

| 项 | 说明 |
|---|---|
| HDR 文件解析/生成 | **格式不固定，一期仅预留类型与 API 位，不实现读写逻辑**（见 FR-HDR） |
| DFR 写出 | Python 亦只读；一期保持只读，写出留二期 |
| 通道智能识别（`channel_recognizer`） | Python 中基于名称启发式推断 type/flag，属业务增强，一期不迁移 |
| `CfgToEquipment` 设备拓扑自动生成 | 依赖通道识别，一期不迁移 |
| DataFrame 语义（pandas） | 以列式 `Vec`/`Box<[T]>` 数组替代，见 FR-DAT-6 |

---

## 2. 现状分析（Python 基线行为盘点）

> 本节是 Rust 版的行为对标依据，所有条目均给出 Python 源码位置。

### 2.1 入口与文件组定位

- 公共入口：`Comtrade.from_file(file_name)` → `ComtradeFile.from_path()`（[comtrade_file.py](file:///d:/codeArea/comtrade/comtrade-io/src/comtrade_io/parser/comtrade_file.py#L36-L87)）。
- `from_path` 规则：
  - 允许后缀（**大小写不敏感**匹配，但生成兄弟文件时保持输入的大小写风格）：`.cfg .dat .cff .dmf .hdr .inf .dfr`。
  - `.cff` / `.dfr` 为单文件模式，只记录自身路径。
  - 其余后缀：以输入文件的 `parent + stem` 拼出全部兄弟文件路径，逐个用 `FilePath` 校验存在性与可读性（不存在的记为禁用）。
- `from_file` 流程：读 CFG → 读 DMF（失败则回退 INF 构建设备组）→ 读 DAT → 组装 `Comtrade`（[comtrade_file.py](file:///d:/codeArea/comtrade/comtrade-io/src/comtrade_io/parser/comtrade_file.py#L89-L126)）。
- **HDR 在 Python 中没有独立解析器**：`hdr_path` 仅是路径占位；HDR 文本只在 CFF 分段（[section_splitter.py](file:///d:/codeArea/comtrade/comtrade-io/src/comtrade_io/parser/cff/section_splitter.py#L20-L51)）中作为原始文本出现。

### 2.2 Comtrade 主模型

[comtrade.py](file:///d:/codeArea/comtrade/comtrade-io/src/comtrade_io/model/comtrade.py#L31-L64)：

- 字段：`config: Configure`、`data: pd.DataFrame | None`、`buses/lines/transformers`。
- 兼容属性委托到 `config.description.*`：`analogs/statuses/channel_num/sampling/start_time/fault_time/data_type/timemult/header/time_info/sampling_time_quality`。
- 数据访问：`get_data()`、`get_analog_channel(i)`、`get_status_channel(i)`（按列取数并挂到通道对象的 `data` 字段）。
- 设备查询：`get_bus/get_line/get_transformer(name)`（info 版不带数据）。
- 分析能力：`get_changed_statuses()` —— 向量化检测数字量变位，生成 `StatusChangeRecord(sample_point, timestamp, state)` 列表（[comtrade.py](file:///d:/codeArea/comtrade/comtrade-io/src/comtrade_io/model/comtrade.py#L262-L327)）。
- 导出：`to_cfg()/to_dmf()/to_inf()` 文本生成；`write_cfg/write_dmf/write_inf` 落盘（**CFG/INF 用 GBK 编码写出，DMF 用 UTF-8**）；`save_comtrade(format=multi_file|json|csv|cff, data_format=BINARY|...)`。

### 2.3 CFG 文件（读 + 写）

解析：[cfg.py](file:///d:/codeArea/comtrade/comtrade-io/src/comtrade_io/parser/cfg/cfg.py#L36-L149)；序列化：[configure.py `__str__`](file:///d:/codeArea/comtrade/comtrade-io/src/comtrade_io/model/configure/configure.py#L33-L52)。

逐行格式（行号基于非空行序列）：

| 行 | 内容 | 字段 |
|---|---|---|
| 1 | 头部 | `station,recorder,version(1991/1999)` |
| 2 | 通道数 | `total,nA,nD`（如 `12,6A,6D`） |
| 3.. | 模拟量通道 ×nA | `an,ch_id,ph,ccbm,uu,a,b,skew,min,max,PS,SS,P/S`（13 字段） |
| .. | 状态量通道 ×nD | `dn,ch_id,ph,ccbm,y`（5 字段） |
| +1 | 采样频率 | `freq`（Hz，浮点） |
| +1 | 采样段数 | `nsamples` |
| +n | 采样段 ×nsamples | `samp_rates,nsamp`（段结束点累计） |
| +1 | 起始时间 | `dd/mm/yyyy,hh:mm:ss.xxxxxx` |
| +1 | 触发时间 | 同上 |
| +1 | 数据格式 | `ASCII/BINARY/BINARY32/FLOAT32`（可带尾逗号） |
| 可选 | 时标倍率 | `timemult`（浮点，默认 1.0） |
| 可选 | 时间信息 | `time_code,local_code`（1999 版） |
| 可选 | 采样时间品质 | `tmq_code`（1999 版） |

关键行为：

- **编码**：先按 GBK 读取（`errors="replace"`）；若内容含 `U+FFFD` 替换符则回退 UTF-8 严格读取（[cfg.py](file:///d:/codeArea/comtrade/comtrade-io/src/comtrade_io/parser/cfg/cfg.py#L172-L179)）。
- **容错**：行数不足抛 `ValueError`；模拟量 `a/b/skew/min/max` 数值非法时 `parse_float` 返回默认值；通道数 `total != nA+nD` 不强制报错（Python 按 nA/nD 实际解析）。
- 通道模型扩展字段（非 CFG 原生，来自 INF/DMF 合并）：`freq/au/bu/type/flag/idx_org/reference/equipment_no`（[analog.py](file:///d:/codeArea/comtrade/comtrade-io/src/comtrade_io/model/channel/analog.py#L42-L67)）。
- 时间解析支持 15 种格式按优先级尝试，含两位年份、欧美日月序、ISO 格式；微秒补零截断；2 月 29 日非法时降级为 28 日（[data_time_parser.py](file:///d:/codeArea/comtrade/comtrade-io/src/comtrade_io/parser/description/data_time_parser.py#L9-L51)）。
- 写出格式：`%m/%d/%Y,%H:%M:%S.%f`（美式月/日）。

### 2.4 DAT 文件（读 + 写）

[dat.py](file:///d:/codeArea/comtrade/comtrade-io/src/comtrade_io/parser/dat/dat.py)：

- **ASCII**：CSV 无表头，列 = `index,timestamp_us,analog...,status...`；编码探测顺序 `utf-8 → gbk → latin-1`；空值/NA 填 0；模拟量应用 `value = raw * multiplier + offset`，再按 `analog_precision`（默认 3，仅 [1,6] 生效）四舍五入。
- **BINARY / BINARY32 / FLOAT32**：小端记录布局 `index:<i4, timestamp:<i4, analog:<i2×nA（BINARY32/FLOAT32 为 <i4）, status:<u2×ceil(nD/16)>`；状态位按字内小端位序展开，截取前 nD 位；字节数不足整记录时按可解析条数读取并告警。
- **timemult**：在 `_post_process` 中乘到时间戳列（采样率重算之后，避免重复乘）。
- **采样段重算**：按时间戳差分检测 >5% 的采样率切换点，重算分段（samp、start/end_point、count、cycle_point_num）。
- **形状校验**：行数多于期望则截断；列数少于 `nA+2` 返回空；列数不足但 ≥`nA+2` 则丢弃多余数字量并补零列。
- **写出**：`write()` 支持 ASCII（工程值反算原始整数 `(v-offset)/multiplier` 取整，全列转 int64）与二进制（BINARY/BINARY32）；可写文件或 BytesIO。

### 2.5 INF 文件（读；写出来自 Comtrade.to_inf）

- INI 风格：节头 `[Area Type_#N]`（如 `[Public Analog_Channel_#1]`、`[ZYHD Analog_Channels_Parameter]`），节内 `Key=Value`。
- 节类型映射（[text_splitter.py](file:///d:/codeArea/comtrade/comtrade-io/src/comtrade_io/parser/inf/text_splitter.py#L10-L21)）：`FILE_DESCRIPTION`、`ANALOG_CHANNEL(S)`、`STATUS_CHANNEL(S)`、`ANALOG_CHANNELS_PARAMETER`、`STATUS_CHANNELS_PARAMETER`、`BUS`、`LINE`、`TRANSFORMER`。
- File_Description 关键 Key：`Station_Name`、`Recording_Device_ID`、`Revision_Year`、`Total_Channel_Count`、`Analog_Channel_Count`、`Status_Channel_Count`、`Line_Frequency`、`Sample_Rate_Count`、`Sample_Rate_#N`、`End_Sample_Rate_#N`、`File_Start_Time`、`Trigger_Time`、`File_Type`、`Time_Multiplier`。
- Analog_Channel 关键 Key：`Channel_ID`、`Phase_ID`、`Monitored_Component`、`Channel_Units`、`Channel_Multiplier`、`Channel_Offset`、`Channel_Skew`、`Range_Minimum_Limit_Value`、`Range_Maximum_Limit_Value`、`Channel_Ratio_Primary`、`Channel_Ratio_Secondary`、`Data_Primary_Secondary`。
- Status_Channel 关键 Key：`Channel_ID`、`Phase_ID`、`Monitored_Component`、`Normal_State`。
- 参数段 `CHNL_INFO_#N`：
  - 模拟量 11 字段：`idx_cfg, idx_org, name, type(flag), freq, t1(primary), unit, t2(secondary), unit2, ad(au), bd(bu)`；
  - 状态量 ≥5 字段：`idx_cfg, idx_org, name, level(type), flag_name, equipment_no`。
- 编码：**GBK 优先**，失败回退 UTF-8（[inf.py](file:///d:/codeArea/comtrade/comtrade-io/src/comtrade_io/parser/inf/inf.py#L16-L29)）。
- 双出口：`to_configure()`（构建/更新 Configure）与 `to_equipment_group()`（Bus/Line/Transformer 拓扑）。
- 写出（`Comtrade.to_inf`）：File_Description + 通道节 + `[ZYHD Analog_Channels_Parameter]` / `[ZYHD Status_Channels_Parameter]` 参数段 + 设备节；**GBK 落盘**。

### 2.6 DMF 文件（读 + 写）

- XML，根 `<scl:ComtradeModel>`，命名空间 `scl=http://www.iec.ch/61850/2003/SCL`，属性 `station_name/version/reference/rec_dev_name`。
- 子元素：
  - `AnalogChannel`：`idx_cfg, idx_org, type, flag, freq, au, bu, sIUnit, multiplier, primary, secondary, ps, idx_rlt, ph`；
  - `StatusChannel`：`idx_cfg, idx_org, type, flag, contact, srcRef`；
  - `Bus`：`idx, bus_name, srcRef, VRtg, VRtgSnd, VRtgSnd_Pos, bus_uuid` + 子元素 `ACVChn(ua/ub/uc/un/ul_idx)`、`SDL_Protect`、`SDL_Breaker`；
  - `Line`：`idx, line_name, bus_ID, srcRef, VRtg, ARtg, ARtgSnd, LinLen, bran_num, line_uuid, remote_ID, remote_Flag, differential_ID` + 子元素 `RX(r1,x1,r0,x0)`、`CG(c1,c0,g1,g0)`、`PX(px,px0)`、`MR(idx,mr0,mx0)`、`ACC_Bran(bran_idx,ia/ib/ic/in_idx,dir)`、`AnaChn(idx_cfg)`、`StaChn(idx_cfg)`、`DifferentialCurrent(ua/ub/uc_idx)`；
  - `Transformer`：`idx, trm_name, srcRef, pwrRtg, transformer_uuid` + `TransformerWinding(location, srcRef, VRtg, ARtg, bran_num, bus_ID, wG)`（含 `ACVChn/ACC_Bran/Igap(zgap_idx,zsgap_idx)`）。
- 命名空间容错：前缀不固定（scl/ns/无），按 URI 或本地名匹配（[dmf_element.py](file:///d:/codeArea/comtrade/comtrade-io/src/comtrade_io/parser/dmf/dmf_element.py#L38-L46)）。
- 写出：`Comtrade.to_dmf()` 字符串拼接，UTF-8 落盘。

### 2.7 CFF 单文件（读 + 写）

解析：[cff.py](file:///d:/codeArea/comtrade/comtrade-io/src/comtrade_io/parser/cff/cff.py) + [section_splitter.py](file:///d:/codeArea/comtrade/comtrade-io/src/comtrade_io/parser/cff/section_splitter.py#L20-L51)。

- **字节级段切分**：正则 `^--{1,2}\s*file\s+type\s*:?\s+(\w+)(?:\s+[^-]*)?\s*---`（多行、忽略大小写）在原始字节上定位段标记（标记均为 ASCII，避免中文字节偏移），段类型取 `CFG/DAT/INF/HDR`；每段取当前标记结束到下一标记开始之间的字节，`strip` 后按 **GBK（errors="replace"）** 解码；DAT 段同时保留原始字节（二进制 DAT 不能走文本解码）。
- 段消费（[cff.py](file:///d:/codeArea/comtrade/comtrade-io/src/comtrade_io/parser/cff/cff.py#L43-L63)）：
  - CFG 段 → `CfgSection.from_str`（等价标准 CFG 解析）；
  - DAT 段 → 按 `config.data_type`：ASCII 走 `DatSection.from_str`（先清洗控制字符，保留 `\n\r\t`），其余走 `DatSection.from_bytes`；
  - INF 段 → `InfSection.from_str`；HDR 段仅保留原文。
- 产物：`CffFile`（sections + file_path），经 `to_configure()/to_data_content()/to_information()` 汇入 `Comtrade`。
- 写出（[cff_exporter.py](file:///d:/codeArea/comtrade/comtrade-io/src/comtrade_io/exporters/cff_exporter.py#L14-L45)）：
  - 段序：`--- file type CFG ---` + CFG 文本 →（有 INF 时）`--- file type INF ---` + INF 文本 → `--- file type DAT ---` + DAT 内容；
  - ASCII DAT：DataFrame 无表头 CSV；二进制 DAT：按 data_format 打包字节；
  - 文本部分 **GBK 编码（errors="ignore"）** 落盘，二进制 DAT 段直接追加字节。
- 测试样本：`tests/data/ascii_cff_2013.cff`（ASCII DAT 的 CFF）。

### 2.8 DFR 单文件（只读）

解析：[dfr.py](file:///d:/codeArea/comtrade/comtrade-io/src/comtrade_io/parser/dfr/dfr.py) + [wndr_section.py](file:///d:/codeArea/comtrade/comtrade-io/src/comtrade_io/parser/dfr/wndr_section.py) + [binary_section.py](file:///d:/codeArea/comtrade/comtrade-io/src/comtrade_io/parser/dfr/binary_section.py) + [converter.py](file:///d:/codeArea/comtrade/comtrade-io/src/comtrade_io/parser/dfr/converter.py)。

- **文件布局**：前 4088 字节为 WNDR 文本头（**cp1251 西里尔编码**），随后以 `[Data]\r\n` 标记定位二进制数据区；找不到标记时回退偏移 4088。
- **WNDR 文本头**（[wndr_section.py](file:///d:/codeArea/comtrade/comtrade-io/src/comtrade_io/parser/dfr/wndr_section.py#L96-L170)）：行 1 魔数 `[WNDR]`；行 2 站名；行 3 `total,xxA,xxD` 通道计数；随后 nA 行模拟通道（CSV 字段：idx,name,相别码,监测回路,满度值,转换系数,单位,相别,...,ch_type(第13列),rated_primary(第14列)）、nD 行状态通道（idx,name）；尾部三行：full_scale（默认 32768）、samples_per_cycle（默认 24）、total_samples。
- **二进制数据区**（[binary_section.py](file:///d:/codeArea/comtrade/comtrade-io/src/comtrade_io/parser/dfr/binary_section.py#L34-L63)）：`[Data]\r\n` 后先读设备 ID（连续字母数字，≤32 字节），按设备注册表（`KNOWN_DEVICES`，如 ЭКРА БЭ2704 的 `2704V042/2704V072`，header_size=248）跳过设备私有头，其余为采样帧；帧结构 = nA×i16 LE + ceil(nD/16)×u16 LE，**无序号与时间戳列**。
- **转换规则**（[converter.py](file:///d:/codeArea/comtrade/comtrade-io/src/comtrade_io/parser/dfr/converter.py#L56-L139)）：WNDR → 标准 `Configure`（version=1999、data_type=BINARY、min/max=-32768/32767、tran_side=S；ch_type≤10→A、=100→kV；kV 一次值 >10000 时 /1000；西里尔相别 а/в/с→A/B/C；采样率 = samples_per_cycle × grid_freq(50)；end_point=total_samples 或默认 12612；起止时间取**文件 mtime**）。数据帧：工程值 = raw×multiplier+offset；序号 1..N 生成；时间戳按采样率等间隔推算（μs，i32）。
- **只读**：Python 无 DFR 写出能力。
- 测试样本：**tests/data 下无 .dfr 样本**（风险，见 §8）。

### 2.9 导出器（JSON / CSV / CFF / 多文件）

[decorators.py](file:///d:/codeArea/comtrade/comtrade-io/src/comtrade_io/exporters/decorators.py#L21-L62)：`ExportFormat` 枚举（multi_file/cff/json/csv）+ `export_format` 装饰器统一分派，签名 `save_comtrade(output_path, format="multi_file", data_format="BINARY", **kwargs)`；`_resolve_export_path` 负责强制目标后缀。

- **JSON**（[json_exporter.py](file:///d:/codeArea/comtrade/comtrade-io/src/comtrade_io/exporters/json_exporter.py#L46-L81)）：`comtrade.model_dump()` 全量序列化（剔除 cfg/file 字段），把 DAT 各列按 `index+2`（模拟）/`analog+index+2`（状态）挂到对应通道对象的 `data` 数组；枚举取 `.value`；`ensure_ascii=False`（中文原样）；UTF-8 落盘；可选 indent。
- **CSV**（[csv_exporter.py](file:///d:/codeArea/comtrade/comtrade-io/src/comtrade_io/exporters/csv_exporter.py#L11-L43)）：表头 `Point,Time,<模拟通道名...>,<状态通道名...>`（空名回退 `A{idx}`/`D{idx}`），pandas `to_csv`（默认逗号、含表头可关、无索引列）。
- **CFF**：见 §2.7 写出。
- **多文件**（[multi_file_exporter.py](file:///d:/codeArea/comtrade/comtrade-io/src/comtrade_io/exporters/multi_file_exporter.py#L14-L37)）：CFG+DAT 必写，INF/DMF 存在则写（失败仅告警不中断）。

### 2.10 HDR 文件

Python 现状：**无模型、无解析、无写出**，仅 `hdr_path` 占位与 CFF 内的原始文本段。HDR 内容格式不固定（各厂商自由文本），**一期仅预留类型与 API 位，不实现读写**（见 FR-HDR）。

### 2.11 已知问题与行为瑕疵（重构时需决策）

| # | 问题 | Python 现状 | 重构决策建议 |
|---|---|---|---|
| P1 | 失败返回 `None` 吞掉原因 | `from_file` 链路上大量 `Optional` | Rust 用 `Result<T, Error>` 携带上下文 |
| P2 | 2/29 降级为 2/28 | 静默修改日期 | 保留行为但记录告警（兼容真实录波器的错误时钟） |
| P3 | 通道数 total 与 nA+nD 不一致 | 不报错 | 可配置：默认告警继续，strict 模式报错 |
| P4 | DAT 采样段重算有副作用 | 解析 DAT 会改写 config.sampling | Rust 中重算结果作为返回值/可选步骤，不改输入 |
| P5 | GBK/UTF-8 探测顺序不统一 | CFG/INF 为 GBK 优先，DAT 为 UTF-8 优先 | 统一探测策略并文档化（见 FR-ENC） |
| P6 | 模拟量 `multiplier` 约束 `ge=0` | pydantic 校验 | Rust 解析时不强制（真实文件存在负增益），仅文档说明 |

---

## 3. 范围定义

### 3.1 一期范围（In Scope）

1. **CFG**：完整读写（1991/1999，含可选行 timemult/time_info/sampling_time_quality）。
2. **DAT**：完整读写（ASCII / BINARY / BINARY32 / FLOAT32 读；ASCII / BINARY / BINARY32 写）。
3. **INF**：完整读写（节解析、File_Description/通道/参数段/设备节；`to_config` 与 `to_equipment_group` 双出口）。
4. **DMF**：完整读写（XML 解析与生成，命名空间容错）。
5. **CFF**：完整读写（字节级段切分读；CFG/INF/DAT 段合成写，ASCII 与二进制 DAT）。
6. **DFR**：只读（WNDR 文本头 + `[Data]` 二进制区 → 标准 Comtrade 模型；不含写出）。
7. **导出**：JSON、CSV、CFF、多文件四种导出格式。
8. **HDR**：**仅预留**（类型占位 + API 签名，不实现读写逻辑）。
9. **文件组入口**：`Comtrade::from_path`（同主名兄弟文件自动定位，扩展名大小写保持）+ CFF/DFR 单文件入口。
10. **公共 API**：通道数据访问、变位检测（`get_changed_statuses` 等价物）、多文件写出。

### 3.2 二期候选（预留接口，不实现）

HDR 读写（待格式约定明确）、DFR 写出、通道智能识别、设备拓扑自动推断、流式/增量解析超大 DAT、serde 派生（当前手写 JSON 生成器）。

---

## 4. 功能性需求

> 编号规则：FR-<模块>-<序号>。优先级：M=必须，S=应当，O=可选。

### 4.1 通用（FR-GEN）

| 编号 | 需求 | 优先级 | 验收标准 |
|---|---|---|---|
| FR-GEN-1 | 统一错误类型，携带文件路径/行号/字段上下文，不使用 panic 传播解析错误 | M | `Error` 枚举实现 `std::error::Error + Send + Sync`；测试断言错误变体 |
| FR-GEN-2 | 文本编码自动探测与转码：UTF-8（含 BOM 剥离）与 GBK 双向 | M | GBK 样本（`binary_inf.inf`）解析出正确中文；UTF-8 样本往返无损 |
| FR-GEN-3 | 所有解析入口同时提供 `from_file(path)` / `from_str`/`from_bytes` 内存版本 | M | 单测全部基于内存版本，文件版本仅集成测试 |
| FR-GEN-4 | 所有模型可序列化回与源文件**语义等价**的文本（round-trip） | M | 对 tests/data 全量样本：parse→serialize→reparse 后关键字段相等 |
| FR-GEN-5 | API 与文档语言：文档注释用中文（与 Python 库一致） | S | `cargo doc` 无告警 |
| FR-GEN-6 | no-std 友好的核心解析（不依赖运行时文件系统），IO 与解析分离 | O | 解析函数接受 `&str`/`&[u8]` |

### 4.2 CFG（FR-CFG）

| 编号 | 需求 | 优先级 | 验收标准 |
|---|---|---|---|
| FR-CFG-1 | 解析 §2.3 全部行，含 3 个可选行；1991 版无 version 字段时按 1991 处理 | M | `binary_1999.cfg`/`ascii_1991.cfg` 解析断言全字段 |
| FR-CFG-2 | 通道数行 `total,nA,nD` 提取数字（容忍 `96A`/`192D` 后缀） | M | 单测覆盖 |
| FR-CFG-3 | 模拟量 13 字段、状态量 5 字段完整映射，缺失尾部字段取默认值（primary/secondary=1，tran_side=S，contact=0） | M | 构造缺字段样本验证默认值 |
| FR-CFG-4 | 时间解析兼容 §2.3 所列全部格式族（日月序歧义按优先级策略，两位年份，微秒补零/截断，2/29 降级告警） | M | 对 15 种格式参数化测试 |
| FR-CFG-5 | 序列化：行序与 Python `Configure.__str__` 一致；时间按 `%m/%d/%Y,%H:%M:%S.%6f` | M | round-trip 测试 |
| FR-CFG-6 | 数值容错：非法浮点取默认值而非报错（对齐 `parse_float` 行为） | M | 单测 |
| FR-CFG-7 | 通道数不一致（total≠nA+nD）：默认告警继续，strict 模式报错（决策 P3） | S | 两种模式单测 |

### 4.3 DAT（FR-DAT）

| 编号 | 需求 | 优先级 | 验收标准 |
|---|---|---|---|
| FR-DAT-1 | ASCII 解析：列结构 `index,ts,analogs,statuses`；非法/缺失数值取 0 | M | `ascii_1999.dat`/`ascii_1991.dat` 断言 |
| FR-DAT-2 | 二进制解析：小端布局按 §2.4；BINARY=i16、BINARY32/FLOAT32=i32；状态字按位小端展开 | M | `binary_1999.dat` 逐点对拍 Python 输出 |
| FR-DAT-3 | 工程值换算 `raw*a+b`；timemult 乘时间戳（且只乘一次） | M | 单测 + 样本对拍 |
| FR-DAT-4 | 尾部不足整记录时截断并告警（不报错） | M | 构造截断样本 |
| FR-DAT-5 | 形状校验语义对齐：行多截断、列少（<nA+2）报错、中间态补零列（决策见设计报告） | M | 三类样本单测 |
| FR-DAT-6 | 列式存储：`index: Vec<i32>`、`timestamps_us: Vec<f64>`、每模拟通道 `Vec<f64>`、状态 `Vec<u8>` 列或位压缩存储 | M | 内存布局文档化 |
| FR-DAT-7 | 写出 ASCII/BINARY/BINARY32：工程值反算原始整数（四舍五入），状态位打包 | M | write→read round-trip 逐点相等 |
| FR-DAT-8 | 采样段重算（5% 阈值）作为**独立纯函数**提供，不修改输入 Config（决策 P4） | S | 构造双采样率样本验证 |
| FR-DAT-9 | 变位检测：返回变位通道及 `StatusChangeRecord` 列表（首条为初始状态） | S | 与 Python `get_changed_statuses` 结果对拍 |

### 4.4 HDR（FR-HDR）——一期仅预留

| 编号 | 需求 | 优先级 | 验收标准 |
|---|---|---|---|
| FR-HDR-1 | 预留 `HdrFile` 类型（`content: String` 占位）与 `Comtrade.hdr` 字段，文件组定位时记录 `hdr` 路径存在性 | M | 类型存在且可编译；`from_path` 不因 HDR 存在/缺失报错 |
| FR-HDR-2 | 预留读写 API 签名（`from_file`/`write_file`），实现返回 `Error::UnsupportedFormat("hdr")` 或 `todo` 标记，文档注明"格式不固定，暂未实现" | M | 调用返回明确错误而非 panic |
| FR-HDR-3 | CFF 段切分遇到 HDR 段时保留原始字节/文本，不因未实现而丢弃或报错 | M | CFF 含 HDR 段样本解析通过 |

### 4.5 CFF（FR-CFF）

| 编号 | 需求 | 优先级 | 验收标准 |
|---|---|---|---|
| FR-CFF-1 | 字节级段切分：§2.7 正则等价的手写扫描（ASCII 标记，多行、忽略大小写），段类型 CFG/DAT/INF/HDR；DAT 段同时保留文本与原始字节 | M | `ascii_cff_2013.cff` 切分出正确段 |
| FR-CFF-2 | 段消费：CFG 段→`Config`；DAT 段按 data_type 分派 ASCII（先清洗控制字符）/二进制；INF 段→`InfFile`；HDR 段保留原文 | M | 端到端加载断言通道数与采样数 |
| FR-CFF-3 | `Comtrade::from_cff(path)` 一步加载为完整 Comtrade | M | 与多文件样本解析结果关键字段一致 |
| FR-CFF-4 | 写出：段序 CFG→(INF)→DAT，段标记 `--- file type X ---`；文本部分 GBK 落盘，二进制 DAT 段字节直写 | M | write→read round-trip 一致 |
| FR-CFF-5 | 段标记缺失 CFG 时报错（`Error::Parse`），缺 INF/HDR 不报错 | M | 构造缺段样本 |

### 4.6 DFR（FR-DFR）

| 编号 | 需求 | 优先级 | 验收标准 |
|---|---|---|---|
| FR-DFR-1 | 文件切分：前 4088 字节 WNDR 文本（cp1251 解码）；`[Data]\r\n` 标记定位数据区，缺失时回退偏移 4088 | M | 合成样本切分正确 |
| FR-DFR-2 | WNDR 解析：魔数校验（缺失告警不中断）、站名、通道计数、模拟/状态通道行（CSV 字段按 §2.8 列位）、尾部 full_scale/samples_per_cycle/total_samples（非法取默认） | M | 单测覆盖各字段与缺省 |
| FR-DFR-3 | 二进制区解析：设备 ID 提取（≤32 字母数字）、设备注册表查私有头尺寸（未知设备 0 并告警）、帧 = nA×i16 LE + 状态字；按 cfg 首段 end_point 截断 | M | 合成帧数据逐点断言 |
| FR-DFR-4 | WNDR→Config 转换规则按 §2.8（单位/相别/一次值缩放/采样率/默认 end_point/mtime 时间） | M | 转换结果字段断言 |
| FR-DFR-5 | 数据帧→DatFile：工程值换算、序号 1..N、时间戳按采样率等间隔推算 | M | 与 FR-DFR-3 样本对拍 |
| FR-DFR-6 | `Comtrade::from_dfr(path)` 一步加载；**不提供写出** | M | 端到端可用 |

### 4.7 INF（FR-INF）

| 编号 | 需求 | 优先级 | 验收标准 |
|---|---|---|---|
| FR-INF-1 | 节解析：`[Area Type_#N]` 节头（Area 可含厂商前缀如 ZYHD）、`Key=Value` 行；未知节**保留原文**不丢弃 | M | `binary_inf.inf`（155KB，356 通道）解析计数断言 |
| FR-INF-2 | `to_config()`：由 File_Description + 通道节构建 Config（含采样段、时间、timemult） | M | 与同组 CFG 解析结果关键字段一致 |
| FR-INF-3 | 参数段合并：`CHNL_INFO_#N` 的 11/6 字段语义按 §2.5 合并进通道（primary/secondary/freq/au/bu/flag/type/equipment_no） | M | 构造参数段样本验证合并 |
| FR-INF-4 | `to_equipment_group()`：Bus/Line/Transformer 节解析为设备模型 | S | 样本含设备节时断言 |
| FR-INF-5 | 序列化：由 Config(+设备) 生成 INF 文本，含 `[ZYHD *_Channels_Parameter]` 参数段 | M | round-trip 测试 |
| FR-INF-6 | 编码：读 GBK 优先回退 UTF-8；写默认 GBK 可配置 UTF-8（对齐 Python 写行为） | M | GBK 中文样本往返 |

### 4.8 DMF（FR-DMF）

| 编号 | 需求 | 优先级 | 验收标准 |
|---|---|---|---|
| FR-DMF-1 | XML 解析：根属性 + §2.6 全部元素/属性/子元素；命名空间前缀无关（按本地名匹配） | M | `binary_1999.dmf`/`ascii_1999.dmf` 全量断言 |
| FR-DMF-2 | 属性缺失/非法数值取默认值，不中断整体解析 | M | 构造残缺样本 |
| FR-DMF-3 | XML 生成：元素顺序与 Python `to_dmf()` 一致（通道→母线→线路→变压器），UTF-8 | M | round-trip + 与 Python 输出 diff 关键行 |
| FR-DMF-4 | 实体转义（& < > " 及中文原样保留） | M | 含特殊字符样本 |
| FR-DMF-5 | 设备-通道引用完整性校验（Line 的 AnaChn/StaChn 引用存在性）作为可选检查 | O | — |

### 4.9 导出（FR-EXP）

| 编号 | 需求 | 优先级 | 验收标准 |
|---|---|---|---|
| FR-EXP-1 | 统一导出入口 `Comtrade::save(path, format, data_format)`，format ∈ {multi_file, cff, json, csv}；输出路径自动强制目标后缀 | M | 四种格式各导出一次成功 |
| FR-EXP-2 | JSON 导出：全量模型序列化（config 扁平化 + 通道定义），每通道挂 `data` 数组（模拟/状态按列对应）；中文原样（不 `\uXXXX` 转义）；UTF-8；可选缩进 | M | 与 Python `save_json` 输出结构对拍 |
| FR-EXP-3 | JSON 手写生成器（不引 serde_json）：字符串转义（`" \ \n \r \t` 与控制字符）、浮点最短往返表示 | M | 含特殊字符样本转义正确 |
| FR-EXP-4 | CSV 导出：表头 `Point,Time,<模拟名...>,<状态名...>`（空名回退 `A{idx}`/`D{idx}`），逗号分隔，可关表头，无索引列 | M | 与 Python `export_csv` 输出逐行对拍 |
| FR-EXP-5 | CFF 导出复用 FR-CFF-4；多文件导出复用 FR-API-3（INF/DMF 失败仅告警不中断） | M | — |
| FR-EXP-6 | 数值格式化对齐 pandas：CSV/ASCII 中整数不带小数点，浮点用最短表示 | S | 对拍样本无 diff |

### 4.10 文件组与对外 API（FR-API）

| 编号 | 需求 | 优先级 | 验收标准 |
|---|---|---|---|
| FR-API-1 | `Comtrade::from_path(p)`：接受任意成员后缀，自动定位兄弟文件（保持大小写风格），CFG+DAT 必需，INF/DMF 可选，HDR 仅记录路径 | M | 用 tests/data 三组样本端到端 |
| FR-API-2 | 数据访问：按索引取模拟/状态通道定义与数据列（零拷贝切片） | M | — |
| FR-API-3 | `write_to_dir(dir, stem)`：整组写出（CFG+DAT 必需，其余存在则写），DAT 格式可指定 | M | 写出后重读一致 |
| FR-API-4 | Builder 风格构造：`Config::builder()` 等，便于程序化生成录波文件 | S | 示例代码 |
| FR-API-5 | 公共再导出：`lib.rs` 顶层暴露核心类型，深层模块可 `pub(crate)` | M | `use comtrade_io::{Comtrade, Config, ...}` 可用 |
| FR-API-6 | 单文件入口：`from_cff`/`from_dfr` 与 `from_path` 产物同为 `Comtrade`，后续访问/导出 API 通用 | M | 三种入口导出 JSON 结果结构一致 |

---

## 5. 非功能性需求

| 类别 | 需求 | 指标/约束 |
|---|---|---|
| 性能 | 二进制 DAT 解析吞吐 | ≥ 200 MB/s（release，单核，顺序读）；10 万点×400 通道文件 < 100 ms |
| 性能 | 零中间堆拷贝 | 二进制解析直接写入目标列向量，禁止"逐记录 Vec<Record>"再转置 |
| 内存 | 155KB INF / 10 万点 DAT 常规样本 | 峰值内存 ≤ 文件大小 × 8 |
| 正确性 | 与 Python 基线对拍 | tests/data 全部样本解析结果逐字段一致（时间、系数、采样值） |
| 可用性 | 错误信息 | 每个错误包含：文件角色（CFG/DAT/...）、路径（若有）、行号/偏移、期望 vs 实际 |
| 可移植 | 平台 | Windows/Linux/macOS；CI 矩阵三平台；`#[cfg]` 隔离路径差异 |
| 依赖纪律 | 第三方 crate | 目标 0 个运行时依赖；若确需（见设计报告 §14），≤2 个且需评审记录 |
| 质量 | 测试 | 行覆盖率 ≥ 85%；每个格式模块含 round-trip 与对拍测试；`cargo clippy` 零告警（`-D warnings`） |
| 质量 | 格式 | `cargo fmt` 全库一致 |
| 文档 | rustdoc | 公共项 100% 中文文档注释；含至少 1 个 doctest 示例 |
| 版本 | Rust 版本 | MSRV ≤ 1.75（edition 2021） |
| 安全 | 不可信输入 | 模糊测试（手工构造畸形样本）不 panic；长度/计数字段防溢出（checked 运算） |

---

## 6. 约束与假设

1. **编码约束**：国内录波设备大量使用 GBK，编解码能力为硬需求；DFR 的 WNDR 头为 **cp1251（西里尔）**，亦需支持。`encoding_rs` 之外的手写 GBK 码表约 2 万条目，体积与维护成本过高——这是"避免第三方库"原则的唯一候选例外（详见设计报告 §14 决策记录）。
2. **字节序**：COMTRADE 二进制恒为小端；Rust 侧用 `from_le_bytes` 显式处理，不假设宿主字节序。
3. **时间语义**：库内统一使用"本地墙钟时间（NaiveDateTime 语义）"，不做时区换算（与 Python 基线一致）；`time_code/local_code` 仅作为数据保存。DFR 起止时间取文件 mtime，需要运行时读取文件元数据。
4. **样本可得性**：`tests/data/` 下已有 1991/1999、ASCII/BINARY、含 INF/DMF 的完整样本与 `ascii_cff_2013.cff`，可直接复制为 Rust 工程测试夹具（注意 `binary_inf.inf` 为 GBK 编码，须按字节复制）；**无 .dfr 样本**，DFR 测试依赖合成样本（见 §8 风险）。
5. **线程模型**：所有类型默认 `Send + Sync`（无全局状态、无日志框架依赖）。
6. **JSON/CSV 不引 serde**：导出结构固定，手写生成器（约 200 行）即可满足；serde 派生留作二期可选 feature。

## 7. 验收里程碑

| 里程碑 | 内容 | 完成判据 |
|---|---|---|
| M1 | 骨架 + error + encoding + time | 三模块单测通过 |
| M2 | CFG 读写 | FR-CFG 全部通过，样本断言绿 |
| M3 | DAT 读写 | FR-DAT 全部通过，与 Python 逐点对拍通过 |
| M4 | INF 读写 + HDR 预留 | FR-INF 通过，155KB 样本解析正确；FR-HDR 预留位编译通过 |
| M5 | DMF 读写 | FR-DMF 通过，两份样本全量断言 |
| M6 | CFF 读写 + DFR 读 | FR-CFF/FR-DFR 通过，CFF 样本 round-trip、DFR 合成样本断言 |
| M7 | JSON/CSV 导出 + Comtrade 入口 + 多文件写出 + 文档 | FR-EXP/FR-API 全部通过；`cargo test/clippy/fmt/doc` 全绿；覆盖率达标 |

## 8. 风险登记

| 风险 | 影响 | 缓解 |
|---|---|---|
| GBK 手写成本过高被迫引依赖 | 违背依赖纪律 | 设计报告 §14 预先决策；若引依赖仅 `encoding_rs`（无传递依赖、Apache/MIT） |
| 日期格式歧义（日/月序）导致与 Python 结果不一致 | 对拍失败 | 完整复刻 Python 优先级表并参数化测试 |
| DMF 手写 XML 解析器遇到畸形文件 | 解析失败 | 以本地名+属性为主的宽松解析；对拍样本兜底；畸形样本库 |
| FLOAT32 真实样本缺失 | 该分支未验证 | 构造合成样本（i32 存储语义）+ 标注待真实样本回归 |
| INF 厂商方言节（非 Public/ZYHD 前缀） | 未知节处理差异 | 未知节一律保留原文（FR-INF-1），round-trip 不丢数据 |
| **DFR 真实样本缺失** | DFR 解析逻辑无法用真实文件验证 | 按 §2.8 格式规范构造合成样本（WNDR 头 + 设备 ID + 帧数据）；代码结构与 Python 逐行对齐；标注"待真实样本回归" |
| **DFR 设备注册表覆盖不全** | 未知设备私有头尺寸误判导致帧错位 | 未知设备 header_size=0 + 告警（对齐 Python）；注册表设计为可扩展常量表 |
| cp1251 解码实现成本 | 需额外码表 | cp1251 仅 256 条目（其中 128 个 ASCII 直通），手写成本极低，不引依赖 |
| JSON 手写生成器与 Python 输出结构漂移 | 对拍失败 | 以 Python `save_json` 实际输出为基准固化结构测试 |
