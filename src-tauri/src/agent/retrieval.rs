use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::agent::rules::{
    AgentRules, AntiPatternEntry, ApiReferenceEntry, CookbookEntry, DesignPatternEntry,
    FewShotExample,
};
use crate::config::AppConfig;

const DEFAULT_EMBEDDING_MODEL: &str = "text-embedding-3-small";
const DEFAULT_RETRIEVAL_BUDGET: u32 = 3500;

const MAX_COOKBOOK: usize = 4;
const MAX_ANTI_PATTERNS: usize = 3;
const MAX_API_REF: usize = 4;
const MAX_FEW_SHOT: usize = 2;
const MAX_DESIGN_PATTERNS: usize = 2;

#[derive(Debug, Clone, Serialize)]
pub struct RetrievedContextItem {
    pub source: String,
    pub id: String,
    pub title: String,
    pub score: f32,
}

#[derive(Debug, Clone)]
struct IndexedItem {
    source: String,
    id: String,
    title: String,
    body: String,
}

#[derive(Debug, Clone)]
pub struct RetrievalResult {
    pub items: Vec<RetrievedContextItem>,
    pub context_markdown: String,
    pub used_embeddings: bool,
    pub lexical_fallback: bool,
}

impl RetrievalResult {
    pub fn empty() -> Self {
        Self {
            items: Vec::new(),
            context_markdown: String::new(),
            used_embeddings: false,
            lexical_fallback: false,
        }
    }
}

static INDEX_CACHE: OnceLock<Mutex<HashMap<String, Vec<IndexedItem>>>> = OnceLock::new();

fn get_index_cache() -> &'static Mutex<HashMap<String, Vec<IndexedItem>>> {
    INDEX_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn make_cache_key(preset: Option<&str>, cq_version: Option<&str>) -> String {
    format!(
        "preset:{}|cq:{}",
        preset.unwrap_or("default"),
        cq_version.unwrap_or("unknown")
    )
}

fn build_index(preset: Option<&str>, cq_version: Option<&str>) -> Vec<IndexedItem> {
    let rules = AgentRules::from_preset(preset).unwrap_or_else(|_| {
        AgentRules::from_preset(None).unwrap_or_else(|_| AgentRules::default_empty())
    });

    let mut docs: Vec<IndexedItem> = Vec::new();

    if let Some(cookbook) = rules.cookbook {
        for (i, entry) in cookbook.iter().enumerate() {
            if let Some(min_ver) = &entry.min_version {
                if let Some(ver) = cq_version {
                    if !crate::python::installer::version_gte(ver, min_ver) {
                        continue;
                    }
                }
            }
            docs.push(index_cookbook(i, entry));
        }
    }

    if let Some(anti_patterns) = rules.anti_patterns {
        for (i, entry) in anti_patterns.iter().enumerate() {
            docs.push(index_anti_pattern(i, entry));
        }
    }

    if let Some(api_ref) = rules.api_reference {
        for (i, entry) in api_ref.iter().enumerate() {
            docs.push(index_api_ref(i, entry));
        }
    }

    if let Some(examples) = rules.few_shot_examples {
        for (i, entry) in examples.iter().enumerate() {
            docs.push(index_few_shot(i, entry));
        }
    }

    if let Some(patterns) = rules.design_patterns {
        for (i, entry) in patterns.iter().enumerate() {
            docs.push(index_design_pattern(i, entry));
        }
    }

    docs
}

fn index_cookbook(i: usize, entry: &CookbookEntry) -> IndexedItem {
    let desc = entry.description.clone().unwrap_or_default();
    IndexedItem {
        source: "cookbook".to_string(),
        id: format!("cookbook:{}", i),
        title: entry.title.clone(),
        body: format!("{}\n{}\n{}", entry.title, desc, truncate(&entry.code, 1000)),
    }
}

fn index_anti_pattern(i: usize, entry: &AntiPatternEntry) -> IndexedItem {
    IndexedItem {
        source: "anti_pattern".to_string(),
        id: format!("anti_pattern:{}", i),
        title: entry.title.clone(),
        body: format!(
            "{}\n{}\nWrong:\n{}\nCorrect:\n{}",
            entry.title,
            entry.explanation,
            truncate(&entry.wrong_code, 600),
            truncate(&entry.correct_code, 600)
        ),
    }
}

fn index_api_ref(i: usize, entry: &ApiReferenceEntry) -> IndexedItem {
    IndexedItem {
        source: "api_ref".to_string(),
        id: format!("api_ref:{}", i),
        title: entry.operation.clone(),
        body: format!(
            "{}\n{}\n{}\n{}",
            entry.operation,
            entry.signature,
            entry.params.join("; "),
            entry.gotchas.join("; ")
        ),
    }
}

fn index_few_shot(i: usize, entry: &FewShotExample) -> IndexedItem {
    IndexedItem {
        source: "few_shot".to_string(),
        id: format!("few_shot:{}", i),
        title: format!("Few-shot {}", i + 1),
        body: format!(
            "Request: {}\nPlan: {}\nCode:\n{}",
            entry.user_request,
            truncate(&entry.design_plan, 600),
            truncate(&entry.code, 900)
        ),
    }
}

fn index_design_pattern(i: usize, entry: &DesignPatternEntry) -> IndexedItem {
    IndexedItem {
        source: "design_pattern".to_string(),
        id: format!("design_pattern:{}", i),
        title: entry.name.clone(),
        body: format!(
            "{}\n{}\nKeywords: {}\nParameters: {}\nGotchas: {}\n{}",
            entry.name,
            entry.description,
            entry.keywords.join(", "),
            entry.parameters.join(", "),
            entry.gotchas.join("; "),
            truncate(&entry.base_code, 800)
        ),
    }
}

fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-')
        .filter(|s| s.len() > 2)
        .map(|s| s.to_string())
        .collect()
}

fn lexical_score(query: &str, doc: &IndexedItem) -> f32 {
    let q = tokenize(query);
    if q.is_empty() {
        return 0.0;
    }

    let text = format!("{}\n{}", doc.title, doc.body).to_lowercase();
    let mut seen = HashSet::new();
    let mut overlap = 0.0f32;

    for token in &q {
        if seen.contains(token) {
            continue;
        }
        seen.insert(token.clone());

        if text.contains(token) {
            overlap += if doc.title.to_lowercase().contains(token) {
                2.0
            } else {
                1.0
            };
        }
    }

    let mut bonus = 0.0;
    if text.contains("shell") && query.to_lowercase().contains("shell") {
        bonus += 0.8;
    }
    if text.contains("fillet") && query.to_lowercase().contains("fillet") {
        bonus += 0.8;
    }
    if text.contains("enclosure") && query.to_lowercase().contains("enclosure") {
        bonus += 0.8;
    }

    overlap / q.len() as f32 + bonus
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    dot / (na.sqrt() * nb.sqrt())
}

#[derive(Debug, Deserialize)]
struct EmbeddingsResponse {
    data: Vec<EmbeddingData>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
}

async fn fetch_embeddings(config: &AppConfig, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
    if texts.is_empty() {
        return Ok(Vec::new());
    }

    let api_key = config
        .api_key
        .clone()
        .ok_or_else(|| "missing api key".to_string())?;

    let base_url = config
        .openai_base_url
        .clone()
        .filter(|u| !u.trim().is_empty())
        .unwrap_or_else(|| "https://api.openai.com/v1".to_string());

    let url = format!("{}/embeddings", base_url.trim_end_matches('/'));

    let client = Client::new();
    let body = serde_json::json!({
        "model": DEFAULT_EMBEDDING_MODEL,
        "input": texts,
    });

    let resp = client
        .post(url)
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("embedding request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("embedding HTTP {}: {}", status, text));
    }

    let parsed: EmbeddingsResponse = resp
        .json()
        .await
        .map_err(|e| format!("embedding parse failed: {}", e))?;

    Ok(parsed.data.into_iter().map(|d| d.embedding).collect())
}

fn source_limit(source: &str) -> usize {
    match source {
        "cookbook" => MAX_COOKBOOK,
        "anti_pattern" => MAX_ANTI_PATTERNS,
        "api_ref" => MAX_API_REF,
        "few_shot" => MAX_FEW_SHOT,
        "design_pattern" => MAX_DESIGN_PATTERNS,
        _ => 1,
    }
}

fn render_item(item: &IndexedItem, score: f32) -> String {
    match item.source.as_str() {
        "cookbook" => format!(
            "### Cookbook: {} (score {:.2})\n```text\n{}\n```\n",
            item.title,
            score,
            truncate(&item.body, 900)
        ),
        "anti_pattern" => format!(
            "### Anti-pattern: {} (score {:.2})\n```text\n{}\n```\n",
            item.title,
            score,
            truncate(&item.body, 850)
        ),
        "api_ref" => format!(
            "### API Reference: {} (score {:.2})\n```text\n{}\n```\n",
            item.title,
            score,
            truncate(&item.body, 700)
        ),
        "few_shot" => format!(
            "### Few-shot: {} (score {:.2})\n```text\n{}\n```\n",
            item.title,
            score,
            truncate(&item.body, 950)
        ),
        "design_pattern" => format!(
            "### Design Pattern: {} (score {:.2})\n```text\n{}\n```\n",
            item.title,
            score,
            truncate(&item.body, 850)
        ),
        _ => format!("### {}\n{}\n", item.title, truncate(&item.body, 600)),
    }
}

fn approx_tokens(s: &str) -> u32 {
    ((s.len() as f32) / 4.0).ceil() as u32
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.len() <= max_chars {
        s.to_string()
    } else {
        format!("{}...", &s[..max_chars])
    }
}

pub async fn retrieve_context(
    query: &str,
    config: &AppConfig,
    preset: Option<&str>,
    cq_version: Option<&str>,
) -> RetrievalResult {
    if !config.retrieval_enabled {
        return RetrievalResult::empty();
    }

    let key = make_cache_key(preset, cq_version);
    let docs = {
        let cache = get_index_cache();
        let mut guard = cache.lock().unwrap();
        if !guard.contains_key(&key) {
            guard.insert(key.clone(), build_index(preset, cq_version));
        }
        guard.get(&key).cloned().unwrap_or_default()
    };

    if docs.is_empty() || query.trim().is_empty() {
        return RetrievalResult::empty();
    }

    let mut scored: Vec<(usize, f32, f32)> = docs
        .iter()
        .enumerate()
        .map(|(idx, doc)| (idx, lexical_score(query, doc), 0.0f32))
        .collect();

    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));
    let lexical_top_n = scored.iter().take(24).map(|t| t.0).collect::<Vec<_>>();

    let mut used_embeddings = false;
    let mut lexical_fallback = true;

    let embed_input = lexical_top_n
        .iter()
        .map(|idx| format!("{}\n{}", docs[*idx].title, docs[*idx].body))
        .collect::<Vec<_>>();

    if let Ok(vecs) = fetch_embeddings(
        config,
        &std::iter::once(query.to_string())
            .chain(embed_input.into_iter())
            .collect::<Vec<_>>(),
    )
    .await
    {
        if vecs.len() == lexical_top_n.len() + 1 {
            let qv = &vecs[0];
            for (i, doc_idx) in lexical_top_n.iter().enumerate() {
                let sim = cosine_similarity(qv, &vecs[i + 1]);
                if let Some(tuple) = scored.iter_mut().find(|t| t.0 == *doc_idx) {
                    tuple.2 = sim;
                }
            }
            used_embeddings = true;
            lexical_fallback = false;
        }
    }

    scored.sort_by(|a, b| {
        let ascore = if used_embeddings {
            a.1 * 0.35 + a.2 * 0.65
        } else {
            a.1
        };
        let bscore = if used_embeddings {
            b.1 * 0.35 + b.2 * 0.65
        } else {
            b.1
        };
        bscore.partial_cmp(&ascore).unwrap_or(Ordering::Equal)
    });

    let mut per_source_count: HashMap<String, usize> = HashMap::new();
    let mut selected: Vec<(usize, f32)> = Vec::new();

    for (idx, lex_score, emb_score) in scored {
        let doc = &docs[idx];
        let score = if used_embeddings {
            lex_score * 0.35 + emb_score * 0.65
        } else {
            lex_score
        };

        if score <= 0.01 {
            continue;
        }

        let entry = per_source_count.entry(doc.source.clone()).or_insert(0);
        if *entry >= source_limit(&doc.source) {
            continue;
        }

        selected.push((idx, score));
        *entry += 1;

        if selected.len()
            >= (MAX_COOKBOOK + MAX_ANTI_PATTERNS + MAX_API_REF + MAX_FEW_SHOT + MAX_DESIGN_PATTERNS)
        {
            break;
        }
    }

    let budget = retrieval_budget_or_default(config).max(500);
    let mut used_budget = 0u32;

    let mut items: Vec<RetrievedContextItem> = Vec::new();
    let mut context_markdown = String::from(
        "## Retrieved CAD Guidance\nUse these retrieved snippets as high-priority references.\n\n",
    );

    for (idx, score) in selected {
        let doc = &docs[idx];
        let section = render_item(doc, score);
        let section_tokens = approx_tokens(&section);
        if used_budget + section_tokens > budget {
            continue;
        }

        used_budget += section_tokens;
        context_markdown.push_str(&section);
        context_markdown.push('\n');

        items.push(RetrievedContextItem {
            source: doc.source.clone(),
            id: doc.id.clone(),
            title: doc.title.clone(),
            score,
        });
    }

    if items.is_empty() {
        return RetrievalResult::empty();
    }

    RetrievalResult {
        items,
        context_markdown,
        used_embeddings,
        lexical_fallback,
    }
}

pub fn retrieval_budget_or_default(config: &AppConfig) -> u32 {
    let v = config.retrieval_token_budget;
    if v == 0 {
        DEFAULT_RETRIEVAL_BUDGET
    } else {
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lexical_prefers_shell_query() {
        let doc = IndexedItem {
            source: "cookbook".to_string(),
            id: "x".to_string(),
            title: "Hollow enclosure with shell".to_string(),
            body: "Use shell after subtracting interior".to_string(),
        };
        let score = lexical_score("design a shell enclosure", &doc);
        assert!(score > 0.8);
    }

    #[test]
    fn test_cosine_similarity_basic() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let c = vec![0.0, 1.0, 0.0];
        assert!(cosine_similarity(&a, &b) > 0.99);
        assert!(cosine_similarity(&a, &c) < 0.1);
    }

    #[test]
    fn test_budget_default() {
        let mut cfg = AppConfig::default();
        cfg.retrieval_token_budget = 0;
        assert_eq!(retrieval_budget_or_default(&cfg), DEFAULT_RETRIEVAL_BUDGET);
    }
}
