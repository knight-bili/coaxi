//! The context window OpenCode gives a model: the denominator of the context
//! gauge on a reopened OpenCode session.
//!
//! OpenCode never writes a window into its transcripts. Its live ACP adapter
//! looks one up for every `usage_update` it sends (`findContextLimit(providers,
//! providerID, modelID)`, the `limit.context` of the latest assistant message's
//! model) in the provider list OpenCode assembles from the models.dev catalog
//! and the user's config. Guessing from the model id alone
//! ([`super::infer_context_window_max_tokens`]) knows none of the models
//! OpenCode's own gateways serve (`opencode/big-pickle`, `opencode-go/*`), none
//! of a custom provider's, and nothing of a catalog that gives one id a
//! different window under a different provider. So a session that showed its
//! gauge live lost it once reopened from history.
//!
//! This reads the same sources, in OpenCode's precedence:
//!
//! 1. a `limit.context` declared in the user's config files, layered the way
//!    OpenCode 1.18 layers them (see [`ModelLimitSources::config_files`]);
//! 2. the models.dev catalog OpenCode caches: `$OPENCODE_MODELS_PATH`, else
//!    `$XDG_CACHE_HOME/opencode/models.json`;
//! 3. coaxi's bundled models.dev snapshot, for a model that catalog does not
//!    list. OpenCode itself falls back to the snapshot compiled into its
//!    binary when it has no readable cache, so a machine that never fetched
//!    the catalog still sized its gauge from one.
//!
//! A model none of them names is left to the caller's name guess.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::SystemTime;

use serde_json::Value;

/// A model the way OpenCode addresses it: the provider that served it, and the
/// id that provider knows it by. Kept exactly as the transcript spells both,
/// since they are looked up as keys.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ModelRef {
    pub(crate) provider_id: Option<String>,
    pub(crate) model_id: String,
}

impl ModelRef {
    /// `None` without a model id. A blank provider counts as no provider.
    pub(crate) fn new(provider_id: Option<&str>, model_id: Option<&str>) -> Option<Self> {
        let model_id = model_id.filter(|id| !id.trim().is_empty())?;
        let provider_id = provider_id
            .filter(|id| !id.trim().is_empty())
            .map(str::to_string);
        Some(Self {
            provider_id,
            model_id: model_id.to_string(),
        })
    }
}

/// Where OpenCode keeps what decides a model's window.
///
/// Every source is optional so a test can point the lookup at fixtures, or at
/// nothing at all (the [`Default`]), which leaves the caller's name guess in
/// charge instead of whatever the machine running the suite has installed.
#[derive(Clone, Debug, Default)]
pub(crate) struct ModelLimitSources {
    /// The global config directory, `$XDG_CONFIG_HOME/opencode`.
    pub(crate) config_dir: Option<PathBuf>,
    /// `$OPENCODE_CONFIG`: one more config file, over the global ones.
    pub(crate) config_file: Option<PathBuf>,
    /// `$OPENCODE_CONFIG_DIR`: a directory read the way a `.opencode` one is.
    pub(crate) config_dir_env: Option<PathBuf>,
    /// The home directory, whose own `.opencode` OpenCode also reads.
    pub(crate) home_dir: Option<PathBuf>,
    /// Whether the session directory's own config files count. OpenCode skips
    /// them under `OPENCODE_DISABLE_PROJECT_CONFIG`.
    pub(crate) project_config: bool,
    /// The models.dev catalog OpenCode caches.
    pub(crate) catalog_file: Option<PathBuf>,
    /// Whether coaxi's bundled models.dev snapshot answers last.
    pub(crate) bundled_catalog: bool,
}

impl ModelLimitSources {
    /// The files the OpenCode that coaxi launches reads. Its agent process
    /// inherits coaxi's environment, so the same variables send both to the
    /// same files.
    pub(crate) fn from_env() -> Self {
        let env_path = |key: &str| {
            std::env::var_os(key)
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
        };
        Self {
            config_dir: crate::acp::opencode_plugins::xdg_config_home()
                .map(|dir| dir.join("opencode")),
            config_file: env_path("OPENCODE_CONFIG"),
            config_dir_env: env_path("OPENCODE_CONFIG_DIR"),
            home_dir: dirs::home_dir(),
            project_config: !env_flag("OPENCODE_DISABLE_PROJECT_CONFIG"),
            catalog_file: env_path("OPENCODE_MODELS_PATH").or_else(|| {
                crate::acp::opencode_plugins::xdg_cache_home()
                    .map(|dir| dir.join("opencode").join("models.json"))
            }),
            bundled_catalog: true,
        }
    }

    /// The window OpenCode gives `model` in a session run from `directory`, or
    /// `None` when no source names the model.
    pub(crate) fn context_window(&self, directory: Option<&Path>, model: &ModelRef) -> Option<u64> {
        // OpenCode keys the lookup by provider AND model, and so does this: the
        // catalog gives one id different windows under different providers
        // (`claude-sonnet-4-5` is 200K under `anthropic` but 1M through
        // OpenCode Zen), so a model with no provider is not looked up at all.
        let provider = model.provider_id.as_deref()?;

        // Config wins over the catalog, and the highest layer that declares a
        // window wins among the layers. A layer that mentions the model without
        // a window leaves the question to the ones below it, exactly as
        // OpenCode's deep merge does.
        let mut catalog_id: Option<String> = None;
        for path in self.config_files(directory) {
            let Some(config) = read_config(&path) else {
                continue;
            };
            let Some(entry) = config_model_entry(&config, provider, &model.model_id) else {
                continue;
            };
            let declared = entry
                .get("limit")
                .and_then(|limit| limit.get("context"))
                .and_then(token_count);
            if declared.is_some() {
                return declared;
            }
            if catalog_id.is_none() {
                catalog_id = declared_catalog_id(entry).map(str::to_string);
            }
        }

        let catalog_id = catalog_id.as_deref().unwrap_or(&model.model_id);
        // A model OpenCode's own catalog lists is settled by it, window or no
        // window. The bundled snapshot answers only for a model that catalog
        // does not list: OpenCode reads its compiled-in snapshot whenever its
        // cache is missing or unreadable, and a model the cache has dropped
        // since was listed when the session ran on it (OpenCode refuses a
        // model its provider list lacks), so an older models.dev revision is
        // the closest record left of the window it had then.
        let listed = self
            .catalog_file
            .as_deref()
            .and_then(cached_catalog_index)
            .and_then(|index| catalog_entry(&index, provider, catalog_id));
        match listed {
            Some(window) => window,
            None if self.bundled_catalog => {
                catalog_entry(bundled_catalog_index(), provider, catalog_id).flatten()
            }
            None => None,
        }
    }

    /// Every config file OpenCode 1.18 layers for a session run from
    /// `directory`, HIGHEST precedence first. OpenCode merges them in the
    /// opposite order, each file overriding the ones before it:
    ///
    /// - `config.json`, `opencode.json`, `opencode.jsonc` in the global config
    ///   directory, then `$OPENCODE_CONFIG`;
    /// - the project's `opencode.json` / `opencode.jsonc` from the worktree
    ///   root down to `directory`, the nearest merged last
    ///   (`ConfigPaths.projectFiles`);
    /// - `opencode.json` / `opencode.jsonc` in each `.opencode` directory from
    ///   `directory` up to the worktree root, then `~/.opencode`, then
    ///   `$OPENCODE_CONFIG_DIR` (`ConfigPaths.directories`). That walk lists
    ///   the NEAREST directory first, so an outer `.opencode` overrides an
    ///   inner one; this keeps OpenCode's order rather than correcting it.
    ///   Project config being off drops the project's `.opencode` directories
    ///   along with its files, but not `~/.opencode` or `$OPENCODE_CONFIG_DIR`.
    ///
    /// Sources that are not files on this machine (`$OPENCODE_CONFIG_CONTENT`,
    /// an organisation's account config, managed preferences) are left out.
    fn config_files(&self, directory: Option<&Path>) -> Vec<PathBuf> {
        const PROJECT_FILES: [&str; 2] = ["opencode.json", "opencode.jsonc"];

        let mut merge_order: Vec<PathBuf> = Vec::new();
        if let Some(dir) = &self.config_dir {
            for name in ["config.json", "opencode.json", "opencode.jsonc"] {
                merge_order.push(dir.join(name));
            }
        }
        merge_order.extend(self.config_file.clone());

        let project_dirs = match directory {
            Some(directory) if self.project_config => dirs_up_to_worktree(directory),
            _ => Vec::new(),
        };
        for dir in project_dirs.iter().rev() {
            for name in PROJECT_FILES {
                merge_order.push(dir.join(name));
            }
        }
        // `unique([configDir, ...project .opencode, ~/.opencode,
        // $OPENCODE_CONFIG_DIR])`, each directory keeping its FIRST place (a
        // session run from under the home directory walks past `~/.opencode`
        // on its own); then only the `.opencode` ones and
        // `$OPENCODE_CONFIG_DIR` are read. OpenCode compares these as strings,
        // so they are compared as spelled here too: `Path` equality would call
        // `$OPENCODE_CONFIG_DIR=/repo/.opencode/` the walk's `/repo/.opencode`
        // and drop the later of the two places OpenCode merges it at.
        let env_dir = self.config_dir_env.as_deref().map(Path::as_os_str);
        let mut dirs: Vec<PathBuf> = Vec::new();
        let listed = self
            .config_dir
            .iter()
            .cloned()
            .chain(project_dirs.iter().map(|dir| dir.join(".opencode")))
            .chain(self.home_dir.as_ref().map(|home| home.join(".opencode")))
            .chain(self.config_dir_env.clone());
        for dir in listed {
            if !dirs.iter().any(|kept| kept.as_os_str() == dir.as_os_str()) {
                dirs.push(dir);
            }
        }
        for dir in dirs {
            let read =
                dir.to_string_lossy().ends_with(".opencode") || env_dir == Some(dir.as_os_str());
            if read {
                for name in PROJECT_FILES {
                    merge_order.push(dir.join(name));
                }
            }
        }

        // Files are NOT de-duplicated across layers: OpenCode merges a file
        // once for every place it is listed (`$OPENCODE_CONFIG` naming a
        // project file loads it twice), so the file's values stand where it
        // was merged last, the first of its places this reversed list reaches.
        merge_order.reverse();
        merge_order
    }
}

/// OpenCode's reading of a boolean flag: `true` or `1`, in any case.
fn env_flag(key: &str) -> bool {
    std::env::var(key).is_ok_and(|value| {
        let value = value.to_ascii_lowercase();
        value == "true" || value == "1"
    })
}

/// `directory` and each of its parents up to its worktree root, nearest first.
///
/// The root is the nearest directory holding `.git`, which is where OpenCode's
/// project detection stops. Outside a repository OpenCode's worktree is `/`, so
/// the walk runs to the filesystem root. A relative `directory` has no
/// ancestors worth reading: resolving it would read coaxi's own working
/// directory instead of the session's.
fn dirs_up_to_worktree(directory: &Path) -> Vec<PathBuf> {
    if !directory.is_absolute() {
        return Vec::new();
    }
    let mut dirs = Vec::new();
    for dir in directory.ancestors() {
        dirs.push(dir.to_path_buf());
        if dir.join(".git").exists() {
            break;
        }
    }
    dirs
}

fn read_config(path: &Path) -> Option<Value> {
    parse_jsonc(&crate::scoped_fs::read_to_string(path).ok()?)
}

/// Parse a config file the way OpenCode does (`jsonc-parser`, trailing commas
/// allowed): JSON, plus `//` and `/* */` comments and trailing commas. A new
/// OpenCode install writes `opencode.jsonc`, so comments are to be expected.
fn parse_jsonc(raw: &str) -> Option<Value> {
    let raw = raw.strip_prefix('\u{feff}').unwrap_or(raw);
    serde_json::from_str(raw)
        .ok()
        .or_else(|| serde_json::from_str(&strip_jsonc(raw)).ok())
}

/// `raw` with its comments and trailing commas blanked out, strings untouched.
fn strip_jsonc(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    // Where the last comma outside a string sits in `out`, for as long as
    // nothing but whitespace and comments has followed it. A `}` or `]` that
    // arrives while it is still open makes it a trailing comma.
    let mut open_comma: Option<usize> = None;
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                open_comma = None;
                out.push(c);
                while let Some(c) = chars.next() {
                    out.push(c);
                    match c {
                        '\\' => {
                            if let Some(escaped) = chars.next() {
                                out.push(escaped);
                            }
                        }
                        '"' => break,
                        _ => {}
                    }
                }
            }
            '/' if chars.peek() == Some(&'/') => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut previous = '\0';
                for c in chars.by_ref() {
                    if previous == '*' && c == '/' {
                        break;
                    }
                    previous = c;
                }
                out.push(' ');
            }
            ',' => {
                open_comma = Some(out.len());
                out.push(c);
            }
            '}' | ']' => {
                if let Some(at) = open_comma.take() {
                    out.replace_range(at..at + 1, " ");
                }
                out.push(c);
            }
            c if c.is_whitespace() => out.push(c),
            c => {
                open_comma = None;
                out.push(c);
            }
        }
    }
    out
}

/// `provider.<provider>.models.<model>` in a config file, which OpenCode 2's
/// config format spells `providers`.
fn config_model_entry<'a>(config: &'a Value, provider: &str, model: &str) -> Option<&'a Value> {
    ["provider", "providers"]
        .into_iter()
        .find_map(|key| config.get(key)?.get(provider)?.get("models")?.get(model))
}

/// The catalog entry a config model stands for. OpenCode resolves a config
/// model against the catalog by its `id` (`api.id` in OpenCode 2's format)
/// before its own key, so an alias still inherits the real model's window.
fn declared_catalog_id(entry: &Value) -> Option<&str> {
    entry
        .get("id")
        .or_else(|| entry.get("api").and_then(|api| api.get("id")))
        .and_then(Value::as_str)
        .filter(|id| !id.trim().is_empty())
}

/// A positive token count. OpenCode truncates a fractional one.
fn token_count(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| {
            value
                .as_f64()
                .filter(|count| count.is_finite() && *count >= 1.0)
                .map(|count| count.trunc() as u64)
        })
        .filter(|count| *count > 0)
}

/// `provider → model → limit.context` for every model a catalog lists, all
/// this needs of one. A model listed without a usable window maps to `None`.
type ContextIndex = HashMap<String, HashMap<String, Option<u64>>>;

/// What a catalog says of a model: `None` if it does not list the model at
/// all, `Some(None)` if it lists it without a window.
fn catalog_entry(index: &ContextIndex, provider: &str, model: &str) -> Option<Option<u64>> {
    index.get(provider)?.get(model).copied()
}

/// Index a models.dev `api.json`, the shape OpenCode caches:
/// `{ <provider>: { models: { <model>: { limit: { context } } } } }`. Keyed by
/// the map keys, which is how OpenCode keys its provider list. Input that is
/// not a catalog indexes to nothing.
fn index_models_dev(raw: &str) -> ContextIndex {
    let Ok(Value::Object(providers)) = serde_json::from_str::<Value>(raw) else {
        return ContextIndex::new();
    };
    let mut index = ContextIndex::new();
    for (provider_id, provider) in providers {
        let Some(models) = provider.get("models").and_then(Value::as_object) else {
            continue;
        };
        let windows = models.iter().map(|(model_id, model)| {
            let window = model
                .get("limit")
                .and_then(|limit| limit.get("context"))
                .and_then(token_count);
            (model_id.clone(), window)
        });
        index.insert(provider_id, windows.collect());
    }
    index
}

/// `(mtime, size)`, the change detector `summary_cache` uses too.
type Fingerprint = (Option<SystemTime>, u64);

fn fingerprint(path: &Path) -> Option<Fingerprint> {
    let meta = crate::scoped_fs::metadata(path).ok()?;
    Some((meta.modified().ok(), meta.len()))
}

struct CachedCatalog {
    path: PathBuf,
    fingerprint: Fingerprint,
    index: Arc<ContextIndex>,
}

/// The catalog at `path`, parsed once per version of the file.
///
/// Every OpenCode history read lands here, and the token-usage sync reads each
/// changed OpenCode session back to back, so re-parsing a multi-megabyte
/// catalog every time would cost that much per session. One slot is enough
/// for the one catalog a process reads; the fingerprint picks up OpenCode's
/// periodic refresh on the next read. A file that changes while it is read is
/// used but not remembered.
fn cached_catalog_index(path: &Path) -> Option<Arc<ContextIndex>> {
    static SLOT: Mutex<Option<CachedCatalog>> = Mutex::new(None);

    let before = fingerprint(path)?;
    if let Ok(slot) = SLOT.lock() {
        if let Some(cached) = slot
            .as_ref()
            .filter(|cached| cached.path == path && cached.fingerprint == before)
        {
            return Some(Arc::clone(&cached.index));
        }
    }

    let raw = crate::scoped_fs::read_to_string(path).ok()?;
    let index = Arc::new(index_models_dev(&raw));
    if fingerprint(path) == Some(before) {
        if let Ok(mut slot) = SLOT.lock() {
            *slot = Some(CachedCatalog {
                path: path.to_path_buf(),
                fingerprint: before,
                index: Arc::clone(&index),
            });
        }
    }
    Some(index)
}

/// coaxi's bundled models.dev snapshot, indexed once per process.
fn bundled_catalog_index() -> &'static ContextIndex {
    static INDEX: OnceLock<ContextIndex> = OnceLock::new();
    INDEX.get_or_init(|| {
        let mut index = ContextIndex::new();
        for provider in crate::acp::opencode_catalog::bundled_catalog() {
            let windows = provider
                .models
                .into_iter()
                .map(|model| (model.id, model.context.filter(|window| *window > 0)));
            index.entry(provider.id).or_default().extend(windows);
        }
        index
    })
}

