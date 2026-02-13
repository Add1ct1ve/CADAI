use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MechanismParameter {
    pub name: String,
    #[serde(default)]
    pub default_value: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub unit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MechanismRecord {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub summary: String,
    pub category: String,
    #[serde(default)]
    pub keywords: Vec<String>,
    pub prompt_block: String,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub source_url: Option<String>,
    #[serde(default)]
    pub preview_url: Option<String>,
    #[serde(default)]
    pub parameters: Vec<MechanismParameter>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ManifestMechanismEntry {
    Inline(MechanismRecord),
    FileRef {
        file: String,
        #[serde(default)]
        checksum_sha256: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MechanismPackageManifest {
    pub package_id: String,
    pub name: String,
    pub version: String,
    pub license: String,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub mechanisms: Vec<ManifestMechanismEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CatalogMechanism {
    pub package_id: String,
    pub package_name: String,
    pub package_version: String,
    pub id: String,
    pub title: String,
    pub summary: String,
    pub category: String,
    pub keywords: Vec<String>,
    pub prompt_block: String,
    pub license: Option<String>,
    pub source_url: Option<String>,
    pub preview_url: Option<String>,
    pub parameters: Vec<MechanismParameter>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CatalogPackage {
    pub package_id: String,
    pub name: String,
    pub version: String,
    pub license: String,
    pub source: Option<String>,
    pub homepage: Option<String>,
    pub mechanism_count: usize,
    pub is_imported: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct MechanismCatalog {
    pub packages: Vec<CatalogPackage>,
    pub mechanisms: Vec<CatalogMechanism>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MechanismImportReport {
    pub package_id: String,
    pub package_name: String,
    pub installed_count: usize,
    pub source_url: String,
}
