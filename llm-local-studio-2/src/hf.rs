#![allow(dead_code)]

use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr;

const DEFAULT_BASE_URL: &str = "https://huggingface.co";
const PREFERRED_QUANTS: &[&str] = &["Q4_K_M", "Q4_K_S", "Q5_K_M", "Q8_0", "Q4_0"];
const USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HuggingFaceModelRef {
    pub owner: String,
    pub repo: String,
}

impl HuggingFaceModelRef {
    pub fn id(&self) -> String {
        format!("{}/{}", self.owner, self.repo)
    }
}

impl FromStr for HuggingFaceModelRef {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (owner, repo) = value
            .split_once('/')
            .with_context(|| format!("expected repository ID in owner/repo form, got {value:?}"))?;
        if owner.is_empty() || repo.is_empty() || repo.contains('/') {
            anyhow::bail!("expected repository ID in owner/repo form, got {value:?}");
        }

        Ok(Self {
            owner: owner.to_string(),
            repo: repo.to_string(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct HuggingFaceClient {
    base_url: String,
    api_url: String,
    http: reqwest::blocking::Client,
}

impl Default for HuggingFaceClient {
    fn default() -> Self {
        Self::new(DEFAULT_BASE_URL)
    }
}

impl HuggingFaceClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        let base_url = base_url.into().trim_end_matches('/').to_string();
        let api_url = format!("{base_url}/api");

        Self {
            base_url,
            api_url,
            http: reqwest::blocking::Client::new(),
        }
    }

    pub fn search_url(&self, query: &str) -> String {
        let encoded = urlencoding::encode(query);
        format!(
            "{}/models?search={encoded}&library=gguf&sort=downloads",
            self.base_url
        )
    }

    pub fn search_models(&self, query: &str, limit: usize) -> Result<Vec<HuggingFaceModel>> {
        let limit = limit.clamp(1, 100);
        let encoded = urlencoding::encode(query);
        let url = format!(
            "{}/models?search={encoded}&filter=gguf&sort=downloads&direction=-1&limit={limit}",
            self.api_url
        );

        self.get_json(&url, || {
            format!("failed to search Hugging Face models for {query:?}")
        })
    }

    pub fn resolve_model(&self, name: &str) -> Result<HuggingFaceModelRef> {
        if let Ok(model) = name.parse() {
            return Ok(model);
        }

        let mut models = self.search_models(name, 1)?;
        let model = models
            .pop()
            .with_context(|| format!("no GGUF models found for {name:?}"))?;

        model
            .id
            .parse()
            .with_context(|| format!("Hugging Face returned invalid model ID {}", model.id))
    }

    pub fn list_gguf_files(&self, model: &HuggingFaceModelRef) -> Result<Vec<HuggingFaceFile>> {
        let url = format!("{}/models/{}", self.api_url, model.id());
        let details = self.get_json::<HuggingFaceModelDetails>(&url, || {
            format!(
                "failed to fetch Hugging Face model metadata for {}",
                model.id()
            )
        })?;

        let mut files: Vec<HuggingFaceFile> = details
            .siblings
            .into_iter()
            .filter(|file| file.rfilename.to_lowercase().ends_with(".gguf"))
            .collect();
        files.sort_by(|a, b| a.rfilename.cmp(&b.rfilename));
        Ok(files)
    }

    pub fn choose_gguf_file<'a>(
        &self,
        files: &'a [HuggingFaceFile],
        requested: Option<&str>,
    ) -> Result<&'a HuggingFaceFile> {
        if let Some(requested) = requested {
            return files
                .iter()
                .find(|file| file.rfilename == requested)
                .with_context(|| {
                    format!("GGUF file {requested:?} was not found in the repository")
                });
        }

        for preferred in PREFERRED_QUANTS {
            if let Some(file) = files.iter().find(|file| file.rfilename.contains(preferred)) {
                return Ok(file);
            }
        }

        files
            .first()
            .context("repository does not contain GGUF files")
    }

    pub fn gguf_download_url(&self, model: &HuggingFaceModelRef, filename: &str) -> String {
        let filename = filename.trim_start_matches('/');
        format!(
            "{}/{}/{}/resolve/main/{}",
            self.base_url, model.owner, model.repo, filename
        )
    }

    pub fn plan_download(&self, request: &DownloadRequest) -> Result<DownloadPlan> {
        let model = self.resolve_model(&request.name)?;
        let files = self.list_gguf_files(&model)?;
        let file = self
            .choose_gguf_file(&files, request.filename.as_deref())?
            .clone();
        let url = self.gguf_download_url(&model, &file.rfilename);
        let destination = local_model_path(&request.models_dir, &model, &file.rfilename)?;

        Ok(DownloadPlan {
            model,
            file,
            url,
            destination,
        })
    }

    pub fn download_gguf(&self, plan: &DownloadPlan, force: bool) -> Result<DownloadedModel> {
        if plan.destination.exists() && !force {
            anyhow::bail!(
                "destination already exists: {} (use --force to overwrite)",
                plan.destination.display()
            );
        }

        let parent = plan
            .destination
            .parent()
            .with_context(|| format!("invalid destination path {}", plan.destination.display()))?;
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create model directory {}", parent.display()))?;

        let mut response = self
            .http
            .get(&plan.url)
            .header(reqwest::header::USER_AGENT, USER_AGENT)
            .send()
            .with_context(|| format!("failed to start download from {}", plan.url))?
            .error_for_status()
            .with_context(|| format!("Hugging Face download request failed: {}", plan.url))?;

        let expected_bytes = response.content_length();
        let temp_path = plan.destination.with_extension("gguf.part");
        let mut output = fs::File::create(&temp_path)
            .with_context(|| format!("failed to create temporary file {}", temp_path.display()))?;
        let bytes_written = io::copy(&mut response, &mut output)
            .with_context(|| format!("failed to write download to {}", temp_path.display()))?;
        drop(output);

        fs::rename(&temp_path, &plan.destination).with_context(|| {
            format!(
                "failed to move temporary file {} to {}",
                temp_path.display(),
                plan.destination.display()
            )
        })?;

        Ok(DownloadedModel {
            model_id: plan.model.id(),
            filename: plan.file.rfilename.clone(),
            path: plan.destination.clone(),
            bytes_written,
            expected_bytes,
        })
    }

    fn get_json<T>(&self, url: &str, context: impl FnOnce() -> String) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        self.http
            .get(url)
            .header(reqwest::header::USER_AGENT, USER_AGENT)
            .send()
            .with_context(context)?
            .error_for_status()
            .with_context(|| format!("Hugging Face request failed: {url}"))?
            .json::<T>()
            .with_context(|| format!("failed to parse Hugging Face response from {url}"))
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HuggingFaceModel {
    pub id: String,
    #[serde(default)]
    pub downloads: Option<u64>,
    #[serde(default)]
    pub likes: Option<u64>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub last_modified: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct HuggingFaceModelDetails {
    #[serde(default)]
    pub siblings: Vec<HuggingFaceFile>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct HuggingFaceFile {
    pub rfilename: String,
    #[serde(default)]
    pub size: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadedModel {
    pub model_id: String,
    pub filename: String,
    pub path: PathBuf,
    pub bytes_written: u64,
    pub expected_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadRequest {
    pub name: String,
    pub filename: Option<String>,
    pub models_dir: PathBuf,
    pub print_url: bool,
    pub force: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadPlan {
    pub model: HuggingFaceModelRef,
    pub file: HuggingFaceFile,
    pub url: String,
    pub destination: PathBuf,
}

fn local_model_path(
    models_dir: &Path,
    model: &HuggingFaceModelRef,
    filename: &str,
) -> Result<PathBuf> {
    let filename = filename.trim_start_matches('/');
    if filename.is_empty() || filename.contains("..") {
        anyhow::bail!("invalid Hugging Face filename {filename:?}");
    }

    let mut path = models_dir
        .join("huggingface")
        .join(&model.owner)
        .join(&model.repo);
    for component in filename.split('/') {
        if component.is_empty() || component == "." || component == ".." {
            anyhow::bail!("invalid Hugging Face filename {filename:?}");
        }
        path.push(component);
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hugging_face_model_ref() {
        let model: HuggingFaceModelRef = "TheBloke/TinyLlama-1.1B-GGUF".parse().unwrap();

        assert_eq!(model.owner, "TheBloke");
        assert_eq!(model.repo, "TinyLlama-1.1B-GGUF");
        assert_eq!(model.id(), "TheBloke/TinyLlama-1.1B-GGUF");
    }

    #[test]
    fn builds_download_url() {
        let client = HuggingFaceClient::new("https://huggingface.co/");
        let model: HuggingFaceModelRef = "owner/repo".parse().unwrap();

        assert_eq!(
            client.gguf_download_url(&model, "model.gguf"),
            "https://huggingface.co/owner/repo/resolve/main/model.gguf"
        );
    }

    #[test]
    fn builds_search_url_with_gguf_library_filter() {
        let client = HuggingFaceClient::new("https://huggingface.co/");

        assert_eq!(
            client.search_url("tiny llama"),
            "https://huggingface.co/models?search=tiny%20llama&library=gguf&sort=downloads"
        );
    }

    #[test]
    fn chooses_preferred_quant_when_filename_is_not_requested() {
        let client = HuggingFaceClient::default();
        let files = vec![
            HuggingFaceFile {
                rfilename: "model.Q8_0.gguf".to_string(),
                size: None,
            },
            HuggingFaceFile {
                rfilename: "model.Q4_K_M.gguf".to_string(),
                size: None,
            },
        ];

        assert_eq!(
            client.choose_gguf_file(&files, None).unwrap().rfilename,
            "model.Q4_K_M.gguf"
        );
    }

    #[test]
    fn builds_local_hugging_face_model_path() {
        let model: HuggingFaceModelRef = "owner/repo".parse().unwrap();
        let path = local_model_path(Path::new("models"), &model, "nested/model.gguf").unwrap();

        assert_eq!(
            path,
            Path::new("models")
                .join("huggingface")
                .join("owner")
                .join("repo")
                .join("nested")
                .join("model.gguf")
        );
    }
}
