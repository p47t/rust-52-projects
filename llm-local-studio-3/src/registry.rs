#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelSource {
    Local,
    HuggingFace { repo: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelLoadStatus {
    Available,
    Loaded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelRecord {
    pub id: String,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub source: ModelSource,
    pub status: ModelLoadStatus,
}

#[derive(Debug, Default)]
pub struct ModelRegistry {
    models: Vec<ModelRecord>,
}

impl ModelRegistry {
    pub fn scan_dir(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref();
        let mut models = Vec::new();
        scan_dir_recursive(root, &mut models)
            .with_context(|| format!("failed to scan model directory {}", root.display()))?;
        models.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(Self { models })
    }

    pub fn models(&self) -> &[ModelRecord] {
        &self.models
    }

    pub fn find(&self, query: &str) -> Result<&ModelRecord> {
        let query_lower = query.to_lowercase();

        let mut matches = self.models.iter().filter(|model| {
            model.id.eq_ignore_ascii_case(query)
                || model
                    .path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.eq_ignore_ascii_case(query))
        });

        if let Some(model) = matches.next() {
            return Ok(model);
        }

        let partial_matches = self
            .models
            .iter()
            .filter(|model| {
                model.id.to_lowercase().contains(&query_lower)
                    || model
                        .path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.to_lowercase().contains(&query_lower))
            })
            .collect::<Vec<_>>();

        match partial_matches.as_slice() {
            [model] => Ok(model),
            [] => bail!("no local model matched {query:?}"),
            matches => {
                let ids = matches
                    .iter()
                    .map(|model| model.id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                bail!("model name {query:?} is ambiguous; matches: {ids}")
            }
        }
    }
}

fn scan_dir_recursive(dir: &Path, models: &mut Vec<ModelRecord>) -> Result<()> {
    if !dir.try_exists()? {
        return Ok(());
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = entry.metadata()?;

        if metadata.is_dir() {
            scan_dir_recursive(&path, models)?;
        } else if is_gguf_file(&path) {
            models.push(ModelRecord {
                id: model_id_from_path(&path),
                path,
                size_bytes: metadata.len(),
                source: ModelSource::Local,
                status: ModelLoadStatus::Available,
            });
        }
    }

    Ok(())
}

fn is_gguf_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("gguf"))
}

fn model_id_from_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown-model")
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_gguf_extension_case_insensitively() {
        assert!(is_gguf_file(Path::new("model.gguf")));
        assert!(is_gguf_file(Path::new("model.GGUF")));
        assert!(!is_gguf_file(Path::new("model.bin")));
    }
}
