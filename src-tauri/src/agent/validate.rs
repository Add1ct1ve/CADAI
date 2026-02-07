use regex::Regex;
use serde::Serialize;

/// A structured representation of a Python error parsed from a traceback.
#[derive(Debug, Serialize, Clone)]
#[allow(dead_code)]
pub struct StructuredError {
    pub error_type: String,
    pub message: String,
    pub line_number: Option<u32>,
    pub suggestion: Option<String>,
}

/// Parse a Python traceback / stderr into a structured error.
///
/// Handles common patterns:
/// - `SyntaxError: invalid syntax` with optional line info
/// - `NameError: name 'X' is not defined`
/// - `AttributeError: ...`
/// - `TypeError: ...`
/// - `ValueError: ...`
/// - CadQuery-specific `OCP.StdFail_NotDone`
/// - Generic fallback for unknown errors
#[allow(dead_code)]
pub fn parse_traceback(stderr: &str) -> StructuredError {
    // Try to extract line number from traceback: "line X" patterns
    let line_number = {
        let line_re = Regex::new(r#"line (\d+)"#).ok();
        line_re.and_then(|re| {
            // Find the last "line N" occurrence (closest to the actual error)
            let mut last_line: Option<u32> = None;
            for cap in re.captures_iter(stderr) {
                if let Ok(n) = cap[1].parse::<u32>() {
                    last_line = Some(n);
                }
            }
            last_line
        })
    };

    // Try to extract the error type and message from the last line of the traceback.
    // Python tracebacks end with "ErrorType: message"
    let error_re = Regex::new(r"(?m)^(\w*Error|\w*Exception|OCP\.\w+):\s*(.*)$").ok();
    if let Some(re) = error_re {
        // Find the last match (the actual error line)
        let mut last_match: Option<(String, String)> = None;
        for cap in re.captures_iter(stderr) {
            last_match = Some((cap[1].to_string(), cap[2].trim().to_string()));
        }

        if let Some((error_type, message)) = last_match {
            let suggestion = generate_suggestion(&error_type, &message, stderr);
            return StructuredError {
                error_type,
                message,
                line_number,
                suggestion,
            };
        }
    }

    // Fallback: use the last non-empty line as the error message
    let last_line = stderr
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("Unknown error")
        .trim()
        .to_string();

    StructuredError {
        error_type: "UnknownError".to_string(),
        message: last_line,
        line_number,
        suggestion: Some("Check the full error output for details.".to_string()),
    }
}

/// Generate a helpful suggestion based on the error type and message.
fn generate_suggestion(error_type: &str, message: &str, _full_stderr: &str) -> Option<String> {
    match error_type {
        "SyntaxError" => Some("Check for missing colons, brackets, or incorrect indentation.".to_string()),
        "NameError" => {
            // Try to extract the undefined name
            let name_re = Regex::new(r"name '(\w+)' is not defined").ok();
            if let Some(re) = name_re {
                if let Some(cap) = re.captures(message) {
                    return Some(format!(
                        "The variable '{}' is not defined. Check spelling or make sure it is assigned before use.",
                        &cap[1]
                    ));
                }
            }
            Some("A variable or function name is not defined. Check for typos.".to_string())
        }
        "AttributeError" => {
            Some("An object does not have the expected attribute or method. Check the CadQuery API documentation.".to_string())
        }
        "TypeError" => {
            Some("Wrong argument types or number of arguments. Check function signatures.".to_string())
        }
        "ValueError" => {
            Some("An invalid value was passed. Check that dimensions and parameters are correct.".to_string())
        }
        "ModuleNotFoundError" | "ImportError" => {
            Some("A required module could not be found. Only cadquery and standard library modules are available.".to_string())
        }
        _ if error_type.starts_with("OCP") => {
            Some("This is a CadQuery/OpenCascade geometry error. The shape operation failed — check dimensions and ensure geometry is valid.".to_string())
        }
        _ => None,
    }
}

/// Extract Python code blocks from AI response text.
/// Looks for ```python ... ``` fenced code blocks.
#[allow(dead_code)]
pub fn extract_python_code(response: &str) -> Option<String> {
    let re = Regex::new(r"```python\s*\n([\s\S]*?)```").ok()?;
    re.captures(response).map(|cap| cap[1].trim().to_string())
}

/// Validate that CadQuery code has the basic required structure.
/// Returns Ok(()) if valid, Err with a list of problems otherwise.
#[allow(dead_code)]
pub fn validate_cadquery_code(code: &str) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    if !code.contains("import cadquery") && !code.contains("import cadquery as cq") {
        errors.push("Code must import cadquery".to_string());
    }

    if !code.contains("result") {
        errors.push("Code must assign final geometry to 'result' variable".to_string());
    }

    // Check for forbidden patterns.
    let forbidden = [
        "show_object",
        "display(",
        "matplotlib",
        "plt.show",
    ];
    for pattern in &forbidden {
        if code.contains(pattern) {
            errors.push(format!("Forbidden pattern found: {}", pattern));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_python_code() {
        let response = "Here is the code:\n```python\nimport cadquery as cq\nresult = cq.Workplane('XY').box(10,10,10)\n```\nDone.";
        let code = extract_python_code(response).unwrap();
        assert!(code.contains("import cadquery as cq"));
        assert!(code.contains("result"));
    }

    #[test]
    fn test_extract_python_code_none() {
        let response = "No code here, just text.";
        assert!(extract_python_code(response).is_none());
    }

    #[test]
    fn test_validate_good_code() {
        let code = "import cadquery as cq\nresult = cq.Workplane('XY').box(10,10,10)";
        assert!(validate_cadquery_code(code).is_ok());
    }

    #[test]
    fn test_validate_missing_import() {
        let code = "result = something()";
        let errors = validate_cadquery_code(code).unwrap_err();
        assert!(errors.iter().any(|e| e.contains("import cadquery")));
    }

    #[test]
    fn test_validate_forbidden_pattern() {
        let code = "import cadquery as cq\nresult = cq.Workplane('XY').box(10,10,10)\nshow_object(result)";
        let errors = validate_cadquery_code(code).unwrap_err();
        assert!(errors.iter().any(|e| e.contains("show_object")));
    }

    #[test]
    fn test_parse_traceback_syntax_error() {
        let stderr = r#"Traceback (most recent call last):
  File "script.py", line 5
    result = cq.Workplane('XY').box(10,10,10
                                           ^
SyntaxError: unexpected EOF while parsing"#;
        let err = parse_traceback(stderr);
        assert_eq!(err.error_type, "SyntaxError");
        assert!(err.message.contains("unexpected EOF"));
        assert_eq!(err.line_number, Some(5));
        assert!(err.suggestion.is_some());
    }

    #[test]
    fn test_parse_traceback_name_error() {
        let stderr = r#"Traceback (most recent call last):
  File "script.py", line 3, in <module>
    result = cq.Workplane('XY').bxo(10,10,10)
NameError: name 'cq' is not defined"#;
        let err = parse_traceback(stderr);
        assert_eq!(err.error_type, "NameError");
        assert!(err.suggestion.unwrap().contains("'cq'"));
        assert_eq!(err.line_number, Some(3));
    }

    #[test]
    fn test_parse_traceback_unknown() {
        let stderr = "something went wrong";
        let err = parse_traceback(stderr);
        assert_eq!(err.error_type, "UnknownError");
        assert_eq!(err.message, "something went wrong");
    }
}
