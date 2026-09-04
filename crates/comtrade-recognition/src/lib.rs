//! CFG -> DMF 识别规则、TOML 持久化与基础设备归组。

use std::collections::BTreeMap;
use std::path::Path;

use comtrade_io::{
    is_iec61850_reference, AccBran, AcvChn, Bus, Config, DmfAnalogChannel, DmfFile,
    DmfStatusChannel, Line, Transformer, TransformerWinding,
};
use regex::Regex;
use serde::{Deserialize, Serialize};

pub const RULE_FORMAT_VERSION: u32 = 2;
/// 规则加载结果。配置缺失或无法识别时 `rules` 为内置默认规则。
#[derive(Debug, Clone)]
pub struct RuleLoadResult {
    pub rules: RecognitionRules,
    pub used_default: bool,
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RecognitionRules {
    pub format_version: u32,
    pub basic: BasicRules,
    pub normalization: NormalizationRules,
    pub analog: AnalogRules,
    pub equipment_rules: Vec<EquipmentRule>,
    pub status_rules: Vec<StatusRule>,
    pub reserve_rules: Vec<ReserveRule>,
    /// 模拟量备用通道判定。与开关量规则分开，避免不同命名习惯相互误判。
    pub analog_reserve_rules: AnalogReserveRules,
    /// 开关量备用通道判定。
    pub status_reserve_rules: StatusReserveRules,
    pub thresholds: ThresholdRules,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BasicRules {
    pub case_sensitive: bool,
    pub normalize_full_width: bool,
    pub default_frequency_hz: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct NormalizationRules {
    pub remove_literals: Vec<String>,
    pub remove_patterns: Vec<String>,
    pub roman_number_normalization: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AnalogRules {
    pub voltage_unit_keywords: Vec<String>,
    pub current_unit_keywords: Vec<String>,
    pub dc_keywords: Vec<String>,
    pub phase_a_keywords: Vec<String>,
    pub phase_b_keywords: Vec<String>,
    pub phase_c_keywords: Vec<String>,
    pub phase_n_keywords: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct EquipmentRule {
    pub name: String,
    pub enabled: bool,
    /// bus / line / transformer。断路器模拟量使用 line。
    pub kind: String,
    pub contains: Vec<String>,
    pub patterns: Vec<String>,
    pub exclude_patterns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct StatusRule {
    pub name: String,
    pub enabled: bool,
    pub channel_type: String,
    pub flag: String,
    pub phase: String,
    pub contains: Vec<String>,
    pub patterns: Vec<String>,
    pub exclude_patterns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ReserveRule {
    pub name: String,
    pub enabled: bool,
    pub contains: Vec<String>,
    pub patterns: Vec<String>,
}

/// 模拟量备用通道规则。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AnalogReserveRules {
    pub enabled: bool,
    /// 一次、二次系数均为 1 时视为备用（允许浮点误差）。
    pub ratio_is_one: bool,
    pub empty_name: bool,
    /// 完全匹配的名称（忽略大小写及首尾空白）。
    pub exact_names: Vec<String>,
    /// 名称同时包含空白和数字时视为备用。
    pub space_and_digit: bool,
}

/// 开关量备用通道规则。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct StatusReserveRules {
    pub enabled: bool,
    pub exact_names: Vec<String>,
    pub space_and_digit: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ThresholdRules {
    pub breaker_number_digits_min: usize,
    pub breaker_number_digits_max: usize,
    pub bus_number_digits: usize,
    pub voltage_levels_kv: Vec<f64>,
}

impl Default for RecognitionRules {
    fn default() -> Self {
        Self {
            format_version: RULE_FORMAT_VERSION,
            basic: BasicRules::default(),
            normalization: NormalizationRules::default(),
            analog: AnalogRules::default(),
            // 顺序即优先级，主变必须在线路和母线之前。
            equipment_rules: vec![
                EquipmentRule::new("主变", "transformer", &["主变", "联变", "变压器"]),
                EquipmentRule::new("母线", "bus", &["母线", "母", "母设"]),
                EquipmentRule::new(
                    "线路及断路器模拟量",
                    "line",
                    &["线路", "线", "回", "路", "断路器", "开关"],
                ),
            ],
            status_rules: default_status_rules(),
            reserve_rules: vec![ReserveRule {
                name: "备用通道".into(),
                enabled: true,
                contains: vec![
                    "备用".into(),
                    "备用通道".into(),
                    "备用量".into(),
                    "reserve".into(),
                ],
                patterns: vec![],
            }],
            analog_reserve_rules: AnalogReserveRules::default(),
            status_reserve_rules: StatusReserveRules::default(),
            thresholds: ThresholdRules::default(),
        }
    }
}

impl Default for BasicRules {
    fn default() -> Self {
        Self {
            case_sensitive: false,
            normalize_full_width: true,
            default_frequency_hz: 50.0,
        }
    }
}

impl Default for NormalizationRules {
    fn default() -> Self {
        Self {
            remove_literals: vec![
                "电压".into(),
                "电流".into(),
                "采样值".into(),
                "A相".into(),
                "B相".into(),
                "C相".into(),
                "N相".into(),
                "零序".into(),
                "Ua".into(),
                "Ub".into(),
                "Uc".into(),
                "Un".into(),
                "Ia".into(),
                "Ib".into(),
                "Ic".into(),
                "In".into(),
                "3U0".into(),
                "3I0".into(),
            ],
            remove_patterns: vec![r"^\w+[-_]+".into(), r"[-_\s]+".into()],
            roman_number_normalization: true,
        }
    }
}

impl Default for AnalogRules {
    fn default() -> Self {
        Self {
            voltage_unit_keywords: vec!["V".into(), "kV".into()],
            current_unit_keywords: vec!["A".into(), "kA".into()],
            dc_keywords: vec!["直流".into(), "DC".into()],
            phase_a_keywords: vec!["A相".into(), "Ua".into(), "Ia".into(), "_A".into()],
            phase_b_keywords: vec!["B相".into(), "Ub".into(), "Ib".into(), "_B".into()],
            phase_c_keywords: vec!["C相".into(), "Uc".into(), "Ic".into(), "_C".into()],
            phase_n_keywords: vec![
                "N相".into(),
                "Un".into(),
                "In".into(),
                "零序".into(),
                "3U0".into(),
                "3I0".into(),
            ],
        }
    }
}

impl EquipmentRule {
    fn new(name: &str, kind: &str, contains: &[&str]) -> Self {
        Self {
            name: name.into(),
            enabled: true,
            kind: kind.into(),
            contains: contains.iter().map(|s| (*s).into()).collect(),
            patterns: vec![],
            exclude_patterns: vec![],
        }
    }
}

impl Default for EquipmentRule {
    fn default() -> Self {
        Self::new("新设备规则", "line", &[])
    }
}

impl Default for StatusRule {
    fn default() -> Self {
        Self {
            name: "新开关量规则".into(),
            enabled: true,
            channel_type: "Other".into(),
            flag: "general".into(),
            phase: String::new(),
            contains: vec![],
            patterns: vec![],
            exclude_patterns: vec![],
        }
    }
}

impl Default for ReserveRule {
    fn default() -> Self {
        Self {
            name: "新备用规则".into(),
            enabled: true,
            contains: vec![],
            patterns: vec![],
        }
    }
}

impl Default for AnalogReserveRules {
    fn default() -> Self {
        Self {
            enabled: true,
            ratio_is_one: true,
            empty_name: true,
            exact_names: vec![
                "备用".into(),
                "备用通道".into(),
                "模拟量".into(),
                "电压".into(),
                "电流".into(),
                "电压ua".into(),
                "电压ub".into(),
                "电压uc".into(),
                "电流ia".into(),
                "电流ib".into(),
                "电流ic".into(),
                "ua".into(),
                "ub".into(),
                "uc".into(),
                "ia".into(),
                "ib".into(),
                "ic".into(),
            ],
            space_and_digit: true,
        }
    }
}

impl Default for StatusReserveRules {
    fn default() -> Self {
        Self {
            enabled: true,
            exact_names: vec![
                "备用".into(),
                "备用通道".into(),
                "开关量".into(),
                "开关".into(),
            ],
            space_and_digit: true,
        }
    }
}

impl Default for ThresholdRules {
    fn default() -> Self {
        Self {
            breaker_number_digits_min: 3,
            breaker_number_digits_max: 4,
            bus_number_digits: 1,
            voltage_levels_kv: vec![500.0, 220.0, 110.0, 35.0, 13.8, 10.0],
        }
    }
}

fn status(name: &str, channel_type: &str, flag: &str, phase: &str, words: &[&str]) -> StatusRule {
    StatusRule {
        name: name.into(),
        enabled: true,
        channel_type: channel_type.into(),
        flag: flag.into(),
        phase: phase.into(),
        contains: words.iter().map(|s| (*s).into()).collect(),
        patterns: vec![],
        exclude_patterns: vec![],
    }
}

fn default_status_rules() -> Vec<StatusRule> {
    vec![
        status(
            "A相合位",
            "Breaker",
            "HWJPhsA",
            "A",
            &["A相合位", "A相合闸", "HWJA"],
        ),
        status(
            "B相合位",
            "Breaker",
            "HWJPhsB",
            "B",
            &["B相合位", "B相合闸", "HWJB"],
        ),
        status(
            "C相合位",
            "Breaker",
            "HWJPhsC",
            "C",
            &["C相合位", "C相合闸", "HWJC"],
        ),
        status(
            "A相跳位",
            "Breaker",
            "TWJPhsA",
            "A",
            &["A相跳位", "A相分位", "A相分闸", "TWJA"],
        ),
        status(
            "B相跳位",
            "Breaker",
            "TWJPhsB",
            "B",
            &["B相跳位", "B相分位", "B相分闸", "TWJB"],
        ),
        status(
            "C相跳位",
            "Breaker",
            "TWJPhsC",
            "C",
            &["C相跳位", "C相分位", "C相分闸", "TWJC"],
        ),
        status(
            "A相跳闸",
            "Relay",
            "TrPhsA",
            "A",
            &["跳A", "A跳", "A相跳闸", "跳闸A相"],
        ),
        status(
            "B相跳闸",
            "Relay",
            "TrPhsB",
            "B",
            &["跳B", "B跳", "B相跳闸", "跳闸B相"],
        ),
        status(
            "C相跳闸",
            "Relay",
            "TrPhsC",
            "C",
            &["跳C", "C跳", "C相跳闸", "跳闸C相"],
        ),
        status("重合闸", "Relay", "RecOpCls", "", &["重合"]),
        status("永跳", "Relay", "BlkRec", "", &["永跳", "闭锁重合"]),
        status(
            "三相跳闸",
            "Relay",
            "OpTP",
            "",
            &["三相跳闸", "三跳", "跳三相", "差动跳闸"],
        ),
        status(
            "保护动作",
            "Relay",
            "Tr",
            "",
            &["保护动作", "保护跳闸", "失灵", "重瓦斯", "过激磁", "动作"],
        ),
        status("合位", "Breaker", "HWJ", "", &["合位", "合闸"]),
        status("跳位", "Breaker", "TWJ", "", &["跳位", "分位", "分闸"]),
        status("保护发信", "Relay", "ProtTx", "", &["发信"]),
        status("保护收信", "Relay", "ProtRv", "", &["收信"]),
        status("VT断线", "Warning", "WarnVt", "", &["VT断线", "PT断线"]),
        status("CT断线", "Warning", "WarnCt", "", &["CT断线"]),
        status(
            "通信告警",
            "Warning",
            "WarnComm",
            "",
            &["通道告警", "通信告警"],
        ),
        status("一般告警", "Warning", "WarnGeneral", "", &["告警", "异常"]),
    ]
}

pub fn validate(rules: &RecognitionRules) -> Result<(), String> {
    if rules.format_version > RULE_FORMAT_VERSION {
        return Err(format!(
            "规则格式版本 {} 高于软件支持版本 {}",
            rules.format_version, RULE_FORMAT_VERSION
        ));
    }
    if !rules.basic.default_frequency_hz.is_finite() || rules.basic.default_frequency_hz <= 0.0 {
        return Err("默认频率必须大于 0".into());
    }
    for pattern in rules
        .normalization
        .remove_patterns
        .iter()
        .chain(
            rules
                .equipment_rules
                .iter()
                .flat_map(|r| r.patterns.iter().chain(r.exclude_patterns.iter())),
        )
        .chain(
            rules
                .status_rules
                .iter()
                .flat_map(|r| r.patterns.iter().chain(r.exclude_patterns.iter())),
        )
        .chain(rules.reserve_rules.iter().flat_map(|r| r.patterns.iter()))
    {
        Regex::new(pattern).map_err(|e| format!("正则表达式无效 `{pattern}`: {e}"))?;
    }
    Ok(())
}

/// 严格加载规则。用于人工导入和编辑校验，错误会直接返回。
pub fn load_strict(path: &Path) -> Result<RecognitionRules, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("读取规则失败: {e}"))?;
    let mut rules: RecognitionRules =
        toml::from_str(&text).map_err(|e| format!("解析 TOML 失败: {e}"))?;
    rules = upgrade_rules(rules)?;
    validate(&rules)?;
    Ok(rules)
}

/// 加载规则；配置文件不存在、无法读取、格式错误或版本不支持时使用内置默认规则。
pub fn load_with_status(path: &Path) -> RuleLoadResult {
    if !path.exists() {
        return RuleLoadResult {
            rules: RecognitionRules::default(),
            used_default: true,
            warning: Some("未找到识别规则配置文件，已使用内置默认规则".into()),
        };
    }
    match load_strict(path) {
        Ok(rules) => RuleLoadResult {
            rules,
            used_default: false,
            warning: None,
        },
        Err(message) => RuleLoadResult {
            rules: RecognitionRules::default(),
            used_default: true,
            warning: Some(format!(
                "识别规则配置无法使用，已回退内置默认规则：{message}"
            )),
        },
    }
}

/// 兼容入口：始终返回可用规则，无法识别配置时回退默认规则。
pub fn load(path: &Path) -> Result<RecognitionRules, String> {
    Ok(load_with_status(path).rules)
}

/// 将旧版规则升级到当前内存格式。升级只补充/重命名字段，不覆盖用户已有配置；
/// 新版本发布时在此处追加明确的版本分支，并可由界面另存为新格式。
pub fn upgrade_rules(mut rules: RecognitionRules) -> Result<RecognitionRules, String> {
    if rules.format_version > RULE_FORMAT_VERSION {
        return Err(format!(
            "规则格式版本 {} 高于软件支持版本 {}",
            rules.format_version, RULE_FORMAT_VERSION
        ));
    }
    if rules.format_version < RULE_FORMAT_VERSION {
        // v2 将模拟量和开关量备用判定拆分。反序列化阶段已通过各字段的
        // Default 补齐新结构，此处只提升内存格式版本，不覆盖旧版自定义规则。
        rules.format_version = RULE_FORMAT_VERSION;
    }
    Ok(rules)
}

pub fn save(path: &Path, rules: &RecognitionRules) -> Result<(), String> {
    validate(rules)?;
    let text = toml::to_string_pretty(rules).map_err(|e| format!("序列化 TOML 失败: {e}"))?;
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, text).map_err(|e| format!("写入规则临时文件失败: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("替换规则文件失败: {e}")
    })
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecognitionReport {
    pub analog_count: usize,
    pub status_count: usize,
    pub bus_count: usize,
    pub line_count: usize,
    pub transformer_count: usize,
    pub other_count: usize,
    pub reserve_count: usize,
    #[serde(skip_serializing)]
    pub reserved_analog_indices: Vec<usize>,
    #[serde(skip_serializing)]
    pub reserved_status_indices: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum DeviceKind {
    Bus,
    Line,
    Transformer,
}

#[derive(Debug, Clone, Default)]
struct Group {
    analogs: Vec<(usize, String, bool)>, // idx, phase, voltage
    statuses: Vec<usize>,
}

fn canonical(input: &str, rules: &RecognitionRules) -> String {
    let mut out = if rules.basic.normalize_full_width {
        input
            .replace('Ａ', "A")
            .replace('Ｂ', "B")
            .replace('Ｃ', "C")
            .replace('＿', "_")
    } else {
        input.to_string()
    };
    if !rules.basic.case_sensitive {
        out = out.to_lowercase();
    }
    out
}

fn matches_rule(
    name: &str,
    contains: &[String],
    patterns: &[String],
    excludes: &[String],
    rules: &RecognitionRules,
) -> bool {
    let value = canonical(name, rules);
    if excludes
        .iter()
        .any(|p| Regex::new(p).is_ok_and(|r| r.is_match(&value)))
    {
        return false;
    }
    contains
        .iter()
        .any(|word| value.contains(&canonical(word, rules)))
        || patterns
            .iter()
            .any(|p| Regex::new(p).is_ok_and(|r| r.is_match(&value)))
}

fn is_reserve(name: &str, rules: &RecognitionRules) -> bool {
    rules
        .reserve_rules
        .iter()
        .any(|r| r.enabled && matches_rule(name, &r.contains, &r.patterns, &[], rules))
}

fn has_space_and_digit(name: &str) -> bool {
    let mut has_space = false;
    let mut has_digit = false;
    for ch in name.chars() {
        if ch.is_whitespace() {
            has_space = true;
        } else if ch.is_ascii_digit() {
            has_digit = true;
        } else {
            return false;
        }
    }
    has_space && has_digit
}

fn exact_name(name: &str, names: &[String], rules: &RecognitionRules) -> bool {
    let value = canonical(name.trim(), rules);
    names
        .iter()
        .any(|candidate| value == canonical(candidate.trim(), rules))
}

/// 按模拟量自身字段判断备用通道。规则是 OR 关系：变比为 1、空名称、
/// 约定名称或“空白+数字”任一命中即可；规则可由 TOML 覆盖。
pub fn is_reserved_analog(channel: &comtrade_io::AnalogChannel, rules: &RecognitionRules) -> bool {
    if !rules.analog_reserve_rules.enabled {
        return false;
    }
    let reserve = &rules.analog_reserve_rules;
    let ratio_one = channel.secondary.abs() > f64::EPSILON
        && (channel.primary / channel.secondary - 1.0).abs() <= 1e-9;
    (reserve.ratio_is_one && ratio_one)
        || (reserve.empty_name && channel.name.trim().is_empty())
        || exact_name(&channel.name, &reserve.exact_names, rules)
        || (reserve.space_and_digit && has_space_and_digit(&channel.name))
        || is_reserve(&channel.name, rules)
}

/// 按开关量名称判断备用通道。
pub fn is_reserved_status(channel: &comtrade_io::StatusChannel, rules: &RecognitionRules) -> bool {
    if !rules.status_reserve_rules.enabled {
        return false;
    }
    let reserve = &rules.status_reserve_rules;
    exact_name(&channel.name, &reserve.exact_names, rules)
        || (reserve.space_and_digit && has_space_and_digit(&channel.name))
        || is_reserve(&channel.name, rules)
}

/// 使用默认规则判断 CFG 通道是否为备用通道，供波形默认选择使用。
pub fn is_reserved_name(name: &str) -> bool {
    let rules = RecognitionRules::default();
    exact_name(name, &rules.analog_reserve_rules.exact_names, &rules)
        || exact_name(name, &rules.status_reserve_rules.exact_names, &rules)
        || has_space_and_digit(name)
        || is_reserve(name, &rules)
}

fn infer_phase(name: &str, configured: &str, rules: &RecognitionRules) -> String {
    if !configured.trim().is_empty() {
        return configured.to_uppercase();
    }
    let value = canonical(name, rules);
    for (phase, words) in [
        ("A", &rules.analog.phase_a_keywords),
        ("B", &rules.analog.phase_b_keywords),
        ("C", &rules.analog.phase_c_keywords),
        ("N", &rules.analog.phase_n_keywords),
    ] {
        if words.iter().any(|w| value.contains(&canonical(w, rules))) {
            return phase.into();
        }
    }
    String::new()
}

fn analog_metadata(name: &str, unit: &str, rules: &RecognitionRules) -> (String, String) {
    let value = canonical(&format!("{} {}", name, unit), rules);
    let unit_value = canonical(unit.trim(), rules);
    let keyword_matches = |keywords: &[String]| {
        keywords.iter().any(|word| {
            let keyword = canonical(word.trim(), rules);
            if keyword.is_empty() {
                false
            } else if keyword.chars().count() <= 1 {
                unit_value == keyword
            } else {
                value.contains(&keyword)
            }
        })
    };
    let name_value = canonical(name, rules);
    let is_voltage = keyword_matches(&rules.analog.voltage_unit_keywords)
        || unit_value.ends_with('v')
        || ["ua", "ub", "uc", "un", "ul", "3u0", "电压"]
            .iter()
            .any(|suffix| name_value.ends_with(suffix) || name_value.contains(suffix));
    let is_current = keyword_matches(&rules.analog.current_unit_keywords)
        || unit_value.ends_with('a')
        || ["ia", "ib", "ic", "in", "3i0", "电流"]
            .iter()
            .any(|suffix| name_value.ends_with(suffix) || name_value.contains(suffix));
    let is_dc = rules
        .analog
        .dc_keywords
        .iter()
        .any(|word| value.contains(&canonical(word, rules)));

    if is_voltage {
        (
            if is_dc { "D" } else { "A" }.into(),
            if is_dc { "DV" } else { "ACV" }.into(),
        )
    } else if is_current {
        (
            if is_dc { "D" } else { "A" }.into(),
            if is_dc { "DA" } else { "ACC" }.into(),
        )
    } else {
        ("O".into(), "general".into())
    }
}

fn status_metadata(name: &str, rules: &RecognitionRules) -> (String, String, String) {
    rules
        .status_rules
        .iter()
        .find(|rule| {
            rule.enabled
                && matches_rule(
                    name,
                    &rule.contains,
                    &rule.patterns,
                    &rule.exclude_patterns,
                    rules,
                )
        })
        .map(|rule| {
            (
                rule.channel_type.clone(),
                rule.flag.clone(),
                rule.phase.clone(),
            )
        })
        .unwrap_or_else(|| ("Other".into(), "general".into(), String::new()))
}

/// 使用内置默认规则推断模拟量的 DMF 类型和标识。
///
/// 用于没有 DMF/INF 扩展字段时的最小会话模型；用户保存的规则仍由
/// `generate` 在“重新识别”流程中统一应用。
pub fn infer_default_analog_metadata(name: &str, unit: &str) -> (String, String) {
    analog_metadata(name, unit, &RecognitionRules::default())
}

/// 使用内置默认规则推断开关量的 DMF 类型、标识和相别。
pub fn infer_default_status_metadata(name: &str) -> (String, String, String) {
    status_metadata(name, &RecognitionRules::default())
}

fn normalized_name(name: &str, equipment: &str, rules: &RecognitionRules) -> String {
    let mut out = if equipment.trim().is_empty() {
        name.to_string()
    } else {
        equipment.to_string()
    };
    for literal in &rules.normalization.remove_literals {
        out = out.replace(literal, "");
        out = out.replace(&literal.to_lowercase(), "");
    }
    for pattern in &rules.normalization.remove_patterns {
        if let Ok(regex) = Regex::new(pattern) {
            out = regex.replace_all(&out, "").into_owned();
        }
    }
    if rules.normalization.roman_number_normalization {
        out = out
            .replace('Ⅰ', "1")
            .replace('Ⅱ', "2")
            .replace('Ⅲ', "3")
            .replace('Ⅳ', "4");
    }
    let trimmed = out.trim().to_string();
    if trimmed.is_empty() {
        name.trim().to_string()
    } else {
        trimmed
    }
}

/// 提取通道名称中用于跨模拟量/开关量关联的设备锚点。
///
/// 常见录波命名以 `设备编号_...` 或 `设备编号 ...` 开头；IEC 参引已在
/// CFG 解析层从 equipment 字段分离，因此这里只比较名称前缀，避免把
/// `PTRC$ST$...` 误当作被监视元件。无法提取时返回完整规范化名称。
fn equipment_anchor(name: &str, rules: &RecognitionRules) -> String {
    let trimmed = name.trim();
    let anchor = trimmed
        .split_once('_')
        .map(|(prefix, _)| prefix)
        .or_else(|| trimmed.split_whitespace().next())
        .unwrap_or(trimmed);
    canonical(anchor, rules)
}

fn device_kind(name: &str, rules: &RecognitionRules) -> Option<DeviceKind> {
    rules
        .equipment_rules
        .iter()
        .filter(|r| r.enabled)
        .find_map(|r| {
            if !matches_rule(name, &r.contains, &r.patterns, &r.exclude_patterns, rules) {
                return None;
            }
            match r.kind.to_ascii_lowercase().as_str() {
                "bus" => Some(DeviceKind::Bus),
                "line" => Some(DeviceKind::Line),
                "transformer" => Some(DeviceKind::Transformer),
                _ => None,
            }
        })
}

fn assign_acv(acv: &mut AcvChn, phase: &str, idx: usize) {
    match phase {
        "A" => acv.ua_idx = idx,
        "B" => acv.ub_idx = idx,
        "C" => acv.uc_idx = idx,
        "L" => acv.ul_idx = idx,
        _ => acv.un_idx = idx,
    }
}

fn assign_acc(acc: &mut AccBran, phase: &str, idx: usize) {
    match phase {
        "A" => acc.ia_idx = idx,
        "B" => acc.ib_idx = idx,
        "C" => acc.ic_idx = idx,
        _ => acc.in_idx = idx,
    }
}

fn transformer_side(name: &str) -> &'static str {
    if name.contains('高') {
        "High"
    } else if name.contains('中') {
        "Medium"
    } else {
        "Low"
    }
}

/// 依据规则生成会话 CFG 覆盖值和 DMF。备用通道保留 CFG、从 DMF 排除。
pub fn generate(
    cfg: &Config,
    rules: &RecognitionRules,
) -> Result<(Config, DmfFile, RecognitionReport), String> {
    validate(rules)?;
    let mut cfg_out = cfg.clone();
    let mut dmf = DmfFile {
        station_name: cfg.header.station.clone(),
        rec_dev_name: cfg.header.recorder.clone(),
        version: "1.0".into(),
        reference: "0".into(),
        ..Default::default()
    };
    let mut groups: BTreeMap<(DeviceKind, String), Group> = BTreeMap::new();
    let mut group_anchors: BTreeMap<String, (DeviceKind, String)> = BTreeMap::new();
    let mut report = RecognitionReport::default();

    for ch in &mut cfg_out.analogs {
        if is_reserved_analog(ch, rules) {
            report.reserve_count += 1;
            report.reserved_analog_indices.push(ch.index);
            continue;
        }
        ch.phase = infer_phase(&ch.name, &ch.phase, rules);
        let (ch_type, flag) = analog_metadata(&ch.name, &ch.unit, rules);
        let is_voltage = flag == "ACV" || flag == "DV";
        let is_ac = ch_type == "A";
        let kind = device_kind(&format!("{} {}", ch.name, ch.equipment), rules);
        if let Some(kind) = kind {
            let group_name = normalized_name(&ch.name, &ch.equipment, rules);
            if ch.equipment.trim().is_empty() {
                ch.equipment = group_name.clone();
            }
            let key = (kind, group_name.clone());
            group_anchors
                .entry(equipment_anchor(&ch.name, rules))
                .or_insert_with(|| key.clone());
            groups
                .entry(key)
                .or_default()
                .analogs
                .push((ch.index, ch.phase.clone(), is_voltage));
        }
        if ch_type == "O" || kind.is_none() {
            report.other_count += 1;
        }
        dmf.analogs.push(DmfAnalogChannel {
            idx_cfg: ch.index,
            idx_org: ch.index,
            ch_type,
            flag,
            freq: if cfg.sampling.freq > 0.0 {
                cfg.sampling.freq
            } else {
                rules.basic.default_frequency_hz
            },
            // 交流电压/电流的标幺换算默认采用单位增益、零偏移。
            // 直流和无法识别通道不自动填充工程参数。
            au: if is_ac { 1.0 } else { 0.0 },
            bu: 0.0,
            unit: ch.unit.clone(),
            multiplier: ch.multiplier,
            primary: ch.primary,
            secondary: ch.secondary,
            ps: ch.tran_side.as_str().into(),
            idx_rlt: 0,
            ph: ch.phase.clone(),
        });
        report.analog_count += 1;
    }

    let group_names: Vec<_> = groups.keys().cloned().collect();
    for ch in &mut cfg_out.statuses {
        if is_reserved_status(ch, rules) {
            report.reserve_count += 1;
            report.reserved_status_indices.push(ch.index);
            continue;
        }
        // CCBM 位置可能直接承载 IEC 61850 参引。将其从被监视元件
        // 中移出，确保既不会参与设备归组，也不会在生成 DMF 时丢失。
        let ccbm_reference = if ch
            .ext
            .as_ref()
            .and_then(|ext| ext.reference.clone())
            .is_none()
            && is_iec61850_reference(&ch.equipment)
        {
            let reference = std::mem::take(&mut ch.equipment);
            ch.ext.get_or_insert_with(Default::default).reference = Some(reference.clone());
            Some(reference)
        } else {
            None
        };
        let (channel_type, flag, phase) = status_metadata(&ch.name, rules);
        if channel_type == "Other" {
            report.other_count += 1;
        }
        if ch.phase.trim().is_empty() {
            ch.phase = if phase.is_empty() {
                infer_phase(&ch.name, "", rules)
            } else {
                phase
            };
        }
        if ch.equipment.trim().is_empty() {
            let normalized = normalized_name(&ch.name, "", rules);
            if let Some((_, device)) = group_names.iter().find(|(_, device)| {
                canonical(&normalized, rules).contains(&canonical(device, rules))
                    || canonical(&ch.name, rules).contains(&canonical(device, rules))
            }) {
                ch.equipment = device.clone();
            }
            // 名称包含相同设备编号时优先使用模拟量已建立的分组，
            // 比较完整通道名更稳定（如 `000F#001_...`）。
            if ch.equipment.trim().is_empty() {
                if let Some((_, device)) = group_anchors.get(&equipment_anchor(&ch.name, rules)) {
                    ch.equipment = device.clone();
                }
            }
        }
        if !ch.equipment.is_empty() {
            if let Some(key) = group_names
                .iter()
                .find(|(_, name)| canonical(name, rules) == canonical(&ch.equipment, rules))
            {
                groups
                    .entry(key.clone())
                    .or_default()
                    .statuses
                    .push(ch.index);
            }
        }
        dmf.statuses.push(DmfStatusChannel {
            idx_cfg: ch.index,
            idx_org: ch.index,
            ch_type: channel_type,
            flag,
            contact: if ch.contact == 0 {
                "NormallyOpen".into()
            } else {
                "NormallyClosed".into()
            },
            // 非法 CCBM 被 CFG 解析层移入 StatusExt.reference；它是 IEC 61850
            // 参引，不得重新作为被监视元件写回。
            src_ref: ccbm_reference
                .or_else(|| ch.ext.as_ref().and_then(|ext| ext.reference.clone()))
                .unwrap_or_default(),
        });
        report.status_count += 1;
    }

    let mut next_bus = 1usize;
    let mut next_line = 1usize;
    let mut next_transformer = 1usize;
    for ((kind, name), group) in groups {
        match kind {
            DeviceKind::Bus => {
                let mut bus = Bus {
                    idx: next_bus,
                    name,
                    tv_pos: "BUS".into(),
                    sta_chns: group.statuses,
                    ..Default::default()
                };
                for (idx, phase, voltage) in group.analogs {
                    if voltage {
                        assign_acv(&mut bus.acv, &phase, idx);
                    } else {
                        bus.ana_chns.push(idx);
                    }
                }
                dmf.buses.push(bus);
                next_bus += 1;
            },
            DeviceKind::Line => {
                let mut line = Line {
                    idx: next_line,
                    name,
                    sta_chns: group.statuses,
                    ..Default::default()
                };
                let mut current = AccBran {
                    bran_idx: 1,
                    ..Default::default()
                };
                let mut voltage = AcvChn::default();
                for (idx, phase, is_voltage) in group.analogs {
                    if is_voltage {
                        assign_acv(&mut voltage, &phase, idx);
                    } else {
                        assign_acc(&mut current, &phase, idx);
                    }
                    line.ana_chns.push(idx);
                }
                if !current.is_empty() {
                    line.currents.push(current);
                    line.bran_num = 1;
                }
                if !voltage.is_empty() {
                    line.voltages.push(voltage);
                }
                dmf.lines.push(line);
                next_line += 1;
            },
            DeviceKind::Transformer => {
                let mut transformer = Transformer {
                    idx: next_transformer,
                    name: name.clone(),
                    object_type: "MAIN".into(),
                    sta_chns: group.statuses,
                    ..Default::default()
                };
                let mut sides: BTreeMap<String, TransformerWinding> = BTreeMap::new();
                for (idx, phase, is_voltage) in group.analogs {
                    let side = transformer_side(&name).to_string();
                    let winding = sides
                        .entry(side.clone())
                        .or_insert_with(|| TransformerWinding {
                            location: side,
                            ..Default::default()
                        });
                    if is_voltage {
                        assign_acv(&mut winding.acv, &phase, idx);
                    } else {
                        if winding.currents.is_empty() {
                            winding.currents.push(AccBran {
                                bran_idx: 1,
                                ..Default::default()
                            });
                        }
                        assign_acc(&mut winding.currents[0], &phase, idx);
                        winding.bran_num = 1;
                    }
                    transformer.ana_chns.push(idx);
                }
                transformer.windings = sides.into_values().collect();
                transformer.winding_num = transformer.windings.len();
                dmf.transformers.push(transformer);
                next_transformer += 1;
            },
        }
    }
    report.bus_count = dmf.buses.len();
    report.line_count = dmf.lines.len();
    report.transformer_count = dmf.transformers.len();
    Ok((cfg_out, dmf, report))
}

#[cfg(test)]
mod tests {
    use super::*;
    use comtrade_io::{AnalogChannel, StatusChannel};

    #[test]
    fn default_rules_round_trip_toml() {
        let rules = RecognitionRules::default();
        let text = toml::to_string_pretty(&rules).unwrap();
        let parsed: RecognitionRules = toml::from_str(&text).unwrap();
        validate(&parsed).unwrap();
        assert_eq!(parsed.format_version, RULE_FORMAT_VERSION);
        assert!(!parsed.status_rules.is_empty());
    }

    #[test]
    fn reserve_rules_are_case_insensitive() {
        let rules = RecognitionRules::default();
        assert!(is_reserve("RESERVE CH 1", &rules));
        assert!(is_reserve("备用通道01", &rules));
    }

    #[test]
    fn generation_excludes_reserve_and_maps_unknown_channels_to_other() {
        let mut cfg = Config::default();
        cfg.analogs = vec![
            AnalogChannel {
                index: 1,
                name: "备用通道01".into(),
                ..Default::default()
            },
            AnalogChannel {
                index: 2,
                name: "温度量".into(),
                unit: "degC".into(),
                primary: 100.0,
                ..Default::default()
            },
        ];
        cfg.statuses = vec![
            StatusChannel {
                index: 1,
                name: "RESERVE DI".into(),
                ..Default::default()
            },
            StatusChannel {
                index: 2,
                name: "自定义接点".into(),
                ..Default::default()
            },
        ];

        let (_, dmf, report) = generate(&cfg, &RecognitionRules::default()).unwrap();

        assert_eq!(dmf.analogs.len(), 1);
        assert_eq!(dmf.analogs[0].idx_cfg, 2);
        assert_eq!(dmf.analogs[0].ch_type, "O");
        assert_eq!(dmf.analogs[0].flag, "general");
        assert_eq!(dmf.statuses.len(), 1);
        assert_eq!(dmf.statuses[0].idx_cfg, 2);
        assert_eq!(dmf.statuses[0].ch_type, "Other");
        assert_eq!(report.reserve_count, 2);
        assert_eq!(report.reserved_analog_indices, vec![1]);
        assert_eq!(report.reserved_status_indices, vec![1]);
        assert_eq!(report.other_count, 2);
    }

    #[test]
    fn ac_channels_default_to_unit_au_and_zero_bu() {
        let mut cfg = Config::default();
        cfg.analogs = vec![
            AnalogChannel {
                index: 1,
                name: "线路Ua".into(),
                unit: "V".into(),
                primary: 1100.0,
                ..Default::default()
            },
            AnalogChannel {
                index: 2,
                name: "线路Ia".into(),
                unit: "A".into(),
                primary: 600.0,
                ..Default::default()
            },
        ];

        let (_, dmf, _) = generate(&cfg, &RecognitionRules::default()).unwrap();
        assert_eq!(dmf.analogs.len(), 2);
        for channel in &dmf.analogs {
            assert_eq!(channel.ch_type, "A");
            assert_eq!(channel.au, 1.0);
            assert_eq!(channel.bu, 0.0);
            assert_eq!(channel.idx_rlt, 0);
        }
    }

    #[test]
    fn analog_and_status_reserve_rules_are_independent() {
        let rules = RecognitionRules::default();
        for name in ["", "备用", "模拟量", "电压Ua", "电流Ic", " 12 "] {
            let channel = AnalogChannel {
                name: name.into(),
                primary: 100.0,
                secondary: 1.0,
                ..Default::default()
            };
            assert!(is_reserved_analog(&channel, &rules), "analog name={name:?}");
        }
        let ratio_one = AnalogChannel {
            name: "正常命名".into(),
            primary: 5.0,
            secondary: 5.0,
            ..Default::default()
        };
        assert!(is_reserved_analog(&ratio_one, &rules));

        for name in ["备用", "备用通道", "开关量", "开关", " 07 "] {
            let channel = StatusChannel {
                name: name.into(),
                ..Default::default()
            };
            assert!(is_reserved_status(&channel, &rules), "status name={name:?}");
        }
        assert!(!is_reserved_status(
            &StatusChannel {
                name: "220kV 1号断路器合位".into(),
                ..Default::default()
            },
            &rules
        ));
    }

    #[test]
    fn invalid_or_missing_config_uses_default_rules() {
        let missing = std::env::temp_dir().join("comtrade-recognition-missing-rules.toml");
        let _ = std::fs::remove_file(&missing);
        let loaded = load_with_status(&missing);
        assert!(loaded.used_default);

        let invalid = std::env::temp_dir().join("comtrade-recognition-invalid-rules.toml");
        std::fs::write(&invalid, "not = [valid").unwrap();
        let loaded = load_with_status(&invalid);
        let _ = std::fs::remove_file(&invalid);
        assert!(loaded.used_default);
        assert!(loaded.warning.unwrap().contains("回退内置默认规则"));
    }

    #[test]
    fn version_one_config_is_upgraded_with_split_reserve_defaults() {
        let legacy = std::env::temp_dir().join("comtrade-recognition-v1-rules.toml");
        std::fs::write(&legacy, "formatVersion = 1\n").unwrap();
        let rules = load_strict(&legacy).unwrap();
        let _ = std::fs::remove_file(&legacy);
        assert_eq!(rules.format_version, RULE_FORMAT_VERSION);
        assert!(rules.analog_reserve_rules.ratio_is_one);
        assert!(rules.status_reserve_rules.enabled);
    }

    #[test]
    fn generated_status_keeps_ccbm_reference_out_of_equipment() {
        let mut cfg = Config::default();
        cfg.statuses = vec![StatusChannel {
            index: 1,
            name: "状态1".into(),
            equipment: String::new(),
            ext: Some(comtrade_io::StatusExt {
                reference: Some("SVOUTMUSV$TCTR1$MX$AmpR".into()),
                ..Default::default()
            }),
            ..Default::default()
        }];
        let (_, dmf, _) = generate(&cfg, &RecognitionRules::default()).unwrap();
        assert_eq!(dmf.statuses[0].src_ref, "SVOUTMUSV$TCTR1$MX$AmpR");
        assert!(cfg.statuses[0].equipment.is_empty());
    }
}
