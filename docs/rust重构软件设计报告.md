# comtrade-io Rust 重构 —— 软件设计报告

- 版本：v1.1（2026-08-14 修订：新增 CFF/DFR 模块与 exporters 导出层，HDR 改为预留，章节号相应调整）
- 日期：2026-08-14
- 配套文档：《需求分析报告》（`tasks/rust重构需求分析报告.md`）
- 目标 crate：`comtrade-io`（edition 2021，MSRV 1.75）

---

## 1. 设计总览

### 1.1 设计原则

1. **解析与 IO 分离**：所有格式的核心解析/序列化函数只接受 `&str`/`&[u8]`/模型引用，文件系统操作是薄封装层。便于测试、嵌入与 no-std 演进。
2. **类型驱动**：用 newtype、枚举、`Result` 把 Python 中靠约定维持的不变量变成编译期保证（如通道序号、采样段、数据格式）。
3. **零拷贝优先、列式存储**：DAT 二进制解析直接填充列向量；文本解析在可行处借用切片，仅在需要所有权时 `to_string`。
4. **错误即上下文**：统一 `Error` 枚举，解析错误携带行号/偏移/期望值；对外不 panic。
5. **往返保真**：每个格式 `parse → serialize → reparse` 语义等价；未知内容（INF 未知节、XML 未知属性）保留原文。
6. **不直译**：Python 的"解析器类 + 模型类 + builder 函数"三层，在 Rust 中收敛为"模型（数据）+ `parse`/`to_string`（自由函数或方法）"，去掉中间 builder 层。

### 1.2 crate 结构

```
comtrade-io/
├── Cargo.toml
├── src/
│   ├── lib.rs              # 公共 API 再导出 + crate 文档
│   ├── error.rs            # Error / Result
│   ├── encoding.rs         # UTF-8/GBK/cp1251 探测与转码
│   ├── time.rs             # COMTRADE 时间解析/格式化
│   ├── cfg/
│   │   ├── mod.rs          # Config 模型 + 读写入口
│   │   ├── channel.rs      # AnalogChannel / StatusChannel
│   │   └── sampling.rs     # Sampling / Segment
│   ├── dat/
│   │   ├── mod.rs          # DatFile 模型 + 读写调度
│   │   ├── ascii.rs        # ASCII 解析/写出
│   │   └── binary.rs       # 二进制解析/写出（i16/i32）
│   ├── hdr.rs              # HdrFile（预留占位，读写未实现）
│   ├── inf/
│   │   ├── mod.rs          # InfFile + 节容器
│   │   ├── section.rs      # 节头解析 / SectionData
│   │   └── builder.rs      # → Config / → EquipmentGroup
│   ├── dmf/
│   │   ├── mod.rs          # DmfFile + 读写入口
│   │   ├── xml_reader.rs   # 轻量 XML 拉式解析器
│   │   ├── xml_writer.rs   # XML 生成
│   │   └── model.rs        # Bus/Line/Transformer/Winding 等
│   ├── cff/
│   │   ├── mod.rs          # CffFile + 读写入口
│   │   └── section_splitter.rs  # 字节级段切分
│   ├── dfr/
│   │   ├── mod.rs          # DfrFile 只读入口
│   │   ├── wndr.rs         # WNDR 文本头解析
│   │   ├── binary.rs       # [Data] 二进制区解析
│   │   └── converter.rs    # WNDR → Config 转换
│   ├── exporters/
│   │   ├── mod.rs          # ExportFormat 枚举 + save 分派
│   │   ├── json.rs         # 手写 JSON 生成器
│   │   └── csv.rs          # CSV 生成器
│   ├── equipment.rs        # EquipmentGroup/Bus/Line/Transformer 共享模型
│   └── comtrade.rs         # Comtrade 顶层聚合 + 文件组定位
└── tests/
    ├── data/               # 从 Python 仓库复制的样本（含 GBK 文件、ascii_cff_2013.cff、合成 .dfr）
    ├── cfg_tests.rs
    ├── dat_tests.rs
    ├── inf_tests.rs
    ├── dmf_tests.rs
    ├── cff_tests.rs
    ├── dfr_tests.rs
    ├── export_tests.rs
    └── comtrade_tests.rs
```

模块依赖方向（无环）：

```
comtrade ──> cfg / dat / hdr / inf / dmf / cff / dfr ──> equipment
   │            │     │      │      │     │     └──> encoding(cp1251), time, error
   └──> exporters ────┴──────┴──────┴─────┴──────────> encoding, time, error
```

> 与 Python 版的关键差异：Python 存在 `comtrade_file ↔ cfg` 的循环导入（靠延迟导入规避）；Rust 版把"文件组定位"放进 `comtrade.rs`，`cfg` 模块不反向依赖它，从结构上消除环。

---

## 2. 错误处理设计（error.rs）

```rust
/// 库统一错误类型
#[derive(Debug)]
pub enum Error {
    /// IO 错误（含路径上下文）
    Io { path: PathBuf, source: std::io::Error },
    /// 文件不存在
    NotFound(PathBuf),
    /// 文本解析错误：文件角色 + 行号 + 说明
    Parse { role: FileRole, line: usize, message: String },
    /// 二进制解析错误：偏移 + 说明
    Binary { role: FileRole, offset: usize, message: String },
    /// 数值解析失败
    Number { role: FileRole, value: String },
    /// 时间格式错误
    Time(String),
    /// 通道数量不一致
    ChannelMismatch { total: usize, analog: usize, status: usize },
    /// 不支持的数据格式
    UnsupportedFormat(String),
    /// XML 解析错误
    Xml { position: usize, message: String },
    /// 编码错误
    Encoding(String),
}

/// 文件角色，用于错误上下文
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileRole { Cfg, Dat, Hdr, Inf, Dmf, Cff, Dfr }

pub type Result<T> = std::result::Result<T, Error>;
```

设计要点：

- 实现 `std::fmt::Display` 与 `std::error::Error`（手写，不引 `thiserror`）。
- `From<std::io::Error>` 只在内部带路径处显式构造，避免丢失路径上下文。
- 提供便捷构造函数 `Error::parse(role, line, msg)` 等减少调用点噪声。
- **不**使用 `Box<dyn Error>`，保证 `Send + Sync` 与匹配友好。

---

## 3. 编码模块设计（encoding.rs）

职责：字节 ⇄ String 的探测与转码，屏蔽 GBK/UTF-8 差异。

```rust
/// 文本编码探测结果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding { Utf8, Gbk, Cp1251 }

/// 探测字节序列的编码（UTF-8 严格校验优先，失败判为 GBK）
pub fn detect(bytes: &[u8]) -> Encoding;

/// 解码为 String（自动剥离 UTF-8 BOM）
pub fn decode(bytes: &[u8]) -> String;

/// 按指定编码解码
pub fn decode_with(bytes: &[u8], enc: Encoding) -> String;

/// 编码为字节
pub fn encode(text: &str, enc: Encoding) -> Vec<u8>;

/// 读取文本文件（自动探测）
pub fn read_text(path: &Path) -> Result<(String, Encoding)>;

/// 写出文本文件（指定编码）
pub fn write_text(path: &Path, text: &str, enc: Encoding) -> Result<()>;
```

GBK 实现策略（对应需求 §6 约束 1 与 §14 决策）：

- **方案 A（默认，若允许 1 个依赖）**：`encoding_rs` 的 GBK codec。
- **方案 B（零依赖）**：内置精简 GBK↔Unicode 映射（双字节区 0x8140–0xFEFE），以排序数组 + 二分或完美哈希表存储；仅覆盖 CJK 基本区与常用符号，未映射字符用 U+FFFD。
- 两方案通过 `feature = "gbk-builtin"` / `feature = "gbk-encoding-rs"` 切换，默认按 §14 决策。探测逻辑两方案一致。

**cp1251（DFR WNDR 头专用）**：单字节编码，仅 256 条目（0x80–0xFF 中约 128 个西里尔/符号映射），以 `const [char; 128]` 静态表手写实现，**不引依赖、不走 feature 开关**。cp1251 不支持编码回写（DFR 只读），`encode` 遇 `Cp1251` 返回 `Error::Encoding`。

探测顺序对齐 Python 的差异做统一（决策 P5）：

- 读：一律 `detect()`（UTF-8 严格优先，失败回退 GBK），不再区分"CFG/INF GBK 优先"。原因：UTF-8 严格校验误判率极低，而 GBK 对任意字节几乎总"成功"，GBK 优先会把 UTF-8 文件误读。
- 写：CFG/INF/CFF 默认 GBK（对齐 Python 落盘行为，可传参改 UTF-8）；DMF/JSON 默认 UTF-8。
- DFR WNDR 头固定 cp1251，不参与探测。

---

## 4. 时间模块设计（time.rs）

不引 `chrono`，自研轻量日期时间类型：

```rust
/// 本地墙钟时间（无时区），精度到微秒
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timestamp {
    pub year: u16,      // 完整年份，如 2023
    pub month: u8,      // 1-12
    pub day: u8,        // 1-31
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub micro: u32,     // 0-999_999
}
```

- 提供 `Timestamp::from_ymd_hms_micro(...) -> Result<Timestamp>` 做合法性校验（每月天数、闰年）。
- 不实现完整日历运算（不需要加减），仅提供字段访问与格式化，保持极简。
- 变位检测需要的"start_time + 微秒偏移"通过 `Timestamp::add_micros(self, us: i64) -> Timestamp` 实现（内部转"自某纪元的微秒数"计算，仅在需要时）。

解析：

```rust
/// 按 Python 优先级表解析时间字符串（15 种格式族）
pub fn parse(s: &str) -> Result<Timestamp>;
/// 格式化为 CFG 标准：MM/DD/YYYY,HH:MM:SS.ffffff
pub fn format_cfg(ts: &Timestamp) -> String;
```

实现要点（复刻 Python 行为，决策 P2）：

- 预处理：折叠时间冒号两侧空格；微秒部分 `ljust(6,'0')[:6]`。
- 按优先级顺序尝试各格式（日月序、两位/四位年、ISO、空格/逗号分隔）。
- 两位年份映射：`%y` 规则（00-68 → 2000-2068，69-99 → 1969-1999，与 strptime 一致）。
- 2/29 非法时降级 2/28 并可通过 `ParseWarning` 回调/返回值上报（不静默）。

---

## 5. CFG 模块设计（cfg/）

### 5.1 模型

```rust
// cfg/mod.rs
#[derive(Debug, Clone)]
pub struct Config {
    pub header: Header,                 // station, recorder, version
    pub channels: ChannelCount,         // total/analog/status（派生校验）
    pub analogs: Vec<AnalogChannel>,    // 按 index 升序
    pub statuses: Vec<StatusChannel>,
    pub sampling: Sampling,             // freq + segments
    pub start_time: Timestamp,
    pub trigger_time: Timestamp,
    pub data_type: DataType,
    pub timemult: f64,                  // 默认 1.0
    pub time_info: Option<TimeInfo>,    // 1999 可选
    pub sampling_time_quality: Option<SamplingTimeQuality>, // 1999 可选
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Version { V1991, V1999 }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DataType { Ascii, Binary, Binary32, Float32 }
impl DataType {
    pub fn parse(s: &str) -> Result<DataType>;
    pub fn as_str(&self) -> &'static str;
    /// 模拟量是否 32 位存储
    pub fn is_32bit(&self) -> bool;
}
```

通道模型（`cfg/channel.rs`）字段与 Python `Analog`/`Status` 对齐，但把"扩展字段"显式分组：

```rust
#[derive(Debug, Clone)]
pub struct AnalogChannel {
    // —— CFG 原生 13 字段 ——
    pub index: usize,
    pub name: String,        // ch_id
    pub phase: String,       // ph
    pub equipment: String,   // ccbm
    pub unit: String,        // uu
    pub multiplier: f64,     // a
    pub offset: f64,         // b
    pub delay: f64,          // skew (μs)
    pub min_value: f64,
    pub max_value: f64,
    pub primary: f64,        // PS
    pub secondary: f64,      // SS
    pub tran_side: TranSide, // P/S
    // —— INF/DMF 合并的扩展字段（Option 表示未提供）——
    pub ext: Option<AnalogExt>,
}

#[derive(Debug, Clone, Default)]
pub struct AnalogExt {
    pub idx_org: Option<usize>,
    pub freq: Option<f64>,
    pub au: Option<f64>,
    pub bu: Option<f64>,
    pub channel_type: Option<String>, // type
    pub flag: Option<String>,
    pub reference: Option<String>,    // IEC61850 参引
}
```

`StatusChannel` 同理（`contact`、`equipment_no`、`reference`、`type`、`flag` 进 `StatusExt`）。

> 设计理由：Python 把 CFG/INF/DMF 字段混在同一个 `Analog` 上，导致"哪些字段来自 CFG"不可知。Rust 用 `ext: Option<...>` 明确来源边界，CFG round-trip 只序列化原生 13 字段。

采样模型（`cfg/sampling.rs`）：

```rust
#[derive(Debug, Clone, Default)]
pub struct Sampling {
    pub freq: f64,                 // 标称频率（Hz）
    pub segments: Vec<Segment>,
}
#[derive(Debug, Clone, Copy)]
pub struct Segment {
    pub samp_rate: f64,   // 该段采样率 Hz
    pub end_point: usize, // 累计结束点
}
```

### 5.2 解析流程（`Config::from_str`）

采用"行游标"状态机，逐段解析，返回 `Result<Config>`：

1. 收集非空行（`trim_end_matches('\r')`）。
2. 行 1 → `Header::parse`；行 2 → `ChannelCount::parse`（提取 `96A`/`192D` 数字）。
3. 校验 `total == analog + status`：strict 模式报 `ChannelMismatch`，宽松模式记 warning 继续（FR-CFG-7）。
4. 顺序解析 `analog_count` 个模拟量行、`status_count` 个状态量行（缺尾字段取默认）。
5. 采样频率行、段数行、各段行。
6. 起始/触发时间（`time::parse`）。
7. 数据类型行（`trim_matches(',')` 后匹配）。
8. 可选行依序探测：timemult → time_info → sampling_time_quality（存在才解析）。

数值容错：内部 `parse_f64_or(s, default)` 帮助函数对齐 Python `parse_float`（非法返回默认）。

### 5.3 序列化（`Config::to_string`）

实现 `fmt::Display`，行序与 Python `Configure.__str__` 完全一致：

```
header
channel_count
analog×nA
status×nD
sampling（freq 行 + 段数行 + 段行）
start_time（format_cfg）
trigger_time
data_type
timemult
[time_info]
[sampling_time_quality]
```

浮点格式化策略：系数类（multiplier/offset 等）用最短往返表示（`{}`），整值类（min/max/primary/secondary、采样率、结束点）用 `{:.0}` 避免 `32767.0` 之类小数尾巴——与 Python `str(float)` 行为对齐需在测试中固化。

### 5.4 文件 IO

```rust
impl Config {
    pub fn from_file(path: &Path) -> Result<Config>;   // encoding::read_text + from_str
    pub fn write_file(&self, path: &Path, enc: Encoding) -> Result<()>; // 默认 GBK
}
```

---

## 6. DAT 模块设计（dat/）

### 6.1 数据模型（列式）

```rust
/// 采样数据集（列式存储，长度一致）
#[derive(Debug, Clone, Default)]
pub struct DatFile {
    pub sample_index: Vec<i32>,     // 采样序号
    pub timestamp_us: Vec<f64>,     // 时间戳（μs，已应用 timemult）
    /// 模拟量工程值：外层按通道，内层按采样点
    pub analogs: Vec<Vec<f64>>,
    /// 状态量：外层按通道，内层按采样点（0/1）
    pub statuses: Vec<Vec<u8>>,
}

impl DatFile {
    pub fn len(&self) -> usize;      // 采样点数
    pub fn is_empty(&self) -> bool;
    pub fn analog_count(&self) -> usize;
    pub fn status_count(&self) -> usize;
}
```

> 选列式而非 `Vec<SampleRecord>` 行式：① 对齐 Python DataFrame 的按列访问模式（`get_analog_channel`）；② 二进制解析可整块 `extend_from_slice` 转置，缓存友好；③ 变位检测按列扫描最高效。

### 6.2 解析

入口：

```rust
impl DatFile {
    pub fn from_file(path: &Path, cfg: &Config) -> Result<DatFile>;
    pub fn from_bytes(bytes: &[u8], cfg: &Config) -> Result<DatFile>; // 按 cfg.data_type 分派
}
```

**二进制（`dat/binary.rs`）**：

- 计算记录尺寸 `record = 4 + 4 + nA*asize + status_words*2`，`asize = is_32bit ? 4 : 2`，`status_words = nD.div_ceil(16)`。
- `sample_count = bytes.len() / record`；若 `bytes.len() % record != 0` 记 warning（FR-DAT-4）。
- 预分配各列容量；单次遍历 `0..sample_count`：
  - `index`/`timestamp` 用 `i32::from_le_bytes`；
  - 模拟量逐通道 `i16/i32::from_le_bytes → f64`，**就地** `raw*multiplier+offset` 后推入对应通道列；
  - 状态字 `u16::from_le_bytes`，对每个状态通道按 `(word >> bit) & 1` 推入对应列。
- 越界防护：所有切片索引用 `get(..)` + `Error::Binary` 兜底（防畸形长度）。

**ASCII（`dat/ascii.rs`）**：

- 逐行 `split(',')`，前 2 列 index/ts，随后 nA 模拟量、nD 状态量；缺失/非法取 0（对齐 `fillna(0)`）。
- 应用系数与 `analog_precision`（可配置，默认 3，范围 [1,6] 才生效）。

**后处理（独立纯函数，决策 P4）**：

```rust
/// 按时间戳重算采样段（5% 阈值），不修改输入
pub fn recalculate_segments(timestamps_us: &[f64], nominal_freq: f64) -> Vec<Segment>;
```

- timemult 在解析时一次性乘到 `timestamp_us`（FR-DAT-3），重算函数接收的已是应用后的值，避免重复乘。

**形状校验（FR-DAT-5）**：解析后按 `cfg` 校验：

- 行数 > 期望（最后段 end_point）→ 截断；
- 列数（实际通道数）< `nA+2` 语义 → 模拟通道不足，返回 `Error::Parse`；
- 状态列不足 → 补零列到 nD。

### 6.3 写出

```rust
impl DatFile {
    pub fn write_file(&self, path: &Path, cfg: &Config, dt: DataType) -> Result<()>;
    pub fn to_bytes(&self, cfg: &Config, dt: DataType) -> Vec<u8>;   // 二进制
    pub fn to_ascii(&self, cfg: &Config) -> String;                  // ASCII
}
```

- 工程值反算：`raw = round((v - offset) / multiplier)`（multiplier==0 → 0），ASCII 全列整数化；二进制按 i16/i32 打包，状态位按字打包（FR-DAT-7）。
- 写文件用 `BufWriter`。

### 6.4 变位检测（FR-DAT-9）

```rust
/// 状态变位记录
#[derive(Debug, Clone, Copy)]
pub struct StatusChange {
    pub sample_point: usize,     // 1-based
    pub timestamp: Option<Timestamp>, // start_time + offset，可得时填充
    pub state: u8,
}

impl DatFile {
    /// 返回发生变位的状态通道索引及其变位记录
    pub fn changed_statuses(&self, start_time: Option<Timestamp>) -> Vec<(usize, Vec<StatusChange>)>;
}
```

逐状态列扫描：首点记初始状态；`prev != cur` 处记变位。O(nD × nSamples) 单次遍历。

---

## 7. HDR 模块设计（hdr.rs）——一期预留

HDR 内容格式不固定（各厂商自由文本），一期**只定义类型与 API 位，不实现读写逻辑**（需求 FR-HDR）：

```rust
/// HDR 自由文本文件（预留，格式不固定暂未实现读写）
#[derive(Debug, Clone, Default)]
pub struct HdrFile {
    /// 文本内容（预留字段）
    pub content: String,
}

impl HdrFile {
    /// 预留：读取 HDR 文件。当前返回 `Error::UnsupportedFormat("hdr")`
    pub fn from_file(_path: &Path) -> Result<HdrFile> {
        Err(Error::UnsupportedFormat("hdr 格式不固定，暂未实现".into()))
    }
    /// 预留：写出 HDR 文件。当前返回 `Error::UnsupportedFormat("hdr")`
    pub fn write_file(&self, _path: &Path) -> Result<()> {
        Err(Error::UnsupportedFormat("hdr 格式不固定，暂未实现".into()))
    }
}
```

要点：

- `Comtrade.hdr: Option<HdrFile>` 字段保留，文件组定位时记录 hdr 路径存在性但不加载内容。
- CFF 段切分遇到 HDR 段时，把段原文存入 `CffFile.hdr_text`（不丢弃、不报错），供将来实现后消费。
- 二期实现时仅需填充两个方法体，API 签名不变，不破坏下游。

---

## 8. INF 模块设计（inf/）

### 8.1 节容器（保留原文，FR-INF-1）

```rust
// inf/section.rs
/// 一个 INF 节
#[derive(Debug, Clone)]
pub struct Section {
    pub area: String,        // 如 Public / ZYHD
    pub kind: SectionKind,   // 已知类型枚举，未知 → Other(原文)
    pub index: usize,        // _#N，无则 0
    pub fields: Vec<(String, String)>, // 保持顺序
    pub raw: String,         // 节原文（含节头），保证未知节往返不丢
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SectionKind {
    FileDescription,
    AnalogChannel,
    StatusChannel,
    AnalogChannelsParameter,
    StatusChannelsParameter,
    Bus,
    Line,
    Transformer,
    RecordInformation,
    Other(String), // 未知类型，保留原始 type 文本
}

impl Section {
    pub fn get(&self, key: &str) -> Option<&str>; // 大小写不敏感查 Key
}
```

`InfFile`：

```rust
#[derive(Debug, Clone, Default)]
pub struct InfFile {
    pub sections: Vec<Section>,
}

impl InfFile {
    pub fn from_file(path: &Path) -> Result<InfFile>; // GBK 优先探测
    pub fn from_str(text: &str) -> InfFile;
    pub fn sections_of(&self, kind: &SectionKind) -> impl Iterator<Item = &Section>;
    pub fn file_description(&self) -> Option<&Section>;
}
```

节头解析：正则等价的手写解析——`[``内容``]`，内容按首个空格分 `area` 与 `rest`，`rest` 末尾 `_#数字` 提取 index，余下为 type；type 大写匹配 `SectionKind`，未命中归 `Other`。

### 8.2 构建器（`inf/builder.rs`）

```rust
impl InfFile {
    /// 由 File_Description + 通道节构建 Config（FR-INF-2）
    pub fn to_config(&self) -> Config;
    /// 合并参数段 CHNL_INFO_#N 到通道 ext（FR-INF-3）
    fn apply_channel_parameters(&self, cfg: &mut Config);
    /// 由 Bus/Line/Transformer 节构建设备组（FR-INF-4）
    pub fn to_equipment_group(&self) -> EquipmentGroup;
}
```

- `to_config`：映射 §需求 2.5 的全部 Key；采样段由 `Sample_Rate_Count` + `Sample_Rate_#N`/`End_Sample_Rate_#N` 组装；时间走 `time::parse`。
- 参数段解析：按 `,` 分割，模拟量取 11 字段、状态量取 ≥5 字段，字段含义与 Python `_parse_analog_params`/`_parse_status_params` 一致，写入对应通道的 `ext`。
- 设备节解析复用 `equipment.rs` 模型。

### 8.3 序列化（FR-INF-5）

```rust
impl InfFile {
    pub fn to_string(&self) -> String; // 逐节输出：节头 + Key=Value + 空行
    pub fn write_file(&self, path: &Path, enc: Encoding) -> Result<()>; // 默认 GBK
}

/// 由 Config(+设备组) 生成 INF（对齐 Python Comtrade.to_inf 结构）
pub fn from_config(cfg: &Config, equipment: Option<&EquipmentGroup>) -> InfFile;
```

生成顺序：`File_Description` → 各 `Analog_Channel_#N` → 各 `Status_Channel_#N` → `[ZYHD Analog_Channels_Parameter]` → `[ZYHD Status_Channels_Parameter]` → 设备节。未知节（来自解析保留的 `Other`）在"编辑后回写"场景中原样输出。

---

## 9. DMF 模块设计（dmf/）

### 9.1 轻量 XML 解析器（xml_reader.rs）

不引 `quick-xml`，实现一个**面向 DMF 场景**的拉式解析器（约 200 行）：

```rust
/// XML 事件
enum XmlEvent<'a> {
    Start { name: &'a str, attrs: Vec<(&'a str, &'a str)>, empty: bool },
    End { name: &'a str },
    Text(&'a str),
    Eof,
}

struct XmlReader<'a> { input: &'a [u8], pos: usize }
impl<'a> XmlReader<'a> {
    fn next_event(&mut self) -> Result<XmlEvent<'a>>;
}
```

能力边界（刻意收窄）：

- 支持：XML 声明跳过、注释/CDATA 跳过、开始/自闭合/结束标签、属性（单/双引号）、实体解码（`&amp; &lt; &gt; &quot; &apos;` + `&#xNN;/&#NN;`）。
- 不支持（DMF 用不到）：DTD、处理指令、命名空间声明解析（**按本地名匹配**，即取 `:` 之后部分，天然满足 FR-DMF-1 的前缀无关）。
- 标签名返回**本地名**（去前缀），属性名同理，从根上解决 `scl:`/`ns:`/无前缀差异。

### 9.2 模型（dmf/model.rs）

与需求 §2.6 一一对应的结构体：`DmfAnalogChannel`、`DmfStatusChannel`、`Bus`、`Line`（含 `Rx/Cg/Px/Mr/AccBran/引用列表/差动电流`）、`Transformer`、`Winding`、`AcvChn`。数值字段 `f64`/`usize`，缺失取默认。

顶层：

```rust
#[derive(Debug, Clone, Default)]
pub struct DmfFile {
    pub station_name: String,
    pub version: String,
    pub reference: String,
    pub rec_dev_name: String,
    pub analogs: Vec<DmfAnalogChannel>,
    pub statuses: Vec<DmfStatusChannel>,
    pub buses: Vec<Bus>,
    pub lines: Vec<Line>,
    pub transformers: Vec<Transformer>,
}
```

### 9.3 解析（栈式状态机）

`DmfFile::from_bytes`：遍历事件，用 `Option<Bus>/Option<Line>/Option<Transformer>/Option<Winding>` 作为"当前容器"，遇 Start 填充属性、遇子元素挂到当前容器、遇对应 End 收尾 push。自闭合容器（`empty=true`）立即收尾（处理 `<scl:Bus .../>` 情形）。属性读取全部经 `attr_or(name, default)`，非法数值不中断（FR-DMF-2）。

### 9.4 生成（xml_writer.rs）

`DmfFile::to_string`：手写字符串拼接（DMF 结构固定、无需通用 DOM），元素顺序与 Python `to_dmf()` 一致：根 → 模拟通道 → 状态通道 → 母线 → 线路 → 变压器。属性值经 `escape()`（FR-DMF-4）。输出 UTF-8，首行 XML 声明。

`write_file` 落盘 UTF-8。

---

## 10. CFF 模块设计（cff/）

### 10.1 段切分（section_splitter.rs）

不引 `regex`，手写 ASCII 标记扫描（标记本身全 ASCII，可在字节级安全匹配，避免 GBK 多字节误命中）：

```rust
/// CFF 段
#[derive(Debug, Clone, Default)]
pub struct CffSections {
    pub cfg: Option<String>,
    pub dat_text: Option<String>,
    pub dat_bytes: Option<Vec<u8>>,
    pub inf: Option<String>,
    pub hdr: Option<String>, // 预留保留，不消费
}

/// 在字节流上定位 `--- file type: XXX ---` 标记（等价 Python 正则：
/// ^--{1,2}\s*file\s+type\s*:?\s+(\w+)(?:\s+[^-]*)?\s*---，多行、忽略大小写），
/// 返回各段内容；段文本按 GBK(errors→U+FFFD) 解码，DAT 段同时保留原始字节
pub fn extract_sections(bytes: &[u8]) -> CffSections;
```

实现要点：

- 逐行扫描（按 `\n` 切字节，行尾 `\r` 容忍），对每行做"标记前缀匹配"：跳过前导 `-`（1–2 个）与空白 → 匹配 `file` → 空白 → `type` → 可选 `:` → 空白 → 捕获 `\w+` 段类型 → 行内其余非 `-` 内容 → 以 `---` 收尾。全部 ASCII 忽略大小写比较。
- 段内容 = 当前标记行之后到下一标记行之前；首尾空白剔除。
- DAT 段双份保留：GBK 解码文本（ASCII DAT 用）+ 原始字节（二进制 DAT 用）。

### 10.2 模型与读取

```rust
#[derive(Debug, Clone, Default)]
pub struct CffFile {
    pub sections: CffSections,
}

impl CffFile {
    pub fn from_file(path: &Path) -> Result<CffFile>;
    pub fn from_bytes(bytes: &[u8]) -> CffFile;

    /// CFG 段 → Config（缺失报 Error::Parse{role: Cff}）
    pub fn to_config(&self) -> Result<Config>;
    /// DAT 段 → DatFile（按 config.data_type 分派；ASCII 先清洗控制字符，保留 \n\r\t）
    pub fn to_data(&self, config: &Config) -> Result<DatFile>;
    /// INF 段 → InfFile（缺失返回 None，不报错）
    pub fn to_inf(&self) -> Option<InfFile>;
    /// 一步加载为 Comtrade（hdr 段原文保留在 sections.hdr）
    pub fn to_comtrade(&self, dir: &Path, stem: &str) -> Result<Comtrade>;
}
```

### 10.3 写出

```rust
impl CffFile {
    /// 由 Comtrade 生成 CFF 字节流：
    /// `--- file type CFG ---\n` + CFG 文本
    /// →（有 INF）`--- file type INF ---\n` + INF 文本
    /// → `--- file type DAT ---\n` + DAT（ASCII 文本或二进制字节）
    /// 文本部分 GBK 编码（不可映射字符丢弃，对齐 Python errors="ignore"）
    pub fn from_comtrade(ct: &Comtrade, dt: DataType) -> Vec<u8>;
    pub fn write_file(ct: &Comtrade, path: &Path, dt: DataType) -> Result<()>;
}
```

- ASCII DAT 段复用 `DatFile::to_ascii`；二进制复用 `DatFile::to_bytes`。
- 输出为字节流拼接（文本段 encode 后与二进制段直接 concat），避免 String 中转破坏二进制内容。

---

## 11. DFR 模块设计（dfr/）——只读

### 11.1 常量与设备注册表（mod.rs）

```rust
pub const WNDR_TEXT_SIZE: usize = 4088;
pub const DATA_MARKER: &[u8] = b"[Data]\r\n";
pub const DEFAULT_FULL_SCALE: u32 = 32768;
pub const DEFAULT_SAMPLES_PER_CYCLE: u32 = 24;
pub const DEFAULT_GRID_FREQ: f64 = 50.0;
pub const DEFAULT_END_POINT: usize = 12612;

/// 已知设备注册表：device_id → 私有头尺寸（可扩展常量表）
pub fn device_header_size(device_id: &str) -> Option<usize>; // "2704V042"/"2704V072" → 248
```

### 11.2 WNDR 文本头解析（wndr.rs）

```rust
#[derive(Debug, Clone, Default)]
pub struct WndrSection {
    pub station_name: String,
    pub analog_channels: Vec<WndrAnalogChannel>,
    pub status_channels: Vec<WndrStatusChannel>,
    pub full_scale: u32,
    pub samples_per_cycle: u32,
    pub total_samples: usize,
    pub grid_freq: f64,
}

impl WndrSection {
    /// 解析 WNDR 文本（cp1251 解码后的 &str）
    /// 行 1 魔数 [WNDR]（缺失告警不中断）→ 行 2 站名 → 行 3 通道计数
    /// → nA 模拟通道行（CSV 列位见需求 §2.8）→ nD 状态通道行
    /// → 尾部 full_scale / samples_per_cycle / total_samples（非法取默认）
    pub fn from_text(text: &str) -> Result<WndrSection>;
}
```

- CSV 行解析手写：支持双引号包裹字段与引号转义（Python 用 csv 模块，DFR 实际仅简单引号包裹名称）。
- 模拟通道取列：0=idx、1=name、2=相别码、3=监测回路、4=满度值、5=转换系数、6=单位串、7=相别、12=ch_type、13=rated_primary；缺列取默认。

### 11.3 二进制区解析（binary.rs）

```rust
#[derive(Debug, Clone, Default)]
pub struct DfrBinary {
    pub device_id: String,
    pub bin_header: Vec<u8>,
    pub frame_data: Vec<u8>,
}

impl DfrBinary {
    /// 从整文件字节切分：[Data]\r\n 定位（缺失回退 4088）→ 设备 ID（≤32 字母数字）
    /// → 注册表查私有头尺寸（未知 0）→ 其余为帧数据
    pub fn from_raw(raw: &[u8]) -> DfrBinary;

    /// 帧解析 → DatFile：帧 = nA×i16 LE + ceil(nD/16)×u16 LE，无序号/时间戳列；
    /// 工程值 raw*multiplier+offset；序号 1..N；时间戳按首段采样率等间隔推算（μs）；
    /// 按 cfg 首段 end_point 截断；尾部不足整帧丢弃
    pub fn to_data(&self, cfg: &Config) -> Result<DatFile>;
}
```

### 11.4 转换器（converter.rs）

```rust
/// WNDR → Config（规则对齐 Python converter，见需求 §2.8）：
/// version=1999、data_type=Binary、min/max=-32768/32767、tran_side=S；
/// ch_type≤10→A、=100→kV；kV 一次值 >10000 → /1000；西里尔相别映射；
/// 采样率 = samples_per_cycle × grid_freq；end_point = total_samples 或 12612；
/// start/trigger 时间由调用方传入（from_file 用文件 mtime）
pub fn wndr_to_config(wndr: &WndrSection, time: Timestamp) -> Config;
```

### 11.5 入口

```rust
#[derive(Debug, Clone, Default)]
pub struct DfrFile {
    pub wndr: WndrSection,
    pub binary: DfrBinary,
}

impl DfrFile {
    pub fn from_file(path: &Path) -> Result<DfrFile>;      // 读字节 + mtime
    pub fn from_bytes(raw: &[u8], time: Timestamp) -> Result<DfrFile>;
    pub fn to_comtrade(&self, time: Timestamp) -> Result<Comtrade>; // 无 dir/stem 来源信息，置空
}
```

> 不提供写出（对齐 Python，需求 FR-DFR-6）。测试用合成样本：按格式规范构造 WNDR 头 + `[Data]\r\n` + 设备 ID + 帧字节。

---

## 12. 导出器设计（exporters/）

### 12.1 统一入口

```rust
/// 导出格式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat { MultiFile, Cff, Json, Csv }

impl ExportFormat {
    pub fn parse(s: &str) -> Result<ExportFormat>; // 忽略大小写
    /// 该格式的目标文件后缀
    pub fn suffix(&self) -> &'static str; // ".cfg"（多文件用目录）/".cff"/".json"/".csv"
}

impl Comtrade {
    /// 统一导出：format 分派；输出路径自动强制目标后缀（对齐 _resolve_export_path）
    pub fn save(&self, path: &Path, format: ExportFormat, dt: DataType) -> Result<()>;
    pub fn save_with(&self, path: &Path, format: ExportFormat, opts: &ExportOptions) -> Result<()>;
}

/// 导出选项
#[derive(Debug, Clone)]
pub struct ExportOptions {
    pub data_type: DataType,        // DAT/CFF 的数据格式
    pub json_indent: Option<usize>, // JSON 缩进（None=紧凑）
    pub csv_headers: bool,          // CSV 表头，默认 true
}
```

### 12.2 JSON 生成器（json.rs，手写不引 serde）

对齐 Python `save_json` 输出结构（需求 FR-EXP-2）：

```rust
/// 生成 Comtrade 的 JSON 文本
/// 结构：config 扁平化字段（station/rec_dev_name/version/channel_num/sampling/
/// start_time/trigger_time/data_type/timemult...）+ analogs[] + statuses[]，
/// 每个通道对象挂 "data" 数组（模拟取对应列 f64，状态取 u8）
pub fn to_json(ct: &Comtrade, indent: Option<usize>) -> String;
pub fn write_json(ct: &Comtrade, path: &Path, indent: Option<usize>) -> Result<()>; // UTF-8
```

实现要点：

- 小型 `JsonWriter`：维护缩进层级，提供 `key/str/num/bool/array/object` 原语；字符串转义 `" \ \n \r \t \b \f` 与 `< 0x20` 控制字符（`\u00XX`）；中文等非 ASCII **原样输出**（对齐 `ensure_ascii=False`）。
- 浮点用 Rust `{}` 最短往返格式；时间字段格式化为 `MM/DD/YYYY,HH:MM:SS.ffffff`（与 CFG 一致，具体以 Python 输出对拍为准）。
- 枚举（data_type 等）输出字符串值。

### 12.3 CSV 生成器（csv.rs）

```rust
/// 表头：Point,Time,<模拟通道名（空名回退 A{idx}）...>,<状态通道名（空名回退 D{idx}）...>
/// 数据行：index,timestamp_us,模拟工程值...,状态 0/1...
pub fn to_csv(ct: &Comtrade, headers: bool) -> String;
pub fn write_csv(ct: &Comtrade, path: &Path, headers: bool) -> Result<()>; // UTF-8
```

- 通道名含逗号/引号时按 RFC4180 加引号转义（pandas 同行为）。
- 数值格式对齐 pandas `to_csv`：整数无小数点，浮点最短表示（FR-EXP-6，以对拍固化）。

### 12.4 CFF / 多文件导出

- CFF 导出 = `CffFile::write_file`（§10.3）。
- 多文件导出 = `Comtrade::write_to_dir`（§13.2），INF/DMF 写出失败仅记录到返回的警告列表，不中断（对齐 Python `export_multi_file`）。

---

## 13. 设备模型与顶层聚合

### 13.1 equipment.rs（INF/DMF 共享）

```rust
#[derive(Debug, Clone, Default)]
pub struct EquipmentGroup {
    pub buses: Vec<Bus>,
    pub lines: Vec<Line>,
    pub transformers: Vec<Transformer>,
}
```

> Bus/Line/Transformer 的**设备拓扑模型**与 DMF 的元素模型字段高度重合，设计上 `dmf::model` 直接复用 `equipment.rs` 的类型（DMF 解析产物即设备组 + 通道），避免 Python 中 `model/equipment` 与 `parser/dmf/*_element` 两套近似模型。INF 的 `to_equipment_group` 也产出同一类型。

### 13.2 comtrade.rs（文件组入口，FR-API-1/6）

```rust
#[derive(Debug, Clone, Default)]
pub struct Comtrade {
    pub config: Config,
    pub data: Option<DatFile>,
    pub hdr: Option<HdrFile>,       // 预留，一期恒为 None
    pub inf: Option<InfFile>,
    pub dmf: Option<DmfFile>,
    pub equipment: EquipmentGroup, // 由 dmf/inf 派生
    pub paths: ComtradePaths,      // 各成员文件路径（存在性已知）
}

#[derive(Debug, Clone, Default)]
pub struct ComtradePaths {
    pub dir: PathBuf,
    pub stem: String,
    pub cfg: Option<PathBuf>,
    pub dat: Option<PathBuf>,
    pub hdr: Option<PathBuf>,
    pub inf: Option<PathBuf>,
    pub dmf: Option<PathBuf>,
}

impl Comtrade {
    /// 从任一成员文件路径加载整组（CFG+DAT 必需）
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Comtrade>;
    /// 从 CFF 单文件加载（FR-CFF-3）
    pub fn from_cff<P: AsRef<Path>>(path: P) -> Result<Comtrade>;
    /// 从 DFR 单文件加载（FR-DFR-6，起止时间取文件 mtime）
    pub fn from_dfr<P: AsRef<Path>>(path: P) -> Result<Comtrade>;
    /// 仅加载配置（不读 DAT）
    pub fn load_config<P: AsRef<Path>>(path: P) -> Result<Config>;

    // 数据访问（FR-API-2）
    pub fn analog_channel(&self, index: usize) -> Option<ChannelView<'_>>;
    pub fn status_channel(&self, index: usize) -> Option<ChannelView<'_>>;
    pub fn changed_statuses(&self) -> Vec<(usize, Vec<StatusChange>)>;

    // 写出（FR-API-3）
    pub fn write_to_dir(&self, dir: &Path, stem: &str, dt: DataType) -> Result<()>;

    // 统一导出（FR-EXP-1，分派到 exporters，见 §12）
    pub fn save(&self, path: &Path, format: ExportFormat, dt: DataType) -> Result<()>;
}
```

文件组定位算法（对齐 Python `from_path`）：

1. 取输入后缀小写，校验属于 `{cfg,dat,hdr,inf,dmf,cff,dfr}`。
2. **后缀为 `cff` → 转 `from_cff`；`dfr` → 转 `from_dfr`**（单文件模式，不做兄弟定位）。
3. 记录输入后缀大小写风格 `is_upper`。
4. 对每个目标后缀，按 `parent / (stem + 目标后缀按风格大小写)` 拼路径，`exists()` 判定填入 `paths`。
5. `from_path` 依次：`Config::from_file(cfg)` → `DatFile::from_file(dat, &cfg)` → INF/DMF 可选加载（HDR 仅记录路径不加载）→ 由 DMF（优先）或 INF 构建 `equipment`。

`ChannelView<'_>`：零拷贝视图，持 `&AnalogChannel` 定义 + `&[f64]`/`&[u8]` 数据切片。

---

## 14. 依赖决策记录（关键设计决策）

| 决策点 | 选项 | 结论 | 理由 |
|---|---|---|---|
| GBK 编解码 | A. `encoding_rs`；B. 内置码表 | **默认 A，feature 可切 B** | GBK 全码表约 2.3 万映射，手写体积大、易错、难维护；`encoding_rs` 零传递依赖、纯 Rust、WHATWG 标准、Apache/MIT。这是"尽量避第三方库"的唯一例外，且通过 feature 保留零依赖退路。 |
| cp1251 编解码 | A. `encoding_rs`；B. 内置 256 条目表 | **B** | 单字节编码仅 128 个非 ASCII 映射，`const` 表 30 行搞定，不值得为其启用依赖。 |
| 日期时间 | A. `chrono`；B. 自研 `Timestamp` | **B** | 仅需字段存储 + 解析/格式化 + 微秒加法，无需时区/日历运算；自研 < 200 行，避免引入大依赖。 |
| XML | A. `quick-xml`；B. 手写拉式解析 | **B** | DMF 场景固定（属性为主、无混合内容、无命名空间语义），手写 200 行可控且零依赖；通用 XML 库的复杂度用不上。 |
| JSON | A. `serde_json`；B. 手写 `JsonWriter` | **B** | 只有"写出"单向需求、结构固定；手写 ~200 行，避免引入 serde 体系。读 JSON 无需求。 |
| CSV | A. `csv` crate；B. 手写拼接 | **B** | 输出结构简单（无嵌套、转义规则单一）；WNDR 的 CSV 行读取同样手写。 |
| 正则（CFF 段标记） | A. `regex`；B. 手写 ASCII 扫描 | **B** | 标记模式简单且全 ASCII，逐行前缀匹配 ~60 行，避免 regex 依赖。 |
| 错误类型 | A. `thiserror`；B. 手写 `Display` | **B** | 变体少，手写 30 行即可，省一个依赖。 |
| 测试断言 | 标准库 `assert!` | 标准库 | 不引测试框架。 |

**净结果**：默认构建最多 1 个运行时依赖（`encoding_rs`），启用 `gbk-builtin` feature 时 0 依赖。dev-dependencies 允许 `criterion`（性能基准，可选）。

---

## 15. 测试策略

### 15.1 测试金字塔

- **单元测试（模块内 `#[cfg(test)]`）**：每个解析/序列化函数的正例、边界、畸形输入。重点：
  - cfg：13/5 字段映射、缺尾字段默认值、可选行、通道数不一致两模式、15 种时间格式参数化。
  - dat：ASCII/二进制/i32 布局、状态位打包展开、截断告警、形状校验三态、系数往返。
  - inf：节头解析（含厂商前缀）、未知节保留、参数段合并、Key 大小写不敏感。
  - dmf：本地名匹配（scl/ns/无前缀三种样本）、自闭合容器、属性缺失默认、实体转义。
  - cff：段标记变体（`--`/`---`、有无冒号、大小写）、GBK 段解码、DAT 双份保留、缺 CFG 段报错。
  - dfr：WNDR 各字段与缺省、设备 ID 提取、已知/未知设备头尺寸、帧截断、转换规则（单位/相别/缩放）。
  - exporters：JSON 转义（引号/反斜杠/控制字符/中文原样）、缩进与紧凑两种模式；CSV 表头回退、RFC4180 转义。
- **集成测试（`tests/`）**：
  - **Round-trip**：对 `tests/data` 每个样本 `parse → serialize → reparse`，断言关键字段相等；CFF 样本 write→read 往返。
  - **端到端**：`Comtrade::from_path` 加载三组样本（binary_1999 / ascii_1999 / ascii_1991 + binary_inf），`from_cff` 加载 `ascii_cff_2013.cff`，`from_dfr` 加载合成样本；断言通道数、采样数、首末采样值。
  - **与 Python 对拍**：导出 Python 解析结果为 JSON 基准（一次性脚本），Rust 侧逐字段比对（时间、系数、采样点）；JSON/CSV 导出输出与 Python `save_json`/`export_csv` 逐行 diff。
- **模糊/畸形测试**：手工构造畸形 CFG/DAT/DMF/CFF/DFR（超长计数、负长度、非法 XML、缺失段标记、截断帧），断言返回 `Err` 而非 panic。

### 15.2 测试数据

从 `comtrade-io/tests/data/` 复制：`binary_1999.*`、`ascii_1999.*`、`ascii_1991.*`、`binary_inf.*`、`*.dmf`、`ascii_cff_2013.cff`。**注意**：`binary_inf.inf` 为 GBK 编码，须按字节复制（不能用文本方式打开改写）。DFR 无真实样本，在 `tests/data/` 提供合成 `.dfr`（构造脚本随仓库提交，格式依据需求 §2.8）。

### 15.3 质量门禁

`cargo fmt --check` + `cargo clippy -- -D warnings` + `cargo test` 全绿；覆盖率（`cargo-llvm-cov`）≥ 85%。

---

## 16. 实施计划（对应需求里程碑）

| 阶段 | 交付 | 依赖 |
|---|---|---|
| M1 | Cargo 骨架 + `error` + `encoding`（含 cp1251 内置表）+ `time` + 各自单测 | 无 |
| M2 | `cfg` 读写 + 样本测试 | M1 |
| M3 | `dat` 读写 + 对拍 | M2 |
| M4 | `inf` 读写 + `hdr` 预留位 + 155KB 样本 | M2 |
| M5 | `dmf` 读写（含 XML 解析器） | M1 |
| M6 | `cff` 读写 + `dfr` 读 + CFF 样本 round-trip + DFR 合成样本 | M2/M4 |
| M7 | `exporters`（JSON/CSV）+ `equipment` + `comtrade` 聚合（含 from_cff/from_dfr/save）+ 多文件写出 + 文档 + 覆盖率 | M2-M6 |

并行性：M4 的 inf 与 M5 的 dmf 可并行；M6 的 cff 依赖 cfg/dat/inf，dfr 依赖 encoding(cp1251)/cfg/dat，两者可并行；`equipment.rs` 在 M4/M5 前定稿。

---

## 17. 与 Python 行为差异清单（显式声明）

| 项 | Python | Rust | 原因 |
|---|---|---|---|
| 失败语义 | 返回 `None` + 日志 | `Result::Err` 带上下文 | Rust 习惯，信息不丢失 |
| DAT 解析改 config.sampling | 有副作用 | 无副作用，重算为独立函数 | 纯函数更易测 |
| 编码探测顺序 | CFG/INF GBK 优先 | 统一 UTF-8 严格优先回退 GBK | 降低误判（P5） |
| HDR | 无实现 | **预留占位，读写返回 `UnsupportedFormat`** | 格式不固定，一期不实现（需求变更） |
| CFF 段切分 | `re` 正则（字节级） | 手写 ASCII 逐行扫描 | 行为等价，去 regex 依赖 |
| DFR | 只读 | 只读（对齐） | 规范未定义写出 |
| JSON/CSV 导出 | json/pandas | 手写生成器 | 去依赖；输出结构以 Python 对拍为准 |
| cp1251 解码 | codecs 内置 | 内置 256 条目 `const` 表 | 去依赖 |
| 通道扩展字段 | 与原生字段混存 | `ext: Option<...>` 分组 | 来源可追溯 |
| 日志 | loguru 全局 | 不内置日志，错误经 `Result` 传递 | 库不绑定日志框架 |
| DataFrame | pandas | 列式 `Vec` | 去重依赖 |

---

## 18. 公共 API 速览（lib.rs 再导出）

```rust
pub use comtrade::{Comtrade, ComtradePaths};
pub use cfg::{Config, AnalogChannel, StatusChannel, Sampling, Segment, DataType, Version};
pub use dat::{DatFile, StatusChange};
pub use hdr::HdrFile;
pub use inf::InfFile;
pub use dmf::DmfFile;
pub use cff::CffFile;
pub use dfr::DfrFile;
pub use exporters::{ExportFormat, ExportOptions};
pub use equipment::{EquipmentGroup, Bus, Line, Transformer};
pub use error::{Error, Result, FileRole};
pub use encoding::Encoding;
pub use time::Timestamp;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
```

使用示例（写入 rustdoc doctest）：

```rust,no_run
use comtrade_io::{Comtrade, ExportFormat, DataType};

// 多文件组加载
let ct = Comtrade::from_path("tests/data/binary_1999.cfg")?;
println!("站名: {}", ct.config.header.station);
println!("采样点数: {}", ct.data.as_ref().map(|d| d.len()).unwrap_or(0));
// 取第 1 个模拟通道数据
if let Some(ch) = ct.analog_channel(1) {
    println!("通道 {} 首点: {:?}", ch.definition.name, ch.samples.first());
}

// 单文件加载与导出
let ct_cff = Comtrade::from_cff("tests/data/ascii_cff_2013.cff")?;
ct_cff.save("out/result.json", ExportFormat::Json, DataType::Ascii)?;
ct_cff.save("out/result.csv", ExportFormat::Csv, DataType::Ascii)?;
# Ok::<(), comtrade_io::Error>(())
```
