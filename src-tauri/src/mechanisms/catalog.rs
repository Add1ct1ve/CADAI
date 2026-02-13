use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use crate::config::AppConfig;
use crate::error::AppError;

use super::schema::{
    CatalogMechanism, CatalogPackage, ManifestMechanismEntry, MechanismCatalog,
    MechanismPackageManifest, MechanismRecord,
};

static CATALOG_CACHE: OnceLock<Mutex<Option<MechanismCatalog>>> = OnceLock::new();

fn cache() -> &'static Mutex<Option<MechanismCatalog>> {
    CATALOG_CACHE.get_or_init(|| Mutex::new(None))
}

pub fn invalidate_cache() {
    if let Ok(mut guard) = cache().lock() {
        *guard = None;
    }
}

fn imported_root() -> Result<PathBuf, AppError> {
    let base = dirs::config_dir()
        .ok_or_else(|| AppError::ConfigError("Cannot resolve config directory".to_string()))?;
    Ok(base
        .join("cadai-studio")
        .join("mechanisms")
        .join("imported"))
}

pub fn workspace_root() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn list_roots() -> Vec<PathBuf> {
    let mut roots = vec![workspace_root().join("mechanisms")];
    if let Ok(imported) = imported_root() {
        roots.push(imported);
    }

    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for root in roots {
        let key = root.to_string_lossy().to_string();
        if seen.insert(key) {
            out.push(root);
        }
    }
    out
}

fn collect_manifest_files(root: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > 4 || !root.exists() {
        return;
    }
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_manifest_files(&path, depth + 1, out);
        } else if path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.eq_ignore_ascii_case("manifest.json"))
            .unwrap_or(false)
        {
            out.push(path);
        }
    }
}

fn parse_manifest(path: &Path) -> Result<MechanismPackageManifest, AppError> {
    let raw = std::fs::read_to_string(path)?;
    serde_json::from_str::<MechanismPackageManifest>(&raw).map_err(|e| {
        AppError::ConfigError(format!(
            "Invalid mechanism manifest {}: {}",
            path.display(),
            e
        ))
    })
}

fn load_local_entry(manifest_dir: &Path, file: &str) -> Result<MechanismRecord, AppError> {
    let path = manifest_dir.join(file);
    let raw = std::fs::read_to_string(&path)?;
    serde_json::from_str::<MechanismRecord>(&raw).map_err(|e| {
        AppError::ConfigError(format!("Invalid mechanism file {}: {}", path.display(), e))
    })
}

fn resolve_entries(
    manifest: &MechanismPackageManifest,
    manifest_path: &Path,
) -> Result<Vec<MechanismRecord>, AppError> {
    let mut out = Vec::new();
    let dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));

    for entry in &manifest.mechanisms {
        let record = match entry {
            ManifestMechanismEntry::Inline(record) => record.clone(),
            ManifestMechanismEntry::FileRef { file, .. } => load_local_entry(dir, file)?,
        };
        out.push(record);
    }

    Ok(out)
}

pub fn get_catalog(config: &AppConfig) -> Result<MechanismCatalog, AppError> {
    if !config.mechanisms_enabled {
        return Ok(MechanismCatalog {
            packages: Vec::new(),
            mechanisms: Vec::new(),
        });
    }

    if let Ok(guard) = cache().lock() {
        if let Some(existing) = guard.clone() {
            return Ok(existing);
        }
    }

    let mut manifests = Vec::new();
    for root in list_roots() {
        collect_manifest_files(&root, 0, &mut manifests);
    }

    let mut packages = Vec::new();
    let mut mechanisms = Vec::new();
    let mut seen_mechanism = HashSet::new();
    let mut seen_package = HashMap::<String, usize>::new();

    let imported_base = imported_root().ok();

    for manifest_path in manifests {
        let manifest = match parse_manifest(&manifest_path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let is_imported = imported_base
            .as_ref()
            .map(|p| manifest_path.starts_with(p))
            .unwrap_or(false);

        let records = match resolve_entries(&manifest, &manifest_path) {
            Ok(r) => r,
            Err(_) => continue,
        };

        for record in records {
            let dedupe_key = format!("{}::{}", manifest.package_id, record.id);
            if !seen_mechanism.insert(dedupe_key) {
                continue;
            }

            mechanisms.push(CatalogMechanism {
                package_id: manifest.package_id.clone(),
                package_name: manifest.name.clone(),
                package_version: manifest.version.clone(),
                id: record.id,
                title: record.title,
                summary: record.summary,
                category: record.category,
                keywords: record.keywords,
                prompt_block: record.prompt_block,
                license: record.license,
                source_url: record.source_url,
                preview_url: record.preview_url,
                parameters: record.parameters,
            });
        }

        let count = seen_package.entry(manifest.package_id.clone()).or_insert(0);
        *count += 1;

        if *count == 1 {
            packages.push(CatalogPackage {
                package_id: manifest.package_id,
                name: manifest.name,
                version: manifest.version,
                license: manifest.license,
                source: manifest.source,
                homepage: manifest.homepage,
                mechanism_count: 0,
                is_imported,
            });
        }
    }

    let mut count_map: HashMap<String, usize> = HashMap::new();
    for m in &mechanisms {
        *count_map.entry(m.package_id.clone()).or_insert(0) += 1;
    }
    for p in &mut packages {
        p.mechanism_count = *count_map.get(&p.package_id).unwrap_or(&0);
    }

    packages.sort_by(|a, b| a.name.cmp(&b.name));
    mechanisms.sort_by(|a, b| a.title.cmp(&b.title));

    let catalog = MechanismCatalog {
        packages,
        mechanisms,
    };

    if let Ok(mut guard) = cache().lock() {
        *guard = Some(catalog.clone());
    }

    Ok(catalog)
}

pub fn get_mechanism_by_id(
    config: &AppConfig,
    id: &str,
) -> Result<Option<CatalogMechanism>, AppError> {
    let catalog = get_catalog(config)?;
    Ok(catalog
        .mechanisms
        .into_iter()
        .find(|m| m.id.eq_ignore_ascii_case(id)))
}

pub fn search_mechanisms(
    config: &AppConfig,
    query: &str,
    limit: usize,
) -> Result<Vec<CatalogMechanism>, AppError> {
    let catalog = get_catalog(config)?;
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() {
        return Ok(catalog.mechanisms.into_iter().take(limit).collect());
    }

    let mut scored = catalog
        .mechanisms
        .into_iter()
        .map(|m| {
            let mut score = 0.0f32;
            let hay = format!(
                "{} {} {} {} {}",
                m.id,
                m.title,
                m.summary,
                m.category,
                m.keywords.join(" ")
            )
            .to_ascii_lowercase();

            for token in q.split_whitespace() {
                if token.len() < 2 {
                    continue;
                }
                if hay.contains(token) {
                    score += 1.0;
                    if m.title.to_ascii_lowercase().contains(token) {
                        score += 0.6;
                    }
                    if m.id.to_ascii_lowercase().contains(token) {
                        score += 0.8;
                    }
                }
            }

            (m, score)
        })
        .filter(|(_, score)| *score > 0.0)
        .collect::<Vec<_>>();

    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    Ok(scored.into_iter().take(limit).map(|(m, _)| m).collect())
}
