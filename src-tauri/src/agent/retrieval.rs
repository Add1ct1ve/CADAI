use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::agent::rules::{
    AgentRules, AntiPatternEntry, ApiReferenceEntry, CookbookEntry, DesignPatternEntry,
    FewShotExample,
};
use crate::ai::capabilities::{context_bucket_for_model, ContextBucket};
use crate::config::AppConfig;
use crate::mechanisms::catalog as mechanism_catalog;

const DEFAULT_EMBEDDING_MODEL: &str = "text-embedding-3-small";
const DEFAULT_RETRIEVAL_BUDGET: u32 = 12_000;

#[derive(Debug, Clone, Copy)]
struct RetrievalCaps {
    cookbook: usize,
    anti_patterns: usize,
    api_ref: usize,
    few_shot: usize,
    design_patterns: usize,
    mechanisms: usize,
}

impl RetrievalCaps {
    fn total_max(self) -> usize {
        self.cookbook
            + self.anti_patterns
            + self.api_ref
            + self.few_shot
            + self.design_patterns
            + self.mechanisms
    }
}

#[derive(Debug, Clone, Copy)]
struct RetrievalTruncation {
    cookbook: usize,
    anti_pattern: usize,
    api_ref: usize,
    few_shot: usize,
    design_pattern: usize,
    mechanism: usize,
}

impl RetrievalTruncation {
    fn for_source(self, source: &str) -> usize {
        match source {
            "cookbook" => self.cookbook,
            "anti_pattern" => self.anti_pattern,
            "api_ref" => self.api_ref,
            "few_shot" => self.few_shot,
            "design_pattern" => self.design_pattern,
            "mechanism" => self.mechanism,
            _ => 600,
        }
    }
}

fn retrieval_caps_for_bucket(bucket: ContextBucket) -> RetrievalCaps {
    match bucket {
        ContextBucket::K128 => RetrievalCaps {
            cookbook: 8,
            anti_patterns: 6,
            api_ref: 8,
            few_shot: 4,
            design_patterns: 4,
            mechanisms: 12,
        },
        ContextBucket::K200 => RetrievalCaps {
            cookbook: 12,
            anti_patterns: 9,
            api_ref: 12,
            few_shot: 6,
            design_patterns: 6,
            mechanisms: 18,
        },
        ContextBucket::K400 => RetrievalCaps {
            cookbook: 16,
            anti_patterns: 12,
            api_ref: 16,
            few_shot: 8,
            design_patterns: 8,
            mechanisms: 24,
        },
        ContextBucket::K1M => RetrievalCaps {
            cookbook: 24,
            anti_patterns: 18,
            api_ref: 24,
            few_shot: 12,
            design_patterns: 12,
            mechanisms: 36,
        },
    }
}

fn retrieval_truncation_for_bucket(bucket: ContextBucket) -> RetrievalTruncation {
    match bucket {
        ContextBucket::K128 => RetrievalTruncation {
            cookbook: 900,
            anti_pattern: 850,
            api_ref: 700,
            few_shot: 950,
            design_pattern: 850,
            mechanism: 900,
        },
        ContextBucket::K200 => RetrievalTruncation {
            cookbook: 1350,
            anti_pattern: 1275,
            api_ref: 1050,
            few_shot: 1425,
            design_pattern: 1275,
            mechanism: 1350,
        },
        ContextBucket::K400 => RetrievalTruncation {
            cookbook: 1800,
            anti_pattern: 1700,
            api_ref: 1400,
            few_shot: 1900,
            design_pattern: 1700,
            mechanism: 1800,
        },
        ContextBucket::K1M => RetrievalTruncation {
            cookbook: 2700,
            anti_pattern: 2550,
            api_ref: 2100,
            few_shot: 2850,
            design_pattern: 2550,
            mechanism: 2700,
        },
    }
}

fn max_truncation_for_source(source: &str) -> usize {
    retrieval_truncation_for_bucket(ContextBucket::K1M).for_source(source)
}

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

fn mechanism_docs(config: &AppConfig) -> Vec<IndexedItem> {
    if !config.mechanisms_enabled {
        return Vec::new();
    }
    let catalog = match mechanism_catalog::get_catalog(config) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    catalog
        .mechanisms
        .into_iter()
        .map(|m| IndexedItem {
            source: "mechanism".to_string(),
            id: format!("mechanism:{}", m.id),
            title: format!("{} ({})", m.title, m.id),
            body: format!(
                "{}\nCategory: {}\nKeywords: {}\nParameters: {}\n{}",
                m.summary,
                m.category,
                m.keywords.join(", "),
                m.parameters
                    .iter()
                    .map(|p| format!("{}={}", p.name, p.default_value))
                    .collect::<Vec<_>>()
                    .join(", "),
                truncate(&m.prompt_block, max_truncation_for_source("mechanism"))
            ),
        })
        .collect()
}

fn index_cookbook(i: usize, entry: &CookbookEntry) -> IndexedItem {
    let desc = entry.description.clone().unwrap_or_default();
    let body = format!("{}\n{}\n{}", entry.title, desc, entry.code);
    IndexedItem {
        source: "cookbook".to_string(),
        id: format!("cookbook:{}", i),
        title: entry.title.clone(),
        body: truncate(&body, max_truncation_for_source("cookbook")),
    }
}

fn index_anti_pattern(i: usize, entry: &AntiPatternEntry) -> IndexedItem {
    let body = format!(
        "{}\n{}\nWrong:\n{}\nCorrect:\n{}",
        entry.title, entry.explanation, entry.wrong_code, entry.correct_code
    );
    IndexedItem {
        source: "anti_pattern".to_string(),
        id: format!("anti_pattern:{}", i),
        title: entry.title.clone(),
        body: truncate(&body, max_truncation_for_source("anti_pattern")),
    }
}

fn index_api_ref(i: usize, entry: &ApiReferenceEntry) -> IndexedItem {
    let body = format!(
        "{}\n{}\n{}\n{}",
        entry.operation,
        entry.signature,
        entry.params.join("; "),
        entry.gotchas.join("; ")
    );
    IndexedItem {
        source: "api_ref".to_string(),
        id: format!("api_ref:{}", i),
        title: entry.operation.clone(),
        body: truncate(&body, max_truncation_for_source("api_ref")),
    }
}

fn index_few_shot(i: usize, entry: &FewShotExample) -> IndexedItem {
    let body = format!(
        "Request: {}\nPlan: {}\nCode:\n{}",
        entry.user_request, entry.design_plan, entry.code
    );
    IndexedItem {
        source: "few_shot".to_string(),
        id: format!("few_shot:{}", i),
        title: format!("Few-shot {}", i + 1),
        body: truncate(&body, max_truncation_for_source("few_shot")),
    }
}

fn index_design_pattern(i: usize, entry: &DesignPatternEntry) -> IndexedItem {
    let body = format!(
        "{}\n{}\nKeywords: {}\nParameters: {}\nGotchas: {}\n{}",
        entry.name,
        entry.description,
        entry.keywords.join(", "),
        entry.parameters.join(", "),
        entry.gotchas.join("; "),
        entry.base_code
    );
    IndexedItem {
        source: "design_pattern".to_string(),
        id: format!("design_pattern:{}", i),
        title: entry.name.clone(),
        body: truncate(&body, max_truncation_for_source("design_pattern")),
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

fn source_limit(source: &str, caps: RetrievalCaps) -> usize {
    match source {
        "cookbook" => caps.cookbook,
        "anti_pattern" => caps.anti_patterns,
        "api_ref" => caps.api_ref,
        "few_shot" => caps.few_shot,
        "design_pattern" => caps.design_patterns,
        "mechanism" => caps.mechanisms,
        _ => 1,
    }
}

fn render_item(item: &IndexedItem, score: f32, truncation: RetrievalTruncation) -> String {
    let max_len = truncation.for_source(item.source.as_str());
    match item.source.as_str() {
        "cookbook" => format!(
            "### Cookbook: {} (score {:.2})\n```text\n{}\n```\n",
            item.title,
            score,
            truncate(&item.body, max_len)
        ),
        "anti_pattern" => format!(
            "### Anti-pattern: {} (score {:.2})\n```text\n{}\n```\n",
            item.title,
            score,
            truncate(&item.body, max_len)
        ),
        "api_ref" => format!(
            "### API Reference: {} (score {:.2})\n```text\n{}\n```\n",
            item.title,
            score,
            truncate(&item.body, max_len)
        ),
        "few_shot" => format!(
            "### Few-shot: {} (score {:.2})\n```text\n{}\n```\n",
            item.title,
            score,
            truncate(&item.body, max_len)
        ),
        "design_pattern" => format!(
            "### Design Pattern: {} (score {:.2})\n```text\n{}\n```\n",
            item.title,
            score,
            truncate(&item.body, max_len)
        ),
        "mechanism" => format!(
            "### Mechanism Library: {} (score {:.2})\n```text\n{}\n```\n",
            item.title,
            score,
            truncate(&item.body, max_len)
        ),
        _ => format!("### {}\n{}\n", item.title, truncate(&item.body, max_len)),
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

    let bucket = context_bucket_for_model(&config.ai_provider, &config.model);
    let caps = retrieval_caps_for_bucket(bucket);
    let truncation = retrieval_truncation_for_bucket(bucket);

    let key = make_cache_key(preset, cq_version);
    let mut docs = {
        let cache = get_index_cache();
        let mut guard = cache.lock().unwrap();
        if !guard.contains_key(&key) {
            guard.insert(key.clone(), build_index(preset, cq_version));
        }
        guard.get(&key).cloned().unwrap_or_default()
    };

    docs.extend(mechanism_docs(config));

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
        .map(|idx| {
            let doc = &docs[*idx];
            let body = truncate(&doc.body, truncation.for_source(&doc.source));
            format!("{}\n{}", doc.title, body)
        })
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
        if *entry >= source_limit(&doc.source, caps) {
            continue;
        }

        selected.push((idx, score));
        *entry += 1;

        if selected.len() >= caps.total_max() {
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
        let section = render_item(doc, score, truncation);
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
    match config.retrieval_budget_mode {
        crate::config::RetrievalBudgetMode::Fixed => {
            let v = config.retrieval_token_budget;
            if v == 0 {
                DEFAULT_RETRIEVAL_BUDGET
            } else {
                v
            }
        }
        crate::config::RetrievalBudgetMode::Adaptive => {
            match context_bucket_for_model(&config.ai_provider, &config.model) {
                ContextBucket::K128 => config.retrieval_budget_128k,
                ContextBucket::K200 => config.retrieval_budget_200k,
                ContextBucket::K400 => config.retrieval_budget_400k,
                ContextBucket::K1M => config.retrieval_budget_1m,
            }
        }
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
        cfg.retrieval_budget_mode = crate::config::RetrievalBudgetMode::Fixed;
        assert_eq!(retrieval_budget_or_default(&cfg), DEFAULT_RETRIEVAL_BUDGET);
    }

    #[test]
    fn test_budget_adaptive_examples() {
        let mut cfg = AppConfig::default();
        cfg.retrieval_budget_mode = crate::config::RetrievalBudgetMode::Adaptive;

        cfg.ai_provider = "deepseek".to_string();
        cfg.model = "deepseek-reasoner".to_string();
        assert_eq!(retrieval_budget_or_default(&cfg), cfg.retrieval_budget_128k);

        cfg.ai_provider = "claude".to_string();
        cfg.model = "claude-sonnet-4-5-20250929".to_string();
        assert_eq!(retrieval_budget_or_default(&cfg), cfg.retrieval_budget_200k);

        cfg.ai_provider = "openai".to_string();
        cfg.model = "gpt-5.2".to_string();
        assert_eq!(retrieval_budget_or_default(&cfg), cfg.retrieval_budget_400k);

        cfg.ai_provider = "gemini".to_string();
        cfg.model = "gemini-2.5-pro".to_string();
        assert_eq!(retrieval_budget_or_default(&cfg), cfg.retrieval_budget_1m);
    }

    #[test]
    fn test_caps_by_bucket() {
        let caps_128 = retrieval_caps_for_bucket(ContextBucket::K128);
        assert_eq!(caps_128.cookbook, 8);
        assert_eq!(caps_128.mechanisms, 12);

        let caps_200 = retrieval_caps_for_bucket(ContextBucket::K200);
        assert_eq!(caps_200.anti_patterns, 9);

        let caps_400 = retrieval_caps_for_bucket(ContextBucket::K400);
        assert_eq!(caps_400.api_ref, 16);

        let caps_1m = retrieval_caps_for_bucket(ContextBucket::K1M);
        assert_eq!(caps_1m.few_shot, 12);
        assert_eq!(caps_1m.total_max(), 24 + 18 + 24 + 12 + 12 + 36);
    }

    #[test]
    fn test_truncation_by_bucket() {
        let trunc_128 = retrieval_truncation_for_bucket(ContextBucket::K128);
        assert_eq!(trunc_128.cookbook, 900);
        assert_eq!(trunc_128.api_ref, 700);

        let trunc_1m = retrieval_truncation_for_bucket(ContextBucket::K1M);
        assert_eq!(trunc_1m.few_shot, 2850);
        assert_eq!(trunc_1m.design_pattern, 2550);
    }
}
