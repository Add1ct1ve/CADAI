use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::error::AppError;

#[derive(Debug, Clone, Serialize)]
pub struct GenerationTraceV1 {
    pub version: u32,
    pub timestamp_ms: u64,
    pub request_hash: String,
    pub intent_tags: Vec<String>,
    pub provider: String,
    pub model: String,
    pub retrieved_items: Vec<TraceRetrievedItem>,
    pub plan_risk_score: Option<u32>,
    pub confidence_score: Option<u32>,
    pub static_findings: Vec<String>,
    pub execution_success: bool,
    pub retry_attempts: Option<u32>,
    pub final_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TraceRetrievedItem {
    pub source: String,
    pub id: String,
    pub title: String,
    pub score: f32,
}

fn fnv1a64(input: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in input.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

pub fn hash_request(text: &str) -> String {
    format!("{:016x}", fnv1a64(text))
}

pub fn infer_intent_tags(text: &str) -> Vec<String> {
    let lower = text.to_lowercase();
    let mut tags = Vec::new();

    let checks: [(&str, &[&str]); 6] = [
        ("enclosure", &["enclosure", "housing", "case", "lid"]),
        ("mechanical", &["bracket", "shaft", "gear", "bolt", "hole"]),
        ("wearable", &["wrist", "band", "tracker", "watch"]),
        ("organic_approx", &["smooth", "organic", "helmet", "face"]),
        ("assembly", &["assembly", "fit", "snap", "plate"]),
        ("printable", &["3d print", "printable", "fdm", "resin"]),
    ];

    for (tag, keywords) in checks {
        if keywords.iter().any(|k| lower.contains(k)) {
            tags.push(tag.to_string());
        }
    }

    if tags.is_empty() {
        tags.push("generic".to_string());
    }

    tags
}

fn telemetry_dir() -> Result<PathBuf, AppError> {
    let base = dirs::config_dir()
        .ok_or_else(|| AppError::ConfigError("Cannot resolve config directory".to_string()))?;
    Ok(base.join("cadai-studio").join("telemetry"))
}

pub fn write_trace(trace: &GenerationTraceV1) -> Result<(), AppError> {
    let dir = telemetry_dir()?;
    fs::create_dir_all(&dir)?;
    let path = dir.join("generation_traces_v1.jsonl");

    let mut file = OpenOptions::new().create(true).append(true).open(path)?;

    let line = serde_json::to_string(trace)?;
    writeln!(file, "{}", line)?;
    Ok(())
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_request_stable() {
        let a = hash_request("make a box");
        let b = hash_request("make a box");
        assert_eq!(a, b);
    }

    #[test]
    fn test_intent_tags_detected() {
        let tags = infer_intent_tags("Create a wrist tracker enclosure with snap fit");
        assert!(tags.contains(&"wearable".to_string()));
        assert!(tags.contains(&"enclosure".to_string()));
        assert!(tags.contains(&"assembly".to_string()));
    }
}
