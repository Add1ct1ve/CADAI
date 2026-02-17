use regex::Regex;
use serde::Serialize;

/// Sub-kinds for topology errors, indicating which operation failed.
#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub enum TopologySubKind {
    FilletFailure,
    ShellFailure,
    BooleanFailure,
    LoftFailure,
    SweepFailure,
    RevolveFailure,
    MissingSolid,
    DisconnectedSolids,
    General,
}

/// High-level error category for classification-based retry strategies.
#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(tag = "kind", content = "sub_kind")]
#[allow(dead_code)]
pub enum ErrorCategory {
    Syntax,
    GeometryKernel,
    Topology(TopologySubKind),
    ApiMisuse,
    ImportRuntime,
    Unknown,
}

/// Additional context extracted from the traceback.
#[derive(Debug, Serialize, Clone)]
#[allow(dead_code)]
pub struct ErrorContext {
    pub source_line: Option<String>,
    pub failing_parameters: Option<String>,
}

/// A targeted retry strategy based on error classification.
#[derive(Debug, Clone)]
pub struct RetryStrategy {
    /// Category-specific instruction for the AI on how to fix this error.
    pub fix_instruction: String,
    /// Operations the AI should avoid in its fix.
    pub forbidden_operations: Vec<String>,
    /// The matching anti-pattern title, if any (for including in prompt).
    pub matching_anti_pattern: Option<String>,
}

/// A structured representation of a Python error parsed from a traceback.
#[derive(Debug, Serialize, Clone)]
#[allow(dead_code)]
pub struct StructuredError {
    pub error_type: String,
    pub message: String,
    pub line_number: Option<u32>,
    pub suggestion: Option<String>,
    pub category: ErrorCategory,
    pub failing_operation: Option<String>,
    pub context: Option<ErrorContext>,
}

/// Classify an OCP/Standard_ error based on keyword scanning of message and stderr.
#[allow(dead_code)]
fn classify_ocp_error(message: &str, full_stderr: &str) -> ErrorCategory {
    let combined = format!("{} {}", message, full_stderr).to_lowercase();

    if combined.contains("no pending wires present")
        || (combined.contains("sweep") && combined.contains("wire"))
    {
        return ErrorCategory::Topology(TopologySubKind::SweepFailure);
    }
    if combined.contains("cannot find a solid on the stack")
        || combined.contains("cannot find a solid in the parent chain")
    {
        return ErrorCategory::Topology(TopologySubKind::MissingSolid);
    }
    if combined.contains("stdfail_notdone")
        || combined.contains("brep_api: command not done")
        || combined.contains("command not done")
    {
        if combined.contains("fillet") || combined.contains("chamfer") {
            return ErrorCategory::Topology(TopologySubKind::FilletFailure);
        }
        if combined.contains("shell") || combined.contains("offset") {
            return ErrorCategory::Topology(TopologySubKind::ShellFailure);
        }
        if combined.contains("boolean")
            || combined.contains("fuse")
            || combined.contains("cut")
            || combined.contains("common")
            || combined.contains("intersect")
            || combined.contains("union")
        {
            return ErrorCategory::Topology(TopologySubKind::BooleanFailure);
        }
        if combined.contains("sweep") {
            return ErrorCategory::Topology(TopologySubKind::SweepFailure);
        }
        if combined.contains("loft") {
            return ErrorCategory::Topology(TopologySubKind::LoftFailure);
        }
        if combined.contains("revolve") || combined.contains("revolution") {
            return ErrorCategory::Topology(TopologySubKind::RevolveFailure);
        }
        return ErrorCategory::Topology(TopologySubKind::General);
    }

    if combined.contains("fillet") || combined.contains("chamfer") {
        ErrorCategory::Topology(TopologySubKind::FilletFailure)
    } else if combined.contains("shell") || combined.contains("offset") {
        ErrorCategory::Topology(TopologySubKind::ShellFailure)
    } else if combined.contains("boolean")
        || combined.contains("fuse")
        || combined.contains("cut")
        || combined.contains("common")
    {
        ErrorCategory::Topology(TopologySubKind::BooleanFailure)
    } else if combined.contains("loft") {
        ErrorCategory::Topology(TopologySubKind::LoftFailure)
    } else if combined.contains("sweep") {
        ErrorCategory::Topology(TopologySubKind::SweepFailure)
    } else if combined.contains("revolve") || combined.contains("revolution") {
        ErrorCategory::Topology(TopologySubKind::RevolveFailure)
    } else if combined.contains("brep") || combined.contains("brepbuilder") {
        ErrorCategory::GeometryKernel
    } else {
        ErrorCategory::GeometryKernel
    }
}

/// Classify an error into a category based on error type, message, and full stderr.
#[allow(dead_code)]
fn classify_error(error_type: &str, message: &str, full_stderr: &str) -> ErrorCategory {
    let lower = format!("{} {}", message, full_stderr).to_lowercase();
    match error_type {
        "SyntaxError" | "IndentationError" => ErrorCategory::Syntax,
        "NameError" | "ModuleNotFoundError" | "ImportError" => ErrorCategory::ImportRuntime,
        "AttributeError" => ErrorCategory::ApiMisuse,
        "TypeError" => {
            if lower.contains("build123d") || lower.contains("buildpart") || lower.contains("build_line") {
                ErrorCategory::ApiMisuse
            } else {
                ErrorCategory::Unknown
            }
        }
        _ if error_type.starts_with("OCP") || error_type.starts_with("StdFail") => {
            classify_ocp_error(message, full_stderr)
        }
        _ if error_type.starts_with("Standard_") => classify_ocp_error(message, full_stderr),
        "ValueError" => {
            if lower.contains("sweep") || lower.contains("wire") {
                ErrorCategory::Topology(TopologySubKind::SweepFailure)
            } else if lower.contains("cannot find a solid on the stack")
                || lower.contains("cannot find a solid in the parent chain")
            {
                ErrorCategory::Topology(TopologySubKind::MissingSolid)
            } else if lower.contains("multiple objects selected") && lower.contains("planar faces")
            {
                ErrorCategory::ApiMisuse
            } else {
                ErrorCategory::ApiMisuse
            }
        }
        "RuntimeError" => {
            if lower.contains("ocp")
                || lower.contains("stdfail")
                || lower.contains("brep")
                || lower.contains("topods")
            {
                classify_ocp_error(message, full_stderr)
            } else {
                ErrorCategory::Unknown
            }
        }
        _ => ErrorCategory::Unknown,
    }
}

/// Known CAD operations for extraction from tracebacks.
const CAD_OPERATIONS: &[&str] = &[
    "fillet",
    "chamfer",
    "shell",
    "loft",
    "sweep",
    "revolve",
    "cut",
    "union",
    "intersect",
    "extrude",
    "hole",
    "translate",
    "rotate",
    "mirror",
    "offset",
    "fuse",
    "combine",
];

/// Extract the failing CAD operation from a traceback by scanning user code lines.
/// Matches both standalone function calls (Build123d) and method-chain patterns.
#[allow(dead_code)]
fn extract_failing_operation(full_stderr: &str) -> Option<String> {
    // Match lines from user code files in the traceback
    let file_re = Regex::new(r#"(?m)File "(?:input\.py|<string>|script\.py)".*\n\s+(.+)"#).ok()?;

    // Collect all user code lines from the traceback
    let mut user_lines: Vec<String> = Vec::new();
    for cap in file_re.captures_iter(full_stderr) {
        user_lines.push(cap[1].trim().to_string());
    }

    // Also check the last few lines of stderr for operation calls
    for line in full_stderr.lines().rev().take(10) {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            user_lines.push(trimmed.to_string());
        }
    }

    // Look for operation( patterns matching known CAD operations
    // Match both `.operation(` (method chain) and standalone `operation(` (Build123d)
    let op_re = Regex::new(r"(?:^|[^.\w])(\w+)\s*\(").ok()?;
    for line in user_lines.iter().rev() {
        for cap in op_re.captures_iter(line) {
            let op = &cap[1];
            if CAD_OPERATIONS.contains(&op) {
                return Some(op.to_string());
            }
        }
    }

    // Also check library traceback lines for operation names in paths
    // e.g., "build123d/operations_generic.py", line 234, in fillet"
    let lib_re = Regex::new(r"in (\w+)\s*$").ok()?;
    for line in full_stderr.lines() {
        if let Some(cap) = lib_re.captures(line.trim()) {
            let op = &cap[1];
            if CAD_OPERATIONS.contains(&op) {
                return Some(op.to_string());
            }
        }
    }

    // Check if the error message itself names an operation
    // e.g., "Boolean operation (cut) failed"
    let msg_re = Regex::new(r"(?i)\b(fillet|chamfer|shell|loft|sweep|revolve|cut|union|intersect|extrude|hole|translate|rotate|mirror)\b").ok()?;
    if let Some(cap) = msg_re.captures(full_stderr) {
        let op = cap[1].to_lowercase();
        if CAD_OPERATIONS.contains(&op.as_str()) {
            return Some(op);
        }
    }

    None
}

/// Extract additional error context from the traceback.
#[allow(dead_code)]
fn extract_error_context(full_stderr: &str, message: &str) -> Option<ErrorContext> {
    // Extract the last user source line from traceback
    let file_re = Regex::new(r#"(?m)File "(?:input\.py|<string>|script\.py)".*\n\s+(.+)"#).ok()?;

    let mut source_line: Option<String> = None;
    for cap in file_re.captures_iter(full_stderr) {
        source_line = Some(cap[1].trim().to_string());
    }

    // Extract parameter hints (numbers) from the error message
    let num_re = Regex::new(r"\b\d+\.?\d*\b").ok()?;
    let numbers: Vec<&str> = num_re.find_iter(message).map(|m| m.as_str()).collect();
    let failing_parameters = if numbers.is_empty() {
        None
    } else {
        Some(numbers.join(", "))
    };

    if source_line.is_some() || failing_parameters.is_some() {
        Some(ErrorContext {
            source_line,
            failing_parameters,
        })
    } else {
        None
    }
}

/// Parse a Python traceback / stderr into a structured error.
///
/// Handles common patterns:
/// - `SyntaxError: invalid syntax` with optional line info
/// - `NameError: name 'X' is not defined`
/// - `AttributeError: ...`
/// - `TypeError: ...`
/// - `ValueError: ...`
/// - OCCT-specific `OCP.StdFail_NotDone`
/// - Generic fallback for unknown errors
#[allow(dead_code)]
pub fn parse_traceback(stderr: &str) -> StructuredError {
    // Early detection: disconnected solids (exit code 5 or SPLIT_BODY marker)
    let lower_stderr = stderr.to_lowercase();
    if lower_stderr.contains("disconnected solids") || stderr.contains("SPLIT_BODY") {
        return StructuredError {
            error_type: "DisconnectedSolids".to_string(),
            message: stderr.lines().find(|l| !l.trim().is_empty()).unwrap_or(stderr).trim().to_string(),
            line_number: None,
            suggestion: Some(
                "A cut went through a wall and split the body into separate pieces. \
                 Reduce cut depth below wall thickness (max 80% of wall) or increase wall thickness."
                    .to_string(),
            ),
            category: ErrorCategory::Topology(TopologySubKind::DisconnectedSolids),
            failing_operation: Some("cut".to_string()),
            context: None,
        };
    }

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
    let mut last_match: Option<(String, String)> = None;
    for line in stderr.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || !trimmed.contains(':') {
            continue;
        }
        let (left, right) = match trimmed.split_once(':') {
            Some(v) => v,
            None => continue,
        };
        let error_type = left.trim();
        let message = right.trim();
        let is_exception_like = error_type.contains("Error")
            || error_type.contains("Exception")
            || error_type.starts_with("OCP.")
            || error_type.starts_with("StdFail")
            || error_type.starts_with("Standard_");
        if is_exception_like && !message.is_empty() {
            last_match = Some((error_type.to_string(), message.to_string()));
        }
    }

    if let Some((error_type, message)) = last_match {
        let category = classify_error(&error_type, &message, stderr);
        let failing_operation = extract_failing_operation(stderr);
        let context = extract_error_context(stderr, &message);
        let suggestion = generate_suggestion(&error_type, &message, stderr, &category);
        return StructuredError {
            error_type,
            message,
            line_number,
            suggestion,
            category,
            failing_operation,
            context,
        };
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
        category: ErrorCategory::Unknown,
        failing_operation: None,
        context: None,
    }
}

/// Generate a helpful suggestion based on the error type, message, and classified category.
fn generate_suggestion(
    error_type: &str,
    message: &str,
    _full_stderr: &str,
    category: &ErrorCategory,
) -> Option<String> {
    // Category-specific suggestions take priority for topology errors
    match category {
        ErrorCategory::Topology(TopologySubKind::MissingSolid) => {
            return Some(
                "A 3D operation requires a solid body. Ensure you're inside a `with BuildPart():` context and have added 3D geometry before applying operations.".to_string(),
            );
        }
        ErrorCategory::Topology(TopologySubKind::FilletFailure) => {
            return Some(
                "Fillet radius is too large for the geometry. Reduce the radius or apply fillets before boolean operations.".to_string(),
            );
        }
        ErrorCategory::Topology(TopologySubKind::ShellFailure) => {
            return Some(
                "Shell operation failed. Try using manual box subtraction instead of .shell(), or simplify the geometry first.".to_string(),
            );
        }
        ErrorCategory::Topology(TopologySubKind::BooleanFailure) => {
            return Some(
                "Boolean operation failed. Ensure cutting/fusing bodies fully overlap the target by at least 0.1mm and are not tangent.".to_string(),
            );
        }
        ErrorCategory::Topology(TopologySubKind::LoftFailure) => {
            return Some(
                "Loft failed. Ensure profiles have compatible edge counts and orientations. Consider using stacked extrudes instead.".to_string(),
            );
        }
        ErrorCategory::Topology(TopologySubKind::SweepFailure) => {
            return Some(
                "Sweep failed. Ensure the path is a Wire object (call .wire() on edge chains) and the profile doesn't self-intersect along the path.".to_string(),
            );
        }
        ErrorCategory::Topology(TopologySubKind::RevolveFailure) => {
            return Some(
                "Revolve failed. The profile must be entirely on one side of the rotation axis with no crossings.".to_string(),
            );
        }
        ErrorCategory::Topology(TopologySubKind::DisconnectedSolids) => {
            return Some(
                "A cut went through a wall and split the body. Reduce cut depth below wall thickness (max 80% of wall) or increase wall thickness.".to_string(),
            );
        }
        ErrorCategory::Topology(TopologySubKind::General) => {
            return Some(
                "A topology operation failed. Simplify the geometry or break complex operations into smaller steps.".to_string(),
            );
        }
        ErrorCategory::GeometryKernel => {
            return Some(
                "OpenCascade geometry kernel error. The shape operation failed — check dimensions and ensure geometry is valid.".to_string(),
            );
        }
        _ => {}
    }

    // Fall back to error-type-based suggestions
    match error_type {
        "SyntaxError" | "IndentationError" => {
            Some("Check for missing colons, brackets, or incorrect indentation.".to_string())
        }
        "NameError" => {
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
        "AttributeError" => Some(
            "An object does not have the expected attribute or method. Check the Build123d API documentation.".to_string(),
        ),
        "TypeError" => Some(
            "Wrong argument types or number of arguments. Check function signatures.".to_string(),
        ),
        "ValueError" => Some(
            "An invalid value was passed. Check that dimensions and parameters are correct."
                .to_string(),
        ),
        "ModuleNotFoundError" | "ImportError" => Some(
            "A required module could not be found. Only build123d and standard library modules are available.".to_string(),
        ),
        _ => None,
    }
}

/// Extract Python code blocks from AI response text.
/// Delegates to the multi-format extraction cascade in `extract.rs`.
#[allow(dead_code)]
pub fn extract_python_code(response: &str) -> Option<String> {
    crate::agent::extract::extract_code(response)
}

/// Determine the matching anti-pattern title based on error category and message.
fn match_anti_pattern(error: &StructuredError, code: Option<&str>) -> Option<String> {
    match &error.category {
        ErrorCategory::Topology(TopologySubKind::MissingSolid) => {
            Some("3D operation on 2D sketch (no solid on stack)".to_string())
        }
        ErrorCategory::Topology(TopologySubKind::FilletFailure) => {
            // Check if code uses blanket fillet() with .edges() pattern
            let is_blanket = code
                .map(|c| (c.contains("fillet(") && c.contains(".edges()")) || (c.contains("chamfer(") && c.contains(".edges()")))
                .unwrap_or(false);
            if is_blanket {
                Some("Blanket fillet on complex geometry".to_string())
            } else {
                let msg = error.message.to_lowercase();
                if msg.contains("before") || msg.contains("boolean") {
                    Some("Fillet before boolean".to_string())
                } else {
                    Some("Fillet radius too large".to_string())
                }
            }
        }
        ErrorCategory::Topology(TopologySubKind::ShellFailure) => {
            Some("Shell on complex boolean body".to_string())
        }
        ErrorCategory::Topology(TopologySubKind::BooleanFailure) => {
            let msg = error.message.to_lowercase();
            if msg.contains("chained") || msg.contains("many") || msg.contains("loop") {
                Some("Too many chained booleans".to_string())
            } else {
                Some("Boolean on non-overlapping bodies".to_string())
            }
        }
        ErrorCategory::Topology(TopologySubKind::LoftFailure) => {
            Some("Loft with mismatched profiles".to_string())
        }
        ErrorCategory::Topology(TopologySubKind::SweepFailure) => {
            Some("Sweep path without .wire()".to_string())
        }
        ErrorCategory::Topology(TopologySubKind::RevolveFailure) => {
            Some("Revolve profile crossing axis".to_string())
        }
        ErrorCategory::Topology(TopologySubKind::DisconnectedSolids) => {
            Some("Through-cut splitting body".to_string())
        }
        ErrorCategory::ApiMisuse => {
            let msg = error.message.to_lowercase();
            if msg.contains("translate") {
                Some("translate() wrong signature".to_string())
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Get a targeted retry strategy based on the classified error and attempt number.
///
/// - Attempt 1: Category-specific targeted fix
/// - Attempt 2: Targeted fix + simplification of the failing operation class
/// - Attempt 3+: Full nuclear simplification (basic primitives only)
///
/// The optional `code` parameter allows detecting blanket `.edges().fillet()` patterns
/// to provide smarter retry advice (wrap in try/except instead of reducing radius).
pub fn get_retry_strategy(
    error: &StructuredError,
    attempt: u32,
    code: Option<&str>,
) -> RetryStrategy {
    let anti_pattern = match_anti_pattern(error, code);

    // Attempt 3+: nuclear option — same for all categories.
    if attempt >= 3 {
        return RetryStrategy {
            fix_instruction: "SIGNIFICANTLY simplify the geometry. Use ONLY basic shapes \
                (box, cylinder, sphere, cone) combined with boolean operations. You may use \
                fillets with small radii (1-2mm). Do NOT use sweep, loft, spline, or revolve. \
                Do NOT use shell on complex geometry. Prioritize getting something that RENDERS."
                .to_string(),
            forbidden_operations: vec![
                "sweep".to_string(),
                "loft".to_string(),
                "spline".to_string(),
                "revolve".to_string(),
                "shell".to_string(),
            ],
            matching_anti_pattern: anti_pattern,
        };
    }

    let line_hint = error
        .line_number
        .map(|n| format!(" on line {}", n))
        .unwrap_or_default();
    let msg = &error.message;
    let op = error
        .failing_operation
        .as_deref()
        .unwrap_or("the failing operation");

    let (mut fix_instruction, mut forbidden_operations) = match &error.category {
        ErrorCategory::Syntax => (
            format!(
                "Fix the syntax error{}. The error is: {}. \
                 Check for missing colons, brackets, parentheses, or incorrect indentation.",
                line_hint, msg
            ),
            vec![],
        ),
        ErrorCategory::Topology(TopologySubKind::MissingSolid) => (
            "A 3D operation requires a solid body. Ensure you're inside a `with BuildPart():` \
             context and have added 3D geometry (Box, Cylinder, extrude, etc.) before applying \
             operations like fillet, chamfer, or boolean cuts."
                .to_string(),
            vec![],
        ),
        ErrorCategory::Topology(TopologySubKind::FilletFailure) => {
            let is_blanket = code
                .map(|c| (c.contains("fillet(") && c.contains(".edges()")) || (c.contains("chamfer(") && c.contains(".edges()")))
                .unwrap_or(false);
            if is_blanket {
                (
                    "The fillet() on all edges pattern CANNOT fillet ALL edges on complex geometry \
                     (loft+shell+boolean). Do NOT reduce the radius — that won't fix this. \
                     Instead wrap the fillet in try/except: \
                     `try: result = fillet(body.edges(), radius=r)` / `except: result = body`. \
                     Or use selective edge filtering."
                        .to_string(),
                    vec![],
                )
            } else {
                (
                    "The fillet/chamfer radius is too large for the geometry. Reduce ALL fillet/chamfer \
                     radii by at least 50%, or remove fillets entirely and add them as the very last \
                     operation with conservative radii (1-2mm)."
                        .to_string(),
                    vec![],
                )
            }
        }
        ErrorCategory::Topology(TopologySubKind::ShellFailure) => (
            "The shell operation failed. Replace .shell() with manual hollowing: create a \
             slightly smaller inner solid and use .cut() to subtract it. Or apply shell BEFORE \
             boolean operations on the simple base shape."
                .to_string(),
            vec!["shell".to_string()],
        ),
        ErrorCategory::Topology(TopologySubKind::BooleanFailure) => (
            format!(
                "The boolean operation ({}) failed. Ensure cutting/fusing bodies overlap the \
                 target by at least 0.5mm — extend tools beyond target surfaces. If using many \
                 booleans, combine features using .pushPoints() + single extrude instead of \
                 looped unions.",
                op
            ),
            vec![],
        ),
        ErrorCategory::Topology(TopologySubKind::LoftFailure) => (
            "The loft failed. Ensure profiles have compatible edge counts. Try: add ruled=True, \
             use similar profile types (both rects or both circles), or replace loft with stacked \
             extrudes connected by fillets."
                .to_string(),
            vec![],
        ),
        ErrorCategory::Topology(TopologySubKind::SweepFailure) => (
            "The sweep failed. Ensure the path is a Wire object — call .wire() on edge chains. \
             Keep the profile perpendicular to the path start and avoid self-intersecting paths."
                .to_string(),
            vec![],
        ),
        ErrorCategory::Topology(TopologySubKind::RevolveFailure) => (
            "The revolve failed. The profile must be entirely on one side of the rotation axis \
             — all X coordinates must be >= 0 when revolving around Y. Move the profile so no \
             points cross the axis."
                .to_string(),
            vec![],
        ),
        ErrorCategory::Topology(TopologySubKind::DisconnectedSolids) => (
            "A cut went through a wall and SPLIT the body into disconnected solids. Fix: \
             1) Reduce cut/pocket depth to at most 80% of the wall thickness. \
             2) On hollowed bodies, do NOT let pocket/slot cuts reach the inner cavity. \
             3) For through-features, use holes that don't intersect internal cavities. \
             4) union()/fuse() calls need at least 0.5mm volumetric overlap. \
             5) The result MUST be exactly 1 connected solid."
                .to_string(),
            vec![],
        ),
        ErrorCategory::Topology(TopologySubKind::General) => (
            "A topology operation failed. Simplify the failing operation or break it into \
             smaller steps. Apply fillets/chamfers last, after all booleans."
                .to_string(),
            vec![],
        ),
        ErrorCategory::ApiMisuse => {
            if msg.to_lowercase().contains("multiple objects selected")
                && msg.to_lowercase().contains("planar faces")
            {
                (
                    "A face selection returned multiple faces. Use a more specific selector \
                     or filter to a single face before proceeding."
                        .to_string(),
                    vec![],
                )
            } else {
                (
                    format!(
                        "There is an API usage error: {}. Check the Build123d API — verify method names, \
                         argument types, and builder context usage.",
                        msg
                    ),
                    vec![],
                )
            }
        }
        ErrorCategory::ImportRuntime => (
            format!(
                "Fix the import/name error: {}. Only build123d and standard library modules are \
                 available. Check variable names for typos.",
                msg
            ),
            vec![],
        ),
        ErrorCategory::GeometryKernel => (
            "The OpenCascade geometry kernel failed. Simplify the geometry: use larger minimum \
             dimensions (>1mm), ensure no zero-thickness walls, and reduce the number of complex \
             operations."
                .to_string(),
            vec![],
        ),
        ErrorCategory::Unknown => (
            format!(
                "The code failed with: {}. Review the error carefully and fix the specific issue.",
                msg
            ),
            vec![],
        ),
    };

    // Attempt 2: append simplification guidance.
    if attempt == 2 {
        match &error.category {
            ErrorCategory::Topology(_) => {
                fix_instruction.push_str(&format!(
                    " If the {} approach doesn't work, replace it with a simpler geometric \
                     construction using only extrude, cut, and union.",
                    op
                ));
                // Add the failing operation to forbidden list for topology errors.
                if let Some(ref failing_op) = error.failing_operation {
                    if !forbidden_operations.contains(failing_op) {
                        forbidden_operations.push(failing_op.clone());
                    }
                }
            }
            _ => {
                fix_instruction.push_str(
                    " If the direct fix doesn't work, simplify the overall approach. \
                     Use basic primitives where possible.",
                );
            }
        }
    }

    RetryStrategy {
        fix_instruction,
        forbidden_operations,
        matching_anti_pattern: anti_pattern,
    }
}

/// Check if code uses a risky blanket fillet pattern on complex geometry.
///
/// Returns a warning message if the code contains `fillet()` with `.edges()`
/// combined with complex operations (loft, shell, or multiple booleans) that are likely to fail.
#[allow(dead_code)]
pub fn check_risky_fillet_pattern(code: &str) -> Option<String> {
    let has_blanket = (code.contains("fillet(") && code.contains(".edges()"))
        || (code.contains("chamfer(") && code.contains(".edges()"));
    if !has_blanket {
        return None;
    }
    let has_complex = code.contains("loft(")
        || code.contains("shell(")
        || code.contains("offset_3d(")
        || (code.contains(".union(") && code.contains(".cut("))
        || code.matches(".cut(").count() >= 3;
    if has_complex {
        Some(
            "Code uses fillet() on complex loft/shell/boolean geometry — high failure risk"
                .to_string(),
        )
    } else {
        None
    }
}

/// Validate that CAD code has the basic required structure.
/// Returns Ok(()) if valid, Err with a list of problems otherwise.
#[allow(dead_code)]
pub fn validate_cad_code(code: &str) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    if !code.contains("from build123d import") && !code.contains("build123d") {
        errors.push("Code must import build123d".to_string());
    }

    if !code.contains("result") {
        errors.push("Code must assign final geometry to 'result' variable".to_string());
    }

    // Check for forbidden patterns.
    let forbidden = ["show_object", "display(", "matplotlib", "plt.show"];
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
        let response = "Here is the code:\n```python\nfrom build123d import *\nresult = Box(10, 10, 10)\n```\nDone.";
        let code = extract_python_code(response).unwrap();
        assert!(code.contains("from build123d import *"));
        assert!(code.contains("result"));
    }

    #[test]
    fn test_extract_python_code_none() {
        let response = "No code here, just text.";
        assert!(extract_python_code(response).is_none());
    }

    #[test]
    fn test_validate_good_code() {
        let code = "from build123d import *\nresult = Box(10, 10, 10)";
        assert!(validate_cad_code(code).is_ok());
    }

    #[test]
    fn test_validate_missing_import() {
        let code = "result = something()";
        let errors = validate_cad_code(code).unwrap_err();
        assert!(errors.iter().any(|e| e.contains("import build123d")));
    }

    #[test]
    fn test_validate_forbidden_pattern() {
        let code =
            "from build123d import *\nresult = Box(10, 10, 10)\nshow_object(result)";
        let errors = validate_cad_code(code).unwrap_err();
        assert!(errors.iter().any(|e| e.contains("show_object")));
    }

    // ========== Updated existing tests ==========

    #[test]
    fn test_parse_traceback_syntax_error() {
        let stderr = r#"Traceback (most recent call last):
  File "script.py", line 5
    result = Box(10, 10, 10
                           ^
SyntaxError: unexpected EOF while parsing"#;
        let err = parse_traceback(stderr);
        assert_eq!(err.error_type, "SyntaxError");
        assert!(err.message.contains("unexpected EOF"));
        assert_eq!(err.line_number, Some(5));
        assert!(err.suggestion.is_some());
        assert_eq!(err.category, ErrorCategory::Syntax);
    }

    #[test]
    fn test_parse_traceback_name_error() {
        let stderr = r#"Traceback (most recent call last):
  File "script.py", line 3, in <module>
    result = Bxo(10,10,10)
NameError: name 'Bxo' is not defined"#;
        let err = parse_traceback(stderr);
        assert_eq!(err.error_type, "NameError");
        assert!(err.suggestion.unwrap().contains("'Bxo'"));
        assert_eq!(err.line_number, Some(3));
        assert_eq!(err.category, ErrorCategory::ImportRuntime);
    }

    #[test]
    fn test_parse_traceback_unknown() {
        let stderr = "something went wrong";
        let err = parse_traceback(stderr);
        assert_eq!(err.error_type, "UnknownError");
        assert_eq!(err.message, "something went wrong");
        assert_eq!(err.category, ErrorCategory::Unknown);
    }

    // ========== Classification unit tests ==========

    #[test]
    fn test_classify_syntax_error() {
        assert_eq!(
            classify_error("SyntaxError", "invalid syntax", ""),
            ErrorCategory::Syntax
        );
    }

    #[test]
    fn test_classify_indentation_error() {
        assert_eq!(
            classify_error("IndentationError", "unexpected indent", ""),
            ErrorCategory::Syntax
        );
    }

    #[test]
    fn test_classify_name_error() {
        assert_eq!(
            classify_error("NameError", "name 'cq' is not defined", ""),
            ErrorCategory::ImportRuntime
        );
    }

    #[test]
    fn test_classify_module_not_found_error() {
        assert_eq!(
            classify_error("ModuleNotFoundError", "No module named 'foo'", ""),
            ErrorCategory::ImportRuntime
        );
    }

    #[test]
    fn test_classify_import_error() {
        assert_eq!(
            classify_error("ImportError", "cannot import name 'bar'", ""),
            ErrorCategory::ImportRuntime
        );
    }

    #[test]
    fn test_classify_attribute_error_on_cq() {
        assert_eq!(
            classify_error(
                "AttributeError",
                "'Workplane' object has no attribute 'bxo'",
                ""
            ),
            ErrorCategory::ApiMisuse
        );
    }

    #[test]
    fn test_classify_type_error_on_b3d() {
        assert_eq!(
            classify_error(
                "TypeError",
                "translate() takes 2 arguments",
                "build123d BuildPart Box(10,10,10)"
            ),
            ErrorCategory::ApiMisuse
        );
    }

    #[test]
    fn test_classify_type_error_non_cq_is_unknown() {
        assert_eq!(
            classify_error("TypeError", "unsupported operand type", "print(1 + 'a')"),
            ErrorCategory::Unknown
        );
    }

    #[test]
    fn test_classify_ocp_stdfail_fillet() {
        assert_eq!(
            classify_error(
                "OCP.StdFail_NotDone",
                "BRep_API: not done",
                "result.fillet(5.0)"
            ),
            ErrorCategory::Topology(TopologySubKind::FilletFailure)
        );
    }

    #[test]
    fn test_classify_ocp_stdfail_shell() {
        assert_eq!(
            classify_error(
                "OCP.StdFail_NotDone",
                "BRep_API: not done",
                "result.shell(-1.0) offset"
            ),
            ErrorCategory::Topology(TopologySubKind::ShellFailure)
        );
    }

    #[test]
    fn test_classify_ocp_stdfail_boolean() {
        assert_eq!(
            classify_error(
                "OCP.StdFail_NotDone",
                "boolean operation failed",
                "result.cut(other)"
            ),
            ErrorCategory::Topology(TopologySubKind::BooleanFailure)
        );
    }

    #[test]
    fn test_classify_ocp_stdfail_loft() {
        assert_eq!(
            classify_error("OCP.StdFail_NotDone", "loft failed", "result.loft()"),
            ErrorCategory::Topology(TopologySubKind::LoftFailure)
        );
    }

    #[test]
    fn test_classify_ocp_stdfail_sweep() {
        assert_eq!(
            classify_error("OCP.StdFail_NotDone", "sweep failed", "result.sweep(path)"),
            ErrorCategory::Topology(TopologySubKind::SweepFailure)
        );
    }

    #[test]
    fn test_classify_ocp_stdfail_revolve() {
        assert_eq!(
            classify_error(
                "OCP.StdFail_NotDone",
                "revolve operation error",
                "result.revolve(360)"
            ),
            ErrorCategory::Topology(TopologySubKind::RevolveFailure)
        );
    }

    #[test]
    fn test_classify_ocp_stdfail_generic_is_geometry_kernel() {
        assert_eq!(
            classify_error("OCP.StdFail_NotDone", "some unknown OCP error", ""),
            ErrorCategory::GeometryKernel
        );
    }

    #[test]
    fn test_classify_brepbuilderapi() {
        assert_eq!(
            classify_error(
                "OCP.BRepBuilderAPI_MakeEdge",
                "edge construction failed",
                "BRepBuilderAPI error"
            ),
            ErrorCategory::GeometryKernel
        );
    }

    #[test]
    fn test_classify_runtime_error_wrapping_ocp() {
        assert_eq!(
            classify_error(
                "RuntimeError",
                "OCP.StdFail_NotDone: fillet failed",
                "result.fillet(10)"
            ),
            ErrorCategory::Topology(TopologySubKind::FilletFailure)
        );
    }

    #[test]
    fn test_classify_value_error_sweep() {
        assert_eq!(
            classify_error(
                "ValueError",
                "Cannot sweep: path is not a wire",
                "result.sweep(path)"
            ),
            ErrorCategory::Topology(TopologySubKind::SweepFailure)
        );
    }

    #[test]
    fn test_parse_traceback_dotted_ocp_type() {
        let stderr = r#"Traceback (most recent call last):
  File "<string>", line 57, in <module>
OCP.OCP.StdFail.StdFail_NotDone: BRep_API: command not done"#;
        let err = parse_traceback(stderr);
        assert_eq!(err.error_type, "OCP.OCP.StdFail.StdFail_NotDone");
        assert_eq!(
            err.category,
            ErrorCategory::Topology(TopologySubKind::General)
        );
    }

    #[test]
    fn test_classify_no_pending_wires_deterministically() {
        assert_eq!(
            classify_error(
                "ValueError",
                "No pending wires present",
                "result.sweep(path)"
            ),
            ErrorCategory::Topology(TopologySubKind::SweepFailure)
        );
    }

    #[test]
    fn test_classify_cannot_find_solid_deterministically() {
        assert_eq!(
            classify_error(
                "ValueError",
                "Cannot find a solid on the stack or in the parent chain",
                "result = body.edges().fillet(1.0)"
            ),
            ErrorCategory::Topology(TopologySubKind::MissingSolid)
        );
    }

    #[test]
    fn test_classify_multiple_objects_planar_faces_as_api_misuse() {
        assert_eq!(
            classify_error(
                "ValueError",
                "If multiple objects selected, they all must be planar faces.",
                "result = body.faces(\">Z\").workplane()"
            ),
            ErrorCategory::ApiMisuse
        );
    }

    #[test]
    fn test_classify_unknown_error() {
        assert_eq!(
            classify_error("ZeroDivisionError", "division by zero", ""),
            ErrorCategory::Unknown
        );
    }

    // ========== Operation extraction tests ==========

    #[test]
    fn test_extract_operation_fillet() {
        let stderr = r#"Traceback (most recent call last):
  File "input.py", line 5, in <module>
    result = base.fillet(2.0)
OCP.StdFail_NotDone: BRep_API: not done"#;
        assert_eq!(
            extract_failing_operation(stderr),
            Some("fillet".to_string())
        );
    }

    #[test]
    fn test_extract_operation_shell() {
        let stderr = r#"Traceback (most recent call last):
  File "input.py", line 4, in <module>
    result = box.shell(-1.0)
OCP.StdFail_NotDone: offset not done"#;
        assert_eq!(extract_failing_operation(stderr), Some("shell".to_string()));
    }

    #[test]
    fn test_extract_operation_loft() {
        let stderr = r#"Traceback (most recent call last):
  File "<string>", line 8, in <module>
    result = wp.loft(ruled=True)
OCP.StdFail_NotDone: loft failed"#;
        assert_eq!(extract_failing_operation(stderr), Some("loft".to_string()));
    }

    #[test]
    fn test_extract_operation_none_for_syntax() {
        let stderr = r#"  File "script.py", line 1
    def foo(
           ^
SyntaxError: unexpected EOF"#;
        // No known CQ operation on the source line
        assert_eq!(extract_failing_operation(stderr), None);
    }

    // ========== Full integration tests with realistic stderr ==========

    #[test]
    fn test_parse_traceback_fillet_too_large() {
        let stderr = r#"Traceback (most recent call last):
  File "input.py", line 6, in <module>
    result = fillet(Box(10, 10, 10).edges(), radius=8.0)
  File "/venv/lib/python3.10/site-packages/build123d/operations_generic.py", line 234, in fillet
    raise StdFail_NotDone
OCP.StdFail_NotDone: BRep_API: not done"#;
        let err = parse_traceback(stderr);
        assert_eq!(
            err.category,
            ErrorCategory::Topology(TopologySubKind::FilletFailure)
        );
        assert_eq!(err.failing_operation, Some("fillet".to_string()));
        assert!(err.suggestion.unwrap().contains("Fillet radius"));
    }

    #[test]
    fn test_parse_traceback_shell_failure() {
        let stderr = r#"Traceback (most recent call last):
  File "input.py", line 5, in <module>
    result = shell(Box(20, 20, 20).faces(), thickness=-2.0)
  File "/venv/lib/python3.10/site-packages/build123d/operations_generic.py", line 987, in shell
    raise StdFail_NotDone
OCP.StdFail_NotDone: Shell offset not done"#;
        let err = parse_traceback(stderr);
        assert_eq!(
            err.category,
            ErrorCategory::Topology(TopologySubKind::ShellFailure)
        );
        assert_eq!(err.failing_operation, Some("shell".to_string()));
        assert!(err.suggestion.unwrap().contains("Shell operation"));
    }

    #[test]
    fn test_parse_traceback_boolean_failure() {
        let stderr = r#"Traceback (most recent call last):
  File "input.py", line 8, in <module>
    result = base - cutter
  File "/venv/lib/python3.10/site-packages/build123d/topology.py", line 555, in __sub__
    raise StdFail_NotDone
OCP.StdFail_NotDone: Boolean operation (cut) failed"#;
        let err = parse_traceback(stderr);
        assert_eq!(
            err.category,
            ErrorCategory::Topology(TopologySubKind::BooleanFailure)
        );
        assert_eq!(err.failing_operation, Some("cut".to_string()));
        assert!(err.suggestion.unwrap().contains("Boolean operation"));
    }

    #[test]
    fn test_parse_traceback_attribute_error_cad() {
        let stderr = r#"Traceback (most recent call last):
  File "input.py", line 3, in <module>
    result = Bocks(10, 10, 10)
AttributeError: module 'build123d' has no attribute 'Bocks'"#;
        let err = parse_traceback(stderr);
        assert_eq!(err.category, ErrorCategory::ApiMisuse);
        assert_eq!(err.error_type, "AttributeError");
    }

    #[test]
    fn test_parse_traceback_type_error_translate() {
        let stderr = r#"Traceback (most recent call last):
  File "input.py", line 4, in <module>
    result = Box(10,10,10).translate((1,2))
TypeError: translate() requires a 3-tuple for build123d.BuildPart"#;
        let err = parse_traceback(stderr);
        assert_eq!(err.category, ErrorCategory::ApiMisuse);
        assert_eq!(err.failing_operation, Some("translate".to_string()));
    }

    #[test]
    fn test_parse_traceback_value_error_sweep() {
        let stderr = r#"Traceback (most recent call last):
  File "input.py", line 7, in <module>
    result = profile.sweep(path)
ValueError: Cannot sweep along path: expected Wire, got Edge"#;
        let err = parse_traceback(stderr);
        assert_eq!(
            err.category,
            ErrorCategory::Topology(TopologySubKind::SweepFailure)
        );
        assert_eq!(err.failing_operation, Some("sweep".to_string()));
        assert!(err.suggestion.unwrap().contains("Sweep failed"));
    }

    // ========== RetryStrategy tests ==========

    /// Helper: create a StructuredError with the given category.
    fn make_error(
        category: ErrorCategory,
        message: &str,
        failing_op: Option<&str>,
    ) -> StructuredError {
        StructuredError {
            error_type: "TestError".to_string(),
            message: message.to_string(),
            line_number: Some(5),
            suggestion: None,
            category,
            failing_operation: failing_op.map(|s| s.to_string()),
            context: Some(ErrorContext {
                source_line: Some("result = base.fillet(10)".to_string()),
                failing_parameters: None,
            }),
        }
    }

    #[test]
    fn test_strategy_syntax_attempt1() {
        let err = make_error(ErrorCategory::Syntax, "invalid syntax", None);
        let strategy = get_retry_strategy(&err, 1, None);
        assert!(strategy.fix_instruction.contains("syntax error"));
        assert!(strategy.fix_instruction.contains("line 5"));
        assert!(strategy.forbidden_operations.is_empty());
        assert!(strategy.matching_anti_pattern.is_none());
    }

    #[test]
    fn test_strategy_fillet_attempt1() {
        let err = make_error(
            ErrorCategory::Topology(TopologySubKind::FilletFailure),
            "BRep_API: not done",
            Some("fillet"),
        );
        let strategy = get_retry_strategy(&err, 1, None);
        assert!(strategy.fix_instruction.contains("fillet/chamfer radius"));
        assert!(strategy.fix_instruction.contains("Reduce"));
        assert_eq!(
            strategy.matching_anti_pattern.as_deref(),
            Some("Fillet radius too large")
        );
    }

    #[test]
    fn test_strategy_shell_attempt1() {
        let err = make_error(
            ErrorCategory::Topology(TopologySubKind::ShellFailure),
            "Shell offset not done",
            Some("shell"),
        );
        let strategy = get_retry_strategy(&err, 1, None);
        assert!(strategy.fix_instruction.contains("shell operation failed"));
        assert!(strategy.forbidden_operations.contains(&"shell".to_string()));
        assert_eq!(
            strategy.matching_anti_pattern.as_deref(),
            Some("Shell on complex boolean body")
        );
    }

    #[test]
    fn test_strategy_boolean_attempt1() {
        let err = make_error(
            ErrorCategory::Topology(TopologySubKind::BooleanFailure),
            "Boolean operation (cut) failed",
            Some("cut"),
        );
        let strategy = get_retry_strategy(&err, 1, None);
        assert!(strategy.fix_instruction.contains("boolean operation"));
        assert!(strategy.fix_instruction.contains("overlap"));
        assert_eq!(
            strategy.matching_anti_pattern.as_deref(),
            Some("Boolean on non-overlapping bodies")
        );
    }

    #[test]
    fn test_strategy_loft_attempt1() {
        let err = make_error(
            ErrorCategory::Topology(TopologySubKind::LoftFailure),
            "loft failed",
            Some("loft"),
        );
        let strategy = get_retry_strategy(&err, 1, None);
        assert!(strategy.fix_instruction.contains("loft failed"));
        assert!(strategy.fix_instruction.contains("edge counts"));
        assert_eq!(
            strategy.matching_anti_pattern.as_deref(),
            Some("Loft with mismatched profiles")
        );
    }

    #[test]
    fn test_strategy_sweep_attempt1() {
        let err = make_error(
            ErrorCategory::Topology(TopologySubKind::SweepFailure),
            "sweep failed",
            Some("sweep"),
        );
        let strategy = get_retry_strategy(&err, 1, None);
        assert!(strategy.fix_instruction.contains("sweep failed"));
        assert!(strategy.fix_instruction.contains("Wire"));
        assert_eq!(
            strategy.matching_anti_pattern.as_deref(),
            Some("Sweep path without .wire()")
        );
    }

    #[test]
    fn test_strategy_revolve_attempt1() {
        let err = make_error(
            ErrorCategory::Topology(TopologySubKind::RevolveFailure),
            "revolve failed",
            Some("revolve"),
        );
        let strategy = get_retry_strategy(&err, 1, None);
        assert!(strategy.fix_instruction.contains("revolve failed"));
        assert!(strategy.fix_instruction.contains("rotation axis"));
        assert_eq!(
            strategy.matching_anti_pattern.as_deref(),
            Some("Revolve profile crossing axis")
        );
    }

    #[test]
    fn test_strategy_api_misuse_attempt1() {
        let err = make_error(
            ErrorCategory::ApiMisuse,
            "translate() requires a 3-tuple",
            Some("translate"),
        );
        let strategy = get_retry_strategy(&err, 1, None);
        assert!(strategy.fix_instruction.contains("API usage error"));
        assert!(strategy.fix_instruction.contains("translate()"));
        assert_eq!(
            strategy.matching_anti_pattern.as_deref(),
            Some("translate() wrong signature")
        );
    }

    #[test]
    fn test_strategy_api_misuse_planar_faces() {
        let err = make_error(
            ErrorCategory::ApiMisuse,
            "If multiple objects selected, they all must be planar faces.",
            None,
        );
        let strategy = get_retry_strategy(&err, 1, None);
        assert!(strategy
            .fix_instruction
            .contains("face selection returned multiple faces"));
        assert!(strategy.forbidden_operations.is_empty());
    }

    #[test]
    fn test_strategy_import_attempt1() {
        let err = make_error(
            ErrorCategory::ImportRuntime,
            "name 'foo' is not defined",
            None,
        );
        let strategy = get_retry_strategy(&err, 1, None);
        assert!(strategy.fix_instruction.contains("import/name error"));
        assert!(strategy.fix_instruction.contains("foo"));
        assert!(strategy.matching_anti_pattern.is_none());
    }

    #[test]
    fn test_strategy_geometry_kernel_attempt1() {
        let err = make_error(ErrorCategory::GeometryKernel, "kernel error", None);
        let strategy = get_retry_strategy(&err, 1, None);
        assert!(strategy.fix_instruction.contains("OpenCascade"));
        assert!(strategy.matching_anti_pattern.is_none());
    }

    #[test]
    fn test_strategy_unknown_attempt1() {
        let err = make_error(ErrorCategory::Unknown, "something broke", None);
        let strategy = get_retry_strategy(&err, 1, None);
        assert!(strategy.fix_instruction.contains("something broke"));
        assert!(strategy.matching_anti_pattern.is_none());
    }

    #[test]
    fn test_strategy_attempt2_adds_simplification() {
        // Topology errors get operation-specific simplification.
        let err = make_error(
            ErrorCategory::Topology(TopologySubKind::FilletFailure),
            "BRep_API: not done",
            Some("fillet"),
        );
        let strategy = get_retry_strategy(&err, 2, None);
        assert!(strategy.fix_instruction.contains("fillet/chamfer radius"));
        assert!(strategy.fix_instruction.contains("simpler geometric"));
        assert!(strategy
            .forbidden_operations
            .contains(&"fillet".to_string()));

        // Non-topology errors get generic simplification.
        let err2 = make_error(ErrorCategory::Syntax, "invalid syntax", None);
        let strategy2 = get_retry_strategy(&err2, 2, None);
        assert!(strategy2.fix_instruction.contains("syntax error"));
        assert!(strategy2
            .fix_instruction
            .contains("simplify the overall approach"));
    }

    #[test]
    fn test_strategy_attempt3_is_nuclear() {
        let err = make_error(
            ErrorCategory::Topology(TopologySubKind::FilletFailure),
            "BRep_API: not done",
            Some("fillet"),
        );
        let strategy = get_retry_strategy(&err, 3, None);
        assert!(strategy.fix_instruction.contains("SIGNIFICANTLY simplify"));
        assert!(strategy.fix_instruction.contains("ONLY basic shapes"));
        assert!(strategy.forbidden_operations.contains(&"sweep".to_string()));
        assert!(strategy.forbidden_operations.contains(&"loft".to_string()));
        assert!(strategy
            .forbidden_operations
            .contains(&"spline".to_string()));
        assert!(strategy
            .forbidden_operations
            .contains(&"revolve".to_string()));
        assert!(strategy.forbidden_operations.contains(&"shell".to_string()));
    }

    // ========== Blanket fillet detection tests ==========

    #[test]
    fn test_strategy_blanket_fillet_suggests_try_except() {
        let err = make_error(
            ErrorCategory::Topology(TopologySubKind::FilletFailure),
            "BRep_API: command not done",
            Some("fillet"),
        );
        let code = "helmet = helmet.cut(visor)\nresult = fillet(helmet.edges(), radius=2.0)";
        let strategy = get_retry_strategy(&err, 1, Some(code));
        assert!(
            strategy.fix_instruction.contains("try/except"),
            "blanket fillet should suggest try/except, got: {}",
            strategy.fix_instruction
        );
        assert!(
            !strategy.fix_instruction.contains("Reduce ALL"),
            "blanket fillet should NOT suggest reducing radius"
        );
        assert_eq!(
            strategy.matching_anti_pattern.as_deref(),
            Some("Blanket fillet on complex geometry")
        );
    }

    #[test]
    fn test_strategy_normal_fillet_still_suggests_radius_reduction() {
        let err = make_error(
            ErrorCategory::Topology(TopologySubKind::FilletFailure),
            "BRep_API: not done",
            Some("fillet"),
        );
        let code = "result = fillet(part.edge_list, radius=8.0)";
        let strategy = get_retry_strategy(&err, 1, Some(code));
        assert!(
            strategy.fix_instruction.contains("Reduce ALL"),
            "non-blanket fillet should suggest reducing radius, got: {}",
            strategy.fix_instruction
        );
        assert_eq!(
            strategy.matching_anti_pattern.as_deref(),
            Some("Fillet radius too large")
        );
    }

    #[test]
    fn test_check_risky_fillet_pattern_loft_plus_fillet() {
        let code = r#"
from build123d import *
with BuildPart() as p:
    with BuildSketch():
        Circle(50)
    with BuildSketch(Plane.XY.offset(80)):
        Circle(30)
    loft()
    fillet(p.edges(), radius=2.0)
result = p.part
"#;
        let warning = check_risky_fillet_pattern(code);
        assert!(warning.is_some(), "should detect loft + blanket fillet");
        assert!(warning.unwrap().contains("high failure risk"));
    }

    #[test]
    fn test_check_risky_fillet_pattern_simple_box_ok() {
        let code = r#"
from build123d import *
result = Box(10, 10, 10)
result = fillet(result.edges().filter_by(Axis.Z), radius=1.0)
"#;
        assert!(
            check_risky_fillet_pattern(code).is_none(),
            "simple selective fillet on a box should not trigger warning"
        );
    }

    #[test]
    fn test_check_risky_fillet_pattern_multi_boolean() {
        let code = r#"
from build123d import *
with BuildPart() as p:
    Box(100, 100, 50)
    with Locations((0, 0)):
        Cylinder(10, 60, mode=Mode.SUBTRACT)
    with Locations((20, 0)):
        Cylinder(10, 60, mode=Mode.SUBTRACT)
    body = p.part
body = body.cut(Cylinder(10, 60))
body = body.cut(Cylinder(10, 60))
body = body.cut(Cylinder(10, 60))
fillet(body.edges(), radius=2.0)
result = body
"#;
        let warning = check_risky_fillet_pattern(code);
        assert!(
            warning.is_some(),
            "should detect multi-boolean + blanket fillet"
        );
    }

    // ========== DisconnectedSolids detection tests ==========

    #[test]
    fn test_parse_traceback_disconnected_solids_exit_code_5() {
        let stderr = "Result contains multiple disconnected solids — a cut likely went through a wall and split the body. Reduce cut depth or increase wall thickness.";
        let err = parse_traceback(stderr);
        assert_eq!(
            err.category,
            ErrorCategory::Topology(TopologySubKind::DisconnectedSolids)
        );
        assert_eq!(err.error_type, "DisconnectedSolids");
        assert!(err.suggestion.unwrap().contains("wall thickness"));
    }

    #[test]
    fn test_parse_traceback_split_body_marker() {
        let stderr = "SPLIT_BODY: result has 3 disconnected solids (fuse failed)";
        let err = parse_traceback(stderr);
        assert_eq!(
            err.category,
            ErrorCategory::Topology(TopologySubKind::DisconnectedSolids)
        );
    }

    #[test]
    fn test_strategy_disconnected_solids_attempt1() {
        let err = make_error(
            ErrorCategory::Topology(TopologySubKind::DisconnectedSolids),
            "Result contains multiple disconnected solids",
            Some("cut"),
        );
        let strategy = get_retry_strategy(&err, 1, None);
        assert!(
            strategy.fix_instruction.contains("SPLIT"),
            "should mention split body, got: {}",
            strategy.fix_instruction
        );
        assert!(strategy.fix_instruction.contains("80%"));
        assert_eq!(
            strategy.matching_anti_pattern.as_deref(),
            Some("Through-cut splitting body")
        );
    }
}
