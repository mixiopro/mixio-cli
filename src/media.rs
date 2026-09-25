//! CLI-native local media commands.
//!
//! These commands intentionally do not go through the hosted MCP endpoint:
//! the endpoint cannot read a path on the caller's filesystem. They mirror the
//! local `upload_file`/`get_public_url` tools from `@mixio-pro/mcp` while
//! presenting the CLI's noun-first interface (`mixio file upload` / `url`).

use anyhow::{bail, Context, Result};
use clap::{builder::PossibleValuesParser, Arg, ArgAction, ArgMatches, Command};
use reqwest::{
    multipart::{Form, Part},
    Client, Url,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::profile::Profile;

const DEFAULT_INFERENCE_FILES_URL: &str = "https://inference.mixio.pro/files";
const MAX_PROXY_UPLOAD_BYTES: u64 = 5 * 1024 * 1024;
const DEFAULT_UPLOAD_TIMEOUT_SECS: u64 = 600;

pub const MEDIA_CATEGORIES: &[&str] = &[
    "source",
    "generated_frame",
    "generated_video",
    "reference",
    "voiceover",
    "music",
    "sfx",
    "final",
];

#[derive(Debug, Clone, Default)]
pub struct MediaOptions {
    project_id: Option<String>,
    organization_id: Option<String>,
    alt: Option<String>,
    category: Option<String>,
    force: bool,
}

#[derive(Debug, Clone)]
struct Fingerprint {
    path: PathBuf,
    sha256: String,
    size: u64,
    mtime_ns: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedEntry {
    pub path: String,
    pub sha256: String,
    pub size: u64,
    pub mtime_ns: String,
    pub media_id: Option<String>,
    pub url: Option<String>,
    pub absolute_url: Option<String>,
    pub filename: Option<String>,
    pub organization_id: Option<String>,
    pub project_id: Option<String>,
    pub uploaded_at: Option<String>,
    pub public_url: Option<String>,
    #[serde(default)]
    pub extra: BTreeMap<String, Value>,
}

impl CachedEntry {
    fn public_url(&self) -> Option<&str> {
        self.absolute_url
            .as_deref()
            .or(self.url.as_deref())
            .or(self.public_url.as_deref())
    }

    fn matches_scope(&self, project_id: Option<&str>, organization_id: Option<&str>) -> bool {
        project_id.is_none_or(|id| self.project_id.as_deref() == Some(id))
            && organization_id.is_none_or(|id| self.organization_id.as_deref() == Some(id))
    }
}

#[derive(Debug, Deserialize)]
struct InferenceFile {
    id: String,
    filename: String,
    #[serde(rename = "mimeType")]
    mime_type: String,
    size: Option<u64>,
    status: String,
    url: String,
}

#[derive(Debug, Deserialize)]
struct UploadReservation {
    #[serde(rename = "fileId")]
    file_id: String,
    #[serde(rename = "uploadUrl")]
    upload_url: String,
    fields: BTreeMap<String, String>,
}

pub struct MediaClient {
    profile_name: String,
    api_key: String,
    studio_base_url: String,
    inference_files_url: String,
    http: Client,
}

impl MediaClient {
    pub fn new(profile: &Profile) -> Result<Self> {
        let upload_timeout_secs = std::env::var("MIXIO_FASTMCP_UPLOAD_TIMEOUT_MS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|value| *value > 0)
            .map(|value| value.div_ceil(1000))
            .unwrap_or(DEFAULT_UPLOAD_TIMEOUT_SECS);
        let timeout = std::time::Duration::from_secs(upload_timeout_secs);

        Ok(Self {
            profile_name: profile.name.clone(),
            api_key: profile.api_key.clone(),
            studio_base_url: studio_base_url(&profile.base_url)?,
            inference_files_url: std::env::var("MIXIO_INFERENCE_FILES_URL")
                .unwrap_or_else(|_| DEFAULT_INFERENCE_FILES_URL.to_string())
                .trim_end_matches('/')
                .to_string(),
            http: Client::builder().timeout(timeout).build()?,
        })
    }

    pub async fn upload(&self, path: &str, options: MediaOptions) -> Result<UploadOutput> {
        let fingerprint = fingerprint(path)?;
        let mut cache = self.load_cache()?;
        if !options.force {
            if let Some(entry) = find_cached(&cache, &fingerprint, &options) {
                return Ok(UploadOutput {
                    ok: true,
                    entry: with_public_url(entry),
                });
            }
        }

        let inference = self.upload_to_inference(&fingerprint).await?;
        let inference_id = inference.id.clone();
        let media = self
            .associate_with_studio(&fingerprint, &inference, &options)
            .await?;
        let entry = CachedEntry {
            path: fingerprint.path.to_string_lossy().into_owned(),
            sha256: fingerprint.sha256,
            size: fingerprint.size,
            mtime_ns: fingerprint.mtime_ns,
            media_id: media.media_id.or(Some(inference_id.clone())),
            url: media.url.or_else(|| Some(inference.url.clone())),
            absolute_url: media.absolute_url,
            filename: media.filename.or(Some(inference.filename)),
            organization_id: media.organization_id.or(options.organization_id),
            project_id: media.project_id.or(options.project_id),
            uploaded_at: Some(epoch_seconds()),
            public_url: None,
            extra: BTreeMap::from([
                ("storage".to_string(), json!("inference")),
                ("inferenceFileId".to_string(), json!(inference_id)),
            ]),
        };
        let output_entry = with_public_url(entry.clone());
        cache.retain(|cached| cached.path != entry.path);
        cache.push(entry);
        self.save_cache(&cache)?;
        Ok(UploadOutput {
            ok: true,
            entry: output_entry,
        })
    }

    pub async fn public_url(
        &self,
        path: &str,
        mut options: MediaOptions,
        upload: bool,
    ) -> Result<UrlOutput> {
        let fingerprint = fingerprint(path)?;
        let cache = self.load_cache()?;
        if let Some(entry) = find_cached(&cache, &fingerprint, &options) {
            return Ok(UrlOutput {
                ok: true,
                found: true,
                source: Some("cache".to_string()),
                public_url: entry.public_url().map(str::to_string),
                entry: Some(with_public_url(entry)),
            });
        }

        if !upload {
            return Ok(UrlOutput {
                ok: true,
                found: false,
                source: None,
                public_url: None,
                entry: None,
            });
        }

        options.force = false;
        let uploaded = self.upload(path, options).await?;
        Ok(UrlOutput {
            ok: true,
            found: true,
            source: Some("uploaded".to_string()),
            public_url: uploaded.entry.public_url().map(str::to_string),
            entry: Some(uploaded.entry),
        })
    }

    async fn upload_to_inference(&self, fingerprint: &Fingerprint) -> Result<InferenceFile> {
        if fingerprint.size <= MAX_PROXY_UPLOAD_BYTES {
            let form = Form::new().part("file", Part::file(&fingerprint.path).await?);
            let response = self
                .http
                .post(&self.inference_files_url)
                .header("x-api-key", &self.api_key)
                .multipart(form)
                .send()
                .await
                .context("Inference Files upload request failed")?;
            return parse_json_response(response, "Inference upload").await;
        }

        let reservation_response = self
            .http
            .post(format!("{}/upload-url", self.inference_files_url))
            .header("x-api-key", &self.api_key)
            .json(&json!({
                "filename": fingerprint.path.file_name().and_then(|name| name.to_str()).unwrap_or("upload"),
                "mimeType": mime_type(&fingerprint.path),
            }))
            .send()
            .await
            .context("Inference upload reservation request failed")?;
        let reservation: UploadReservation =
            parse_json_response(reservation_response, "Inference upload reservation").await?;
        validate_https_url(&reservation.upload_url, "Inference upload URL")?;

        let mut form = Form::new();
        for (key, value) in reservation.fields {
            form = form.text(key, value);
        }
        form = form.part("file", Part::file(&fingerprint.path).await?);
        let response = self
            .http
            .post(&reservation.upload_url)
            .multipart(form)
            .send()
            .await
            .context("presigned Inference upload request failed")?;
        ensure_success(response, "presigned Inference upload").await?;

        let confirmation_response = self
            .http
            .post(format!("{}/upload-url/confirm", self.inference_files_url))
            .header("x-api-key", &self.api_key)
            .json(&json!({ "fileId": reservation.file_id }))
            .send()
            .await
            .context("Inference upload confirmation request failed")?;
        parse_json_response(confirmation_response, "Inference upload confirmation").await
    }

    async fn associate_with_studio(
        &self,
        fingerprint: &Fingerprint,
        inference: &InferenceFile,
        options: &MediaOptions,
    ) -> Result<MediaAssociation> {
        validate_inference_file(inference)?;
        let mut body = json!({
            "inferenceFileId": inference.id,
            "url": inference.url,
            "filename": inference.filename,
            "mimeType": inference.mime_type,
            "filesize": inference.size.unwrap_or(fingerprint.size),
        });
        let object = body
            .as_object_mut()
            .expect("media association body is an object");
        if let Some(value) = &options.organization_id {
            object.insert("organizationId".into(), json!(value));
        }
        if let Some(value) = &options.project_id {
            object.insert("projectId".into(), json!(value));
        }
        if let Some(value) = &options.alt {
            object.insert("alt".into(), json!(value));
        }
        if let Some(value) = &options.category {
            object.insert("category".into(), json!(value));
        }

        let response = self
            .http
            .post(format!("{}/api/media/inference", self.studio_base_url))
            .header("x-api-key", &self.api_key)
            .header("authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await
            .context("Studio media association request failed")?;
        let payload: Value = parse_json_response(response, "Studio media association").await?;
        MediaAssociation::from_payload(payload, &self.studio_base_url)
    }

    fn cache_path(&self) -> Result<PathBuf> {
        let base = dirs::cache_dir().context("could not determine cache directory")?;
        Ok(base
            .join("mixio")
            .join(format!("media-{}.json", self.profile_name)))
    }

    fn load_cache(&self) -> Result<Vec<CachedEntry>> {
        let path = self.cache_path()?;
        match fs::read(path) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes).context("invalid media cache")?),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(error) => Err(error).context("reading media cache"),
        }
    }

    fn save_cache(&self, entries: &[CachedEntry]) -> Result<()> {
        let path = self.cache_path()?;
        fs::create_dir_all(path.parent().context("media cache has no parent")?)?;
        fs::write(path, serde_json::to_vec_pretty(entries)?)?;
        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub struct UploadOutput {
    pub ok: bool,
    pub entry: CachedEntry,
}

#[derive(Debug, Serialize)]
pub struct UrlOutput {
    pub ok: bool,
    pub found: bool,
    pub source: Option<String>,
    #[serde(rename = "public_url")]
    pub public_url: Option<String>,
    pub entry: Option<CachedEntry>,
}

#[derive(Debug, Default)]
struct MediaAssociation {
    media_id: Option<String>,
    url: Option<String>,
    absolute_url: Option<String>,
    filename: Option<String>,
    organization_id: Option<String>,
    project_id: Option<String>,
}

impl MediaAssociation {
    fn from_payload(payload: Value, studio_base_url: &str) -> Result<Self> {
        let doc = payload.get("doc").unwrap_or(&payload);
        let media_id = string_field(doc, "id");
        let url = string_field(doc, "url");
        let absolute_url = string_field(doc, "absoluteUrl").or_else(|| {
            url.as_deref().map(|value| {
                if value.starts_with("http://") || value.starts_with("https://") {
                    value.to_string()
                } else if value.starts_with('/') {
                    format!("{}{}", studio_base_url, value)
                } else {
                    format!("{}/{}", studio_base_url, value)
                }
            })
        });
        if media_id.is_none() || absolute_url.is_none() {
            bail!("Studio media association response missing id/url fields")
        }
        Ok(Self {
            media_id,
            url,
            absolute_url,
            filename: string_field(doc, "filename"),
            organization_id: string_field(doc, "organizationId"),
            project_id: string_field(doc, "projectId"),
        })
    }
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn with_public_url(mut entry: CachedEntry) -> CachedEntry {
    entry.public_url = entry.public_url().map(str::to_string);
    entry
}

fn find_cached(
    entries: &[CachedEntry],
    fingerprint: &Fingerprint,
    options: &MediaOptions,
) -> Option<CachedEntry> {
    entries
        .iter()
        .find(|entry| {
            entry.path == fingerprint.path.to_string_lossy()
                && entry.sha256 == fingerprint.sha256
                && entry.public_url().is_some()
                && entry.matches_scope(
                    options.project_id.as_deref(),
                    options.organization_id.as_deref(),
                )
        })
        .cloned()
}

fn fingerprint(raw_path: &str) -> Result<Fingerprint> {
    let path = fs::canonicalize(raw_path)
        .with_context(|| format!("could not resolve local file `{raw_path}`"))?;
    let metadata = fs::metadata(&path)
        .with_context(|| format!("could not inspect local file `{}`", path.display()))?;
    if !metadata.is_file() {
        bail!("path is not a regular file: {}", path.display())
    }

    let mut file = File::open(&path)
        .with_context(|| format!("could not read local file `{}`", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let modified = metadata.modified().unwrap_or(UNIX_EPOCH);
    let mtime_ns = modified
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .to_string();
    Ok(Fingerprint {
        path,
        sha256: format!("{:x}", hasher.finalize()),
        size: metadata.len(),
        mtime_ns,
    })
}

fn mime_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "m4a" => "audio/mp4",
        "ogg" => "audio/ogg",
        "json" => "application/json",
        "txt" => "text/plain",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}

fn validate_inference_file(file: &InferenceFile) -> Result<()> {
    if file.id.trim().is_empty()
        || file.filename.trim().is_empty()
        || file.mime_type.trim().is_empty()
        || file.status != "active"
    {
        bail!("Inference Files returned an inactive or incomplete file")
    }
    validate_https_url(&file.url, "Inference public URL")
}

fn validate_https_url(raw: &str, label: &str) -> Result<()> {
    let url = Url::parse(raw).with_context(|| format!("{label} is invalid"))?;
    if url.scheme() != "https" {
        bail!("{label} must use HTTPS")
    }
    Ok(())
}

async fn parse_json_response<T: for<'de> Deserialize<'de>>(
    response: reqwest::Response,
    label: &str,
) -> Result<T> {
    let status = response.status();
    let body = response.text().await.context("reading response body")?;
    if !status.is_success() {
        bail!(
            "{label} failed ({status}): {}",
            body.chars().take(500).collect::<String>()
        )
    }
    serde_json::from_str(&body).with_context(|| format!("{label} returned invalid JSON"))
}

async fn ensure_success(response: reqwest::Response, label: &str) -> Result<()> {
    let status = response.status();
    if status.is_success() {
        return Ok(());
    }
    let body = response.text().await.unwrap_or_default();
    bail!(
        "{label} failed ({status}): {}",
        body.chars().take(500).collect::<String>()
    )
}

fn epoch_seconds() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}

pub fn studio_base_url(raw: &str) -> Result<String> {
    let mut url = Url::parse(raw).with_context(|| format!("invalid MCP endpoint `{raw}`"))?;
    let path = url.path().trim_end_matches('/').to_string();
    if let Some(prefix) = path.strip_suffix("/api/mcp") {
        let root = if prefix.is_empty() { "/" } else { prefix };
        url.set_path(root);
    } else {
        url.set_path("/");
    }
    url.set_query(None);
    url.set_fragment(None);
    Ok(url.to_string().trim_end_matches('/').to_string())
}

fn option_string(matches: &ArgMatches, name: &str) -> Option<String> {
    matches
        .get_one::<String>(name)
        .cloned()
        .filter(|value| !value.is_empty())
}

fn media_options(matches: &ArgMatches) -> MediaOptions {
    MediaOptions {
        project_id: option_string(matches, "project-id"),
        organization_id: option_string(matches, "organization-id"),
        alt: option_string(matches, "alt"),
        category: option_string(matches, "category"),
        force: matches.get_flag("force"),
    }
}

fn path_argument(matches: &ArgMatches) -> Result<&str> {
    matches
        .get_one::<String>("path")
        .map(String::as_str)
        .context("PATH is required")
}

fn common_file_args(command: Command) -> Command {
    command
        .arg(
            Arg::new("path")
                .value_name("PATH")
                .required(true)
                .help("Local file path"),
        )
        .arg(
            Arg::new("project-id")
                .long("project-id")
                .help("Mixio project to scope the media to"),
        )
        .arg(
            Arg::new("organization-id")
                .long("organization-id")
                .help("Mixio organization to scope the media to"),
        )
        .arg(
            Arg::new("alt")
                .long("alt")
                .help("Alt text/description for the media"),
        )
        .arg(
            Arg::new("category")
                .long("category")
                .value_parser(PossibleValuesParser::new(
                    MEDIA_CATEGORIES
                        .iter()
                        .map(|value| (*value).to_string())
                        .collect::<Vec<_>>(),
                )),
        )
}

pub fn build_file_command() -> Command {
    Command::new("file")
        .about("Upload and resolve local files")
        .subcommand_required(true)
        .subcommand(common_file_args(
            Command::new("upload")
                .about("Upload a local file and associate it with Mixio Studio")
                .arg(
                    Arg::new("force")
                        .long("force")
                        .action(ArgAction::SetTrue)
                        .help("Upload even when a matching cache entry exists"),
                ),
        ))
        .subcommand(common_file_args(
            Command::new("url")
                .about("Get a cached/public URL for a local file")
                .arg(
                    Arg::new("no-upload")
                        .long("no-upload")
                        .action(ArgAction::SetTrue)
                        .help("Only consult the local cache"),
                ),
        ))
}

pub async fn handle_file(sub: &ArgMatches) -> Result<()> {
    let profile = crate::profile::active()?;
    let client = MediaClient::new(&profile)?;
    let (verb, matches) = sub
        .subcommand()
        .context("specify a file verb — see `mixio file --help`")?;
    let path = path_argument(matches)?;
    match verb {
        "upload" => println!(
            "{}",
            serde_json::to_string_pretty(&client.upload(path, media_options(matches)).await?)?
        ),
        "url" => println!(
            "{}",
            serde_json::to_string_pretty(
                &client
                    .public_url(path, media_options(matches), !matches.get_flag("no-upload"))
                    .await?,
            )?
        ),
        _ => bail!("unknown file verb `{verb}` — see `mixio file --help`"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn normalizes_default_mcp_endpoint_to_studio_root() {
        assert_eq!(
            studio_base_url("https://studio.mixio.pro/api/mcp").unwrap(),
            "https://studio.mixio.pro"
        );
        assert_eq!(
            studio_base_url("https://studio.mixio.pro/api/mcp/").unwrap(),
            "https://studio.mixio.pro"
        );
    }

    #[test]
    fn file_command_uses_cli_native_noun_verb_shape() {
        let matches = build_file_command()
            .try_get_matches_from([
                "file",
                "upload",
                "./reference.png",
                "--category",
                "reference",
            ])
            .expect("file upload syntax should parse");

        let (verb, args) = matches.subcommand().expect("file verb");
        assert_eq!(verb, "upload");
        assert_eq!(
            args.get_one::<String>("path").map(String::as_str),
            Some("./reference.png")
        );
    }

    #[test]
    fn cache_scope_requires_matching_project_and_organization() {
        let entry = CachedEntry {
            project_id: Some("project-1".into()),
            organization_id: Some("org-1".into()),
            ..Default::default()
        };

        assert!(entry.matches_scope(Some("project-1"), Some("org-1")));
        assert!(!entry.matches_scope(Some("project-2"), Some("org-1")));
        assert!(!entry.matches_scope(Some("project-1"), Some("org-2")));
    }

    #[test]
    fn media_entry_serializes_public_url_without_credentials() {
        let entry = CachedEntry {
            url: Some("https://cdn.example/file.png".into()),
            ..Default::default()
        };
        let value = serde_json::to_value(with_public_url(entry)).unwrap();
        assert_eq!(value["publicUrl"], json!("https://cdn.example/file.png"));
        assert!(value.get("apiKey").is_none());
    }

    #[test]
    fn url_command_can_disable_uploads() {
        let matches = build_file_command()
            .try_get_matches_from(["file", "url", "./reference.png", "--no-upload"])
            .expect("file url syntax should parse");
        let (_, args) = matches.subcommand().expect("file verb");
        assert!(args.get_flag("no-upload"));
    }
}
