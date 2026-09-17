//! CFF 字节级段切分
//!
//! 在字节流上定位 `--- file type: XXX ---` 标记，段文本按 UTF-8 优先、
//! 失败回退 GBK 的策略解码（与 CFG/INF 的 `read_text_gbk` 行为对齐）。
//! BINARY/FLOAT32 DAT 段保留原始字节，不做文本解码也不做空白裁剪。

/// CFF 段内容
#[derive(Debug, Clone, Default)]
pub struct CffSections {
    /// CFG 段文本
    pub cfg: Option<String>,
    /// DAT 段文本（ASCII 格式）
    pub dat: Option<String>,
    /// DAT 段原始字节（BINARY/FLOAT32 格式）
    pub dat_bytes: Option<Vec<u8>>,
    /// INF 段文本
    pub inf: Option<String>,
    /// HDR 段文本
    pub hdr: Option<String>,
}

/// 提取 CFF 段
pub fn extract_sections(data: &[u8]) -> CffSections {
    let mut result = CffSections::default();
    let markers = find_markers(data);

    for &(section_type, start, end) in &markers {
        let section_bytes = &data[start..end];

        match section_type {
            "CFG" => {
                result.cfg = Some(decode_text(trim_bytes(section_bytes)));
            },
            "DAT" => {
                // 文本（ASCII）DAT 段：解码后供 `DatFile::from_ascii` 使用
                result.dat = Some(decode_text(trim_bytes(section_bytes)));
                // 二进制 DAT 段：保留**未裁剪**的原始字节。
                // BINARY/FLOAT32 采样字节合法地等于 0x20/0x09/0x0A/0x0D，
                // 两端裁剪会静默吃掉首尾样本并移位后续全部数据。
                result.dat_bytes = Some(section_bytes.to_vec());
            },
            "INF" => {
                result.inf = Some(decode_text(trim_bytes(section_bytes)));
            },
            "HDR" => {
                result.hdr = Some(decode_text(trim_bytes(section_bytes)));
            },
            _ => {},
        }
    }

    result
}

/// 查找所有段标记，返回 (类型, 内容开始, 内容结束)
fn find_markers(data: &[u8]) -> Vec<(&str, usize, usize)> {
    let mut markers = Vec::new();
    let mut pos = 0;

    while pos < data.len() {
        // 查找行首的 "--" 或 "---"
        let is_start = pos == 0 || data[pos - 1] == b'\n';

        if is_start && pos + 1 < data.len() && data[pos] == b'-' && data[pos + 1] == b'-' {
            // 查找这一行的结束
            let mut line_end = pos;
            while line_end < data.len() && data[line_end] != b'\n' && data[line_end] != b'\r' {
                line_end += 1;
            }
            let line = &data[pos..line_end];

            // 尝试匹配 "--- file type: XXX ---" 模式
            if let Some(section_type) = match_section_line(line) {
                // 跳过行尾换行符
                let content_start = skip_newlines(data, line_end);
                markers.push((section_type, content_start, data.len())); // 先暂存 end
                if markers.len() > 1 {
                    // 更新前一个标记的 end
                    let prev_end = pos;
                    let prev_idx = markers.len() - 2;
                    markers[prev_idx].2 = prev_end;
                }
            }
        }

        pos += 1;
    }

    // 最后一个标记的 end 已经是 data.len()
    markers
}

/// 匹配段标记行，返回段类型
fn match_section_line(line: &[u8]) -> Option<&str> {
    let text = std::str::from_utf8(line).ok()?;
    let lower = text.trim().to_lowercase();

    // 必须以 -- 或 --- 开头，以 --- 结尾
    if !lower.starts_with("--") || !lower.ends_with("---") {
        return None;
    }

    // 去掉开头的 -- 或 ---
    let inner = if let Some(stripped) = lower.strip_prefix("---") {
        stripped
    } else {
        lower.strip_prefix("--").unwrap_or(&lower)
    };
    let inner = inner[..inner.len() - 3].trim();
    if !inner.contains("file") || !inner.contains("type") {
        return None;
    }

    // 提取段类型
    for part in inner.split_whitespace() {
        let cleaned = part.trim_end_matches(':');
        match cleaned.to_uppercase().as_str() {
            "CFG" => return Some("CFG"),
            "DAT" => return Some("DAT"),
            "INF" => return Some("INF"),
            "HDR" => return Some("HDR"),
            _ => {},
        }
    }

    // 在 "file type" 后面找类型
    if let Some(type_pos) = inner.find("type") {
        let after = &inner[type_pos + 4..].trim();
        let after = after.trim_start_matches(':').trim();
        let word = after.split_whitespace().next().unwrap_or("");
        match word.to_uppercase().as_str() {
            "CFG" => return Some("CFG"),
            "DAT" => return Some("DAT"),
            "INF" => return Some("INF"),
            "HDR" => return Some("HDR"),
            _ => {},
        }
    }

    None
}

fn skip_newlines(data: &[u8], pos: usize) -> usize {
    let mut p = pos;
    while p < data.len() && (data[p] == b'\n' || data[p] == b'\r') {
        p += 1;
    }
    p
}

fn trim_bytes(data: &[u8]) -> &[u8] {
    let mut start = 0;
    let mut end = data.len();
    while start < end && data[start].is_ascii_whitespace() {
        start += 1;
    }
    while end > start && data[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    &data[start..end]
}

/// 按段独立探测编码并解码：UTF-8 严格校验优先，失败回退 GBK。
///
/// 必须**按段**探测而非对整个文件一次性判定——BINARY DAT 段永远不是合法
/// UTF-8，若参与全局判定会把 CFG 段的 UTF-8 中文拖成 GBK 误读。
fn decode_text(data: &[u8]) -> String {
    cbase::encoding::decode(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_cff_sections() {
        let data = b"--- file type CFG ---\nSTATION,RECORDER,1999\n...\n--- file type DAT ---\n1,0,100,200\n--- file type INF ---\n[Public File_Description]\nStation_Name=TEST\n";
        let sections = extract_sections(data);
        assert!(sections.cfg.is_some());
        assert!(sections.dat.is_some());
        assert!(sections.inf.is_some());
    }

    #[test]
    fn test_dat_bytes_not_trimmed() {
        // BINARY DAT 段首尾字节合法地等于 ASCII 空白值（0x20/0x09/0x0A/0x0D），
        // 绝不能被 trim_bytes 裁掉——否则首尾样本丢失、后续样本移位。
        let payload: &[u8] = &[0x20, 0x0A, 0x00, 0x01, 0x09, 0x0D, 0x20, 0x20];
        let mut data = Vec::new();
        data.extend_from_slice(b"--- file type CFG ---\nSTATION,X,1999\n...\n");
        data.extend_from_slice(b"--- file type DAT ---\n");
        data.extend_from_slice(payload);
        let sections = extract_sections(&data);
        let dat_bytes = sections.dat_bytes.expect("应有 DAT 字节段");
        assert_eq!(dat_bytes, payload, "BINARY DAT 字节必须逐字保留，不得裁剪");
    }

    #[test]
    fn test_utf8_cfg_section_not_misread_as_gbk() {
        // UTF-8 编码的中文不应被 GBK 解码成乱码。
        // 「母线」= E6 AF 8D E7 BA BF（UTF-8）。
        let cfg = "--- file type CFG ---\nGHBZ,220kV\u{6bcd}\u{7ebf}\u{6545}\u{969c},2013\n1,1,1,1,50,1,1,1,50A,1,1,0,0,0,0,0\n0\n";
        let data = cfg.as_bytes();
        let sections = extract_sections(data);
        let cfg_text = sections.cfg.expect("应有 CFG 段");
        assert!(
            cfg_text.contains('\u{6bcd}'),
            "UTF-8 中文应正确解码，实际: {}",
            cfg_text
        );
    }
}
