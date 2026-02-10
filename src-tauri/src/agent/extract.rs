use regex::Regex;

/// Which extraction format matched the AI response.
#[derive(Debug, Clone, PartialEq)]
pub enum ExtractionFormat {
    /// `<CODE>...</CODE>` XML-style tags
    XmlTags,
    /// `` ```python ... ``` `` markdown fence
    MarkdownFence,
    /// Any code block containing CadQuery markers
    Heuristic,
}

/// The result of a successful code extraction.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ExtractionOutcome {
    pub code: String,
    pub format: ExtractionFormat,
}

/// Extract Python/CadQuery code from an AI response using a 3-tier cascade:
///
/// 1. `<CODE>...</CODE>` XML tags (case-insensitive)
/// 2. `` ```python ... ``` `` markdown fence
/// 3. Any `` ``` `` block containing `import cadquery` or `cq.`
///
/// Returns `None` if no code block is found.
pub fn extract_python_code(response: &str) -> Option<ExtractionOutcome> {
    // Tier 1: <CODE>...</CODE> XML tags (case-insensitive)
    if let Some(outcome) = try_xml_tags(response) {
        return Some(outcome);
    }
    // Tier 2: ```python ... ``` markdown fence
    if let Some(outcome) = try_markdown_fence(response) {
        return Some(outcome);
    }
    // Tier 3: Any ``` block with cadquery/cq. markers
    if let Some(outcome) = try_heuristic(response) {
        return Some(outcome);
    }

    eprintln!(
        "[extract] No code block found in AI response ({} chars)",
        response.len()
    );
    None
}

/// Convenience wrapper — returns just the code string.
pub fn extract_code(response: &str) -> Option<String> {
    extract_python_code(response).map(|o| o.code)
}

/// Tier 1: Extract code from `<CODE>...</CODE>` XML-style tags (case-insensitive).
fn try_xml_tags(response: &str) -> Option<ExtractionOutcome> {
    let re = Regex::new(r"(?si)<CODE>([\s\S]*?)</CODE>").ok()?;
    let cap = re.captures(response)?;
    let code = cap[1].trim().to_string();
    if code.is_empty() {
        return None;
    }
    Some(ExtractionOutcome {
        code,
        format: ExtractionFormat::XmlTags,
    })
}

/// Tier 2: Extract code from `` ```python ... ``` `` markdown fence.
fn try_markdown_fence(response: &str) -> Option<ExtractionOutcome> {
    let re = Regex::new(r"```python\s*\n([\s\S]*?)```").ok()?;
    let cap = re.captures(response)?;
    let code = cap[1].trim().to_string();
    if code.is_empty() {
        return None;
    }
    Some(ExtractionOutcome {
        code,
        format: ExtractionFormat::MarkdownFence,
    })
}

/// Tier 3: Find any fenced code block containing CadQuery markers (`import cadquery` or `cq.`).
fn try_heuristic(response: &str) -> Option<ExtractionOutcome> {
    let re = Regex::new(r"```\w*\s*\n([\s\S]*?)```").ok()?;
    for cap in re.captures_iter(response) {
        let code = cap[1].trim().to_string();
        if !code.is_empty()
            && (code.contains("import cadquery") || code.contains("cq."))
        {
            return Some(ExtractionOutcome {
                code,
                format: ExtractionFormat::Heuristic,
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_xml_tags() {
        let response = "Here is the code:\n<CODE>\nimport cadquery as cq\nresult = cq.Workplane('XY').box(10,10,10)\n</CODE>\nDone.";
        let outcome = extract_python_code(response).unwrap();
        assert_eq!(outcome.format, ExtractionFormat::XmlTags);
        assert!(outcome.code.contains("import cadquery as cq"));
    }

    #[test]
    fn test_extract_xml_tags_case_insensitive() {
        let response = "<code>\nimport cadquery as cq\nresult = cq.Workplane('XY').box(5,5,5)\n</code>";
        let outcome = extract_python_code(response).unwrap();
        assert_eq!(outcome.format, ExtractionFormat::XmlTags);
        assert!(outcome.code.contains("import cadquery"));
    }

    #[test]
    fn test_extract_markdown_fence() {
        let response = "Here is the code:\n```python\nimport cadquery as cq\nresult = cq.Workplane('XY').box(10,10,10)\n```\nDone.";
        let outcome = extract_python_code(response).unwrap();
        assert_eq!(outcome.format, ExtractionFormat::MarkdownFence);
        assert!(outcome.code.contains("import cadquery as cq"));
    }

    #[test]
    fn test_extract_heuristic() {
        let response = "Here:\n```\nimport cadquery as cq\nresult = cq.Workplane('XY').box(10,10,10)\n```";
        let outcome = extract_python_code(response).unwrap();
        assert_eq!(outcome.format, ExtractionFormat::Heuristic);
        assert!(outcome.code.contains("import cadquery"));
    }

    #[test]
    fn test_extract_xml_preferred_over_markdown() {
        let response = "<CODE>\nimport cadquery as cq\nresult = cq.Workplane('XY').box(1,1,1)\n</CODE>\n\n```python\nimport cadquery as cq\nresult = cq.Workplane('XY').box(2,2,2)\n```";
        let outcome = extract_python_code(response).unwrap();
        assert_eq!(outcome.format, ExtractionFormat::XmlTags);
        assert!(outcome.code.contains("box(1,1,1)"));
    }

    #[test]
    fn test_extract_markdown_preferred_over_heuristic() {
        let response = "```python\nimport cadquery as cq\nresult = cq.Workplane('XY').box(1,1,1)\n```\n\n```\nimport cadquery as cq\nresult = cq.Workplane('XY').box(2,2,2)\n```";
        let outcome = extract_python_code(response).unwrap();
        assert_eq!(outcome.format, ExtractionFormat::MarkdownFence);
        assert!(outcome.code.contains("box(1,1,1)"));
    }

    #[test]
    fn test_extract_empty_code_block_skipped() {
        // Empty <CODE> tags should fall through to the markdown fence
        let response = "<CODE>\n\n</CODE>\n\n```python\nimport cadquery as cq\nresult = cq.Workplane('XY').box(10,10,10)\n```";
        let outcome = extract_python_code(response).unwrap();
        assert_eq!(outcome.format, ExtractionFormat::MarkdownFence);
    }

    #[test]
    fn test_extract_no_code_returns_none() {
        let response = "This is just plain text with no code blocks at all.";
        assert!(extract_python_code(response).is_none());
    }

    #[test]
    fn test_extract_code_convenience() {
        let response = "<CODE>\nimport cadquery as cq\nresult = cq.Workplane('XY').box(10,10,10)\n</CODE>";
        let code = extract_code(response).unwrap();
        assert!(code.contains("import cadquery as cq"));
    }

    #[test]
    fn test_extract_heuristic_skips_non_cadquery() {
        let response = "```\nprint('hello world')\n```";
        assert!(extract_python_code(response).is_none());
    }
}
