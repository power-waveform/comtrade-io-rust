//! INF 节数据结构

/// 节类型枚举
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SectionKind {
    /// 文件描述节 `[Area File_Description]`
    FileDescription,
    /// 模拟量通道节 `[Area Analog_Channel_#N]`
    AnalogChannel,
    /// 状态量通道节 `[Area Status_Channel_#N]`
    StatusChannel,
    /// 模拟量通道参数节 `[Area Analog_Channels_Parameter]`
    AnalogChannelsParameter,
    /// 状态量通道参数节 `[Area Status_Channels_Parameter]`
    StatusChannelsParameter,
    /// 母线节 `[Area Bus_#N]`
    Bus,
    /// 线路节 `[Area Line_#N]`
    Line,
    /// 变压器节 `[Area Transformer_#N]`
    Transformer,
    /// 发电机（规范 §1.6 `[ZYHD POWER_#n]`）
    Generator,
    /// 励磁机（规范 §1.7 `[ZYHD EXCITATION_#n]`）
    Exciter,
    /// 录波信息节 `[Area Record_Information]`
    RecordInformation,
    /// 未建模的其他节，保留原始节名
    Other(String),
}

/// 一个 INF 节
#[derive(Debug, Clone)]
pub struct Section {
    /// 节所属区域名
    pub area: String,
    /// 节类型
    pub kind: SectionKind,
    /// 节序号（无编号时为 0）
    pub index: usize,
    /// 键值对字段列表（按出现顺序）
    pub fields: Vec<(String, String)>,
    /// 节头的原始文本
    pub raw: String,
}

/// INF 文件容器
#[derive(Debug, Clone, Default)]
pub struct InfFile {
    /// 所有节的列表（按文件出现顺序）
    pub sections: Vec<Section>,
}

impl Section {
    /// 大小写不敏感查 Key
    pub fn get(&self, key: &str) -> Option<&str> {
        let key_lower = key.to_lowercase();
        self.fields
            .iter()
            .find(|(k, _)| k.to_lowercase() == key_lower)
            .map(|(_, v)| v.as_str())
    }

    /// 序列化为文本
    pub fn to_text(&self) -> String {
        let type_name = match &self.kind {
            SectionKind::FileDescription => "File_Description".to_string(),
            SectionKind::AnalogChannel => "Analog_Channel".to_string(),
            SectionKind::StatusChannel => "Status_Channel".to_string(),
            SectionKind::AnalogChannelsParameter => "Analog_Channels_Parameter".to_string(),
            SectionKind::StatusChannelsParameter => "Status_Channels_Parameter".to_string(),
            SectionKind::Bus => "Bus".to_string(),
            SectionKind::Line => "Line".to_string(),
            SectionKind::Transformer => "Transformer".to_string(),
            SectionKind::Generator => "Power".to_string(),
            SectionKind::Exciter => "Excitation".to_string(),
            SectionKind::RecordInformation => "Record_Information".to_string(),
            SectionKind::Other(name) => name.clone(),
        };

        let header = if self.index > 0 {
            format!("[{} {}_#{}]", self.area, type_name, self.index)
        } else {
            format!("[{} {}]", self.area, type_name)
        };

        let mut lines = vec![header];
        for (key, value) in &self.fields {
            lines.push(format!("{}={}", key, value));
        }
        lines.join("\n")
    }
}

/// 解析 INF 文本为节列表
pub fn parse_sections(text: &str) -> InfFile {
    let mut sections = Vec::new();
    let mut current_section: Option<Section> = None;

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            if let Some(section) = current_section.take() {
                sections.push(section);
            }
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            // 保存前一个节
            if let Some(section) = current_section.take() {
                sections.push(section);
            }
            // 解析新节头
            let inner = &line[1..line.len() - 1];
            let (area, kind, index) = parse_section_header(inner);
            current_section = Some(Section {
                area,
                kind,
                index,
                fields: Vec::new(),
                raw: line.to_string(),
            });
        } else if let Some(ref mut section) = current_section {
            // Key=Value 行
            if let Some(eq_pos) = line.find('=') {
                let key = line[..eq_pos].trim().to_string();
                let value = line[eq_pos + 1..].trim().to_string();
                section.fields.push((key, value));
            }
        }
    }

    // 保存最后一个节
    if let Some(section) = current_section {
        sections.push(section);
    }

    InfFile { sections }
}

/// 解析节头 "[Area Type_#N]" 或 "[Area Type]"
fn parse_section_header(inner: &str) -> (String, SectionKind, usize) {
    let parts: Vec<&str> = inner.splitn(2, ' ').collect();
    let area = parts.first().unwrap_or(&"").to_string();
    let rest = parts.get(1).unwrap_or(&"");

    // 提取 index
    let (type_part, index) = if let Some(hash_pos) = rest.rfind("_#") {
        let type_name = &rest[..hash_pos];
        let idx = rest[hash_pos + 2..].parse::<usize>().unwrap_or(0);
        (type_name.to_string(), idx)
    } else {
        (rest.to_string(), 0)
    };

    let kind = match type_part.to_uppercase().as_str() {
        "FILE_DESCRIPTION" => SectionKind::FileDescription,
        "ANALOG_CHANNEL" | "ANALOG_CHANNELS" => SectionKind::AnalogChannel,
        "STATUS_CHANNEL" | "STATUS_CHANNELS" => SectionKind::StatusChannel,
        "ANALOG_CHANNELS_PARAMETER" => SectionKind::AnalogChannelsParameter,
        "STATUS_CHANNELS_PARAMETER" => SectionKind::StatusChannelsParameter,
        "BUS" => SectionKind::Bus,
        "LINE" => SectionKind::Line,
        "TRANSFORMER" => SectionKind::Transformer,
        "POWER" => SectionKind::Generator,
        "EXCITATION" => SectionKind::Exciter,
        "RECORD_INFORMATION" => SectionKind::RecordInformation,
        // 偏离 Python 基线：未建模的节名保留**原始大小写**，不写回大写形式。
        // 原实现存 `to_uppercase()` 的结果，序列化时把 `ZprdInfo` / `Channels_Group`
        // 等 32 个节名整体改写成 `ZPRDINFO` / `CHANNELS_GROUP`，破坏 round-trip。
        // 匹配仍走大写比较，只有存储用原文。
        _ => SectionKind::Other(type_part.clone()),
    };

    (area, kind, index)
}

impl InfFile {
    /// 按类型筛选节
    pub fn sections_of(&self, kind: &SectionKind) -> Vec<&Section> {
        self.sections
            .iter()
            .filter(|s| std::mem::discriminant(&s.kind) == std::mem::discriminant(kind))
            .collect()
    }

    /// 获取 File_Description 节
    pub fn file_description(&self) -> Option<&Section> {
        self.sections
            .iter()
            .find(|s| matches!(s.kind, SectionKind::FileDescription))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_other_section_name_preserves_case() {
        // 未建模节名必须逐字保留，不能被大写化
        let (area, kind, idx) = parse_section_header("ZYHD ZprdInfo");
        assert_eq!(area, "ZYHD");
        assert_eq!(idx, 0);
        assert_eq!(kind, SectionKind::Other("ZprdInfo".into()));

        let (_, kind, idx) = parse_section_header("ZYHD Channels_Group_#12");
        assert_eq!(kind, SectionKind::Other("Channels_Group".into()));
        assert_eq!(idx, 12);
    }

    #[test]
    fn test_known_section_kinds_are_case_insensitive() {
        // 已建模节的匹配仍不区分大小写
        for name in ["ZYHD Bus_#1", "ZYHD BUS_#1", "ZYHD bus_#1"] {
            let (_, kind, idx) = parse_section_header(name);
            assert_eq!(kind, SectionKind::Bus, "{}", name);
            assert_eq!(idx, 1);
        }
        assert_eq!(
            parse_section_header("ZYHD Transformer_#1").1,
            SectionKind::Transformer
        );
        // 发电机段 / 励磁机段（规范 §1.6 / §1.7）
        for name in ["ZYHD Power_#1", "ZYHD POWER_#1", "ZYHD power_#1"] {
            let (_, kind, idx) = parse_section_header(name);
            assert_eq!(kind, SectionKind::Generator, "{}", name);
            assert_eq!(idx, 1);
        }
        for name in ["ZYHD Excitation_#1", "ZYHD EXCITATION_#1"] {
            let (area, kind, idx) = parse_section_header(name);
            assert_eq!(area, "ZYHD");
            assert_eq!(kind, SectionKind::Exciter, "{}", name);
            assert_eq!(idx, 1);
        }
        assert_eq!(
            parse_section_header("Public File_Description").1,
            SectionKind::FileDescription
        );
    }
}
