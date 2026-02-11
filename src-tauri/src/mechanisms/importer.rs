use std::path::PathBuf;

use reqwest::Client;
use sha2::{Digest, Sha256};

use crate::config::AppConfig;
use crate::error::AppError;

use super::catalog;
use super::license::is_allowed_license;
use super::schema::{
    ManifestMechanismEntry, MechanismImportReport, MechanismPackageManifest, MechanismRecord,
};

fn imported_root() -> Result<PathBuf, AppError> {
    let base = dirs::config_dir()
        .ok_or_else(|| AppError::ConfigError("Cannot resolve config directory".to_string()))?;
    Ok(base
        .join("cadai-studio")
        .join("mechanisms")
        .join("imported"))
}

fn normalize_hex(s: &str) -> String {
    s.trim().to_ascii_lowercase()
}

fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

fn resolve_child_url(manifest_url: &str, child: &str) -> String {
    let child = child.trim();
    if child.starts_with("http://") || child.starts_with("https://") {
        return child.to_string();
    }

    let mut base = manifest_url.trim().to_string();
    if let Some(idx) = base.rfind('/') {
        base.truncate(idx + 1);
    } else {
        base.push('/');
    }
    format!("{}{}", base, child.trim_start_matches('/'))
}

async fn fetch_text(client: &Client, url: &str) -> Result<String, AppError> {
    let res = client
        .get(url)
        .send()
        .await
        .map_err(|e| AppError::ConfigError(format!("Download failed: {}", e)))?;
    if !res.status().is_success() {
        return Err(AppError::ConfigError(format!(
            "Download failed ({}): {}",
            res.status(),
            url
        )));
    }
    res.text()
        .await
        .map_err(|e| AppError::ConfigError(format!("Response read failed: {}", e)))
}

fn validate_record(record: &MechanismRecord) -> Result<(), AppError> {
    if record.id.trim().is_empty() {
        return Err(AppError::ConfigError(
            "Mechanism id cannot be empty".to_string(),
        ));
    }
    if record.title.trim().is_empty() {
        return Err(AppError::ConfigError(format!(
            "Mechanism '{}' must have a title",
            record.id
        )));
    }
    if record.prompt_block.trim().is_empty() {
        return Err(AppError::ConfigError(format!(
            "Mechanism '{}' must have prompt_block",
            record.id
        )));
    }
    Ok(())
}

fn validate_package_id(package_id: &str) -> Result<(), AppError> {
    let re = regex::Regex::new(r"^[a-zA-Z0-9._-]{2,64}$")
        .map_err(|e| AppError::ConfigError(format!("regex init failed: {}", e)))?;
    if !re.is_match(package_id) {
        return Err(AppError::ConfigError(
            "Invalid package_id (allowed: a-z A-Z 0-9 . _ -)".to_string(),
        ));
    }
    Ok(())
}

pub async fn install_pack_from_url(
    config: &AppConfig,
    manifest_url: &str,
) -> Result<MechanismImportReport, AppError> {
    if !config.mechanism_import_enabled {
        return Err(AppError::ConfigError(
            "Mechanism import is disabled by settings".to_string(),
        ));
    }

    let client = Client::new();
    let manifest_raw = fetch_text(&client, manifest_url).await?;
    let manifest: MechanismPackageManifest = serde_json::from_str(&manifest_raw).map_err(|e| {
        AppError::ConfigError(format!(
            "Invalid mechanism manifest at {}: {}",
            manifest_url, e
        ))
    })?;

    if !is_allowed_license(&manifest.license, config) {
        return Err(AppError::ConfigError(format!(
            "Package license '{}' is not in allowed SPDX list",
            manifest.license
        )));
    }
    validate_package_id(&manifest.package_id)?;

    let mut inline_records = Vec::<MechanismRecord>::new();
    for entry in &manifest.mechanisms {
        let record = match entry {
            ManifestMechanismEntry::Inline(record) => record.clone(),
            ManifestMechanismEntry::FileRef {
                file,
                checksum_sha256,
            } => {
                let file_url = resolve_child_url(manifest_url, file);
                let body = fetch_text(&client, &file_url).await?;
                if let Some(expected) = checksum_sha256 {
                    let got = sha256_hex(body.as_bytes());
                    if normalize_hex(expected) != got {
                        return Err(AppError::ConfigError(format!(
                            "Checksum mismatch for {}",
                            file_url
                        )));
                    }
                }
                serde_json::from_str::<MechanismRecord>(&body).map_err(|e| {
                    AppError::ConfigError(format!(
                        "Invalid mechanism record at {}: {}",
                        file_url, e
                    ))
                })?
            }
        };

        let license = record.license.as_deref().unwrap_or(&manifest.license);
        if !is_allowed_license(license, config) {
            return Err(AppError::ConfigError(format!(
                "Mechanism '{}' license '{}' is not allowed",
                record.id, license
            )));
        }

        validate_record(&record)?;
        inline_records.push(record);
    }

    let save_manifest = MechanismPackageManifest {
        package_id: manifest.package_id.clone(),
        name: manifest.name.clone(),
        version: manifest.version.clone(),
        license: manifest.license.clone(),
        source: Some(manifest_url.to_string()),
        homepage: manifest.homepage.clone(),
        mechanisms: inline_records
            .iter()
            .cloned()
            .map(ManifestMechanismEntry::Inline)
            .collect(),
    };

    let root = imported_root()?;
    std::fs::create_dir_all(&root)?;
    let target_dir = root.join(&manifest.package_id);
    std::fs::create_dir_all(&target_dir)?;

    let bytes = serde_json::to_vec_pretty(&save_manifest)?;
    std::fs::write(target_dir.join("manifest.json"), bytes)?;

    catalog::invalidate_cache();

    Ok(MechanismImportReport {
        package_id: manifest.package_id,
        package_name: manifest.name,
        installed_count: inline_records.len(),
        source_url: manifest_url.to_string(),
    })
}

pub fn remove_imported_pack(package_id: &str) -> Result<bool, AppError> {
    validate_package_id(package_id)?;
    let dir = imported_root()?.join(package_id);
    if dir.exists() {
        std::fs::remove_dir_all(dir)?;
        catalog::invalidate_cache();
        return Ok(true);
    }
    Ok(false)
}
