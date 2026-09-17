//! INF 文本序列化。

use super::section::InfFile;

impl InfFile {
    /// 序列化为 INF 文本
    #[allow(clippy::inherent_to_string)]
    pub fn to_string(&self) -> String {
        let mut lines = Vec::new();
        for section in &self.sections {
            lines.push(section.to_text());
            lines.push(String::new());
        }
        lines.join("\n")
    }
}
