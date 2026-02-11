use crate::config::AppConfig;

pub const DEFAULT_ALLOWED_LICENSES: &[&str] = &[
    "MIT",
    "Apache-2.0",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "CC0-1.0",
];

pub fn normalize_spdx(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

pub fn is_allowed_license(license: &str, config: &AppConfig) -> bool {
    let allowed = if config.allowed_spdx_licenses.is_empty() {
        DEFAULT_ALLOWED_LICENSES
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
    } else {
        config.allowed_spdx_licenses.clone()
    };
    let needle = normalize_spdx(license);
    allowed.iter().any(|l| normalize_spdx(l) == needle)
}
