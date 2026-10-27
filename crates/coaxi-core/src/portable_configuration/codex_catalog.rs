// 来源 Codeg aff1e6c278453ed8d0b96a5f664edcdca621aa3f，Apache-2.0；原目录纯算法移植。
//! Codex custom-model catalog generation for codex's `model_catalog_json`.
//!
//! Codex only lists a model in its picker when the model is a first-class
//! catalog entry with `visibility: "list"` (and `supported_in_api: true`, or a
//! ChatGPT login). A model set only via the root `model` key is visible *only*
//! while it is the current value — which is why a stale preference replay makes
//! a custom model vanish, and why it cannot be re-selected once dropped.
//!
//! To make custom models first-class we generate a `model_catalog_json` file
//! and point `~/.codex/config.toml` at it. Because that key is a **whole-table
//! replace** (codex ignores its own catalog entirely once set), the generated
//! file must contain *every* model the user wants visible — so we auto-include
//! the current official catalog **verbatim** (minus any the user removed) and
//! append the user's custom entries.
//!
//! The official catalog is sourced at runtime from the codex coaxi actually
//! launches (see 实际启动程序提供的目录读取入口); this module is the pure,
//! snapshot-in / catalog-out core. We store a **compact** intent
//! ([`CodexModelConfig`]: sparse `customs` + `excluded_officials`) so the set of
//! officials tracks whatever codex ships, and expand it against a provided
//! `snapshot` at write time.
//!
//! A single unknown enum value makes codex reject the *entire* catalog (every
//! model then vanishes), so custom overrides are **sanitized** here against the
//! authoritative enum sets before they can reach the file.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Official codex catalog snapshot compiled into the binary. Generated from the
/// `codex debug models --bundled` of the codex that `codex-acp` drives (shape
/// `{"models":[ ModelInfo, ... ]}`). This is only the **offline fallback**; the
/// live catalog is fetched at runtime by 实际启动程序提供的目录读取入口.
const BUNDLED_SNAPSHOT: &str =
    include_str!("../../resources/portable-config/codex-bundled-catalog.json");

/// File name (relative to `CODEX_HOME`) of the generated catalog codex reads.
pub const CATALOG_REL: &str = "coaxi-model-catalog.json";
/// File name (relative to `CODEX_HOME`) of the compact source list we round-trip
/// back into the editor when no DB provider owns the list (api-key mode).
pub const SOURCE_REL: &str = "coaxi-model-catalog.source.json";

/// Fields coaxi owns/derives itself, so they are never captured as `overrides`
/// when importing a pre-existing catalog: `slug`/`display_name`/`context_window`
/// map to dedicated compact fields, and `visibility`/`supported_in_api`/
/// `priority`/`upgrade` are force-set by [`expand_to_catalog`].
const IMPORT_SKIP_KEYS: &[&str] = &[
    "slug",
    "display_name",
    "context_window",
    "visibility",
    "supported_in_api",
    "priority",
    "upgrade",
];

/// One strict-enum field: its allowed values and whether JSON `null` is also
/// accepted. Both halves are load-bearing — codex rejects the WHOLE catalog on
/// an unknown variant *and* on a `null` in a non-optional slot.
struct EnumSpec {
    allowed: &'static [&'static str],
    nullable: bool,
}

// Authoritative value sets + nullability, extracted from the codex binary itself
// by feeding it candidate catalogs and reading the full `unknown variant …,
// expected …` / `invalid type: null, expected …` errors (re-probed on 0.159.2,
// unchanged since 0.147 — treat these as version-specific and re-probe when
// codex moves).
fn enum_spec_for(key: &str) -> Option<EnumSpec> {
    let spec = |allowed, nullable| Some(EnumSpec { allowed, nullable });
    match key {
        "default_reasoning_summary" => spec(&["auto", "concise", "detailed", "none"], false),
        "default_verbosity" => spec(&["low", "medium", "high"], true),
        "shell_type" => spec(
            &[
                "default",
                "local",
                "unified_exec",
                "disabled",
                "shell_command",
            ],
            false,
        ),
        // Only `freeform` is a variant; `function` would reject the whole catalog.
        "apply_patch_tool_type" => spec(&["freeform"], true),
        "web_search_tool_type" => spec(&["text", "text_and_image"], false),
        // `tool_mode` / `multi_agent_version` parse leniently (codex silently
        // downgrades an unknown value to `None`), but pinning them keeps the
        // written catalog honest about what the user actually selected.
        "tool_mode" => spec(&["direct", "code_mode", "code_mode_only"], true),
        "multi_agent_version" => spec(&["v1", "v2"], true),
        _ => None,
    }
}

/// `ModelInfo` fields codex parses as a plain `bool`. A string here ("true",
/// "yes", …) rejects the whole catalog, so the type is checked, not assumed.
///
/// `supports_parallel_tool_calls` is kept although 0.153.4 dropped the field
/// (it now parses as an ignored unknown key): guarding it still costs nothing
/// and keeps a stored override honest for anyone pinned to an older codex.
///
/// `supports_experimental_context` is the only `ModelInfo` field 0.154.0 added
/// (codex-acp 1.12.0 moves `@openai/codex` ^0.153.4 → ^0.154.0). It is strict:
/// `supports_experimental_context = "yes"` earns `invalid type: string "yes",
/// expected a boolean` and takes the WHOLE catalog down with it — probed
/// against the 0.154.0 binary, same as the rest of this list.
///
/// `node_repl_auto_review_required` / `node_repl_disabled` ship as booleans on
/// every entry and are just as strict: a string earns the same `expected a
/// boolean` rejection, and so does a `null` — probed against the 0.155.1
/// binary, which also re-confirmed every other entry here. The editor offers
/// no control for either, so only an imported or hand-edited override can
/// carry one, which is exactly the input this list exists to catch.
///
/// `supports_reasoning_effort_updates` is the only boolean 0.156.1 added
/// (codex-acp 1.13.1 moves `@openai/codex` ^0.155.1 → ^0.156.1). It ships on
/// every entry and is as strict as the rest: `"yes"` earns `expected a
/// boolean`, and so does `null` — probed against the 0.156.1 binary, which
/// also re-confirmed every other entry here and every enum set above. The one
/// other key the 0.156.1 snapshot newly carries, `default_service_tier`
/// (`"priority"` on GPT-6 Sol and Luna), is an optional string rather than an
/// enum — an unknown tier name parses and only a non-string is refused — so
/// it needs no entry in either list.
///
/// 0.158.0 (codex-acp 2.0.0 moves `@openai/codex` ^0.156.1 → ^0.158.0) adds no
/// `ModelInfo` field at all; it deletes the hidden `gpt-5.4` stub outright
/// (10 slugs). Re-probed against the 0.158.0 binary: `"yes"` and `null` in a
/// boolean still take the catalog down, and so does an unknown `shell_type`.
///
/// 0.159.x (codex-acp 2.0.1 moves `@openai/codex` ^0.158.0 → ^0.159.1) adds
/// GPT-6.1 Sol (11 slugs) and still no `ModelInfo` field: no key is new to the
/// catalog, and every existing entry keeps its previous key set (GPT-6.1 Sol
/// sets the optional `multi_agent_reasoning_effort` and omits
/// `default_service_tier`, which GPT-6 Sol does the other way round). Re-probed
/// against the 0.159.2 binary: each field below rejects both `"yes"` and `null`
/// (bar the ignored `supports_parallel_tool_calls`), no boolean-typed key sits
/// outside this list, and every enum set above is unchanged.
///
/// 0.160.1 (codex-acp 2.2.1 moves `@openai/codex` ^0.159.1 → ^0.160.1) ships
/// the bundled catalog byte for byte as 0.159.3 did and adds no `ModelInfo`
/// field. Re-probed against the 0.160.1 binary: every field here and every
/// enum set above behaves as on 0.159.3. Its one new key is nested — see
/// [`model_messages_override_is_safe`].
const BOOL_FIELDS: &[&str] = &[
    "use_responses_lite",
    "supported_in_api",
    "support_verbosity",
    "supports_parallel_tool_calls",
    "supports_search_tool",
    "supports_image_detail_original",
    "supports_reasoning_summary_parameter",
    "supports_experimental_context",
    "include_apps_usage_instructions",
    "include_plugin_usage_instructions",
    "include_skills_usage_instructions",
    "node_repl_auto_review_required",
    "node_repl_disabled",
    "supports_reasoning_effort_updates",
];

/// Whether a custom `overrides` entry is safe to write. A single value codex
/// can't parse rejects the entire catalog, so:
/// - strict enum fields must carry a string in their allowed set, or `null`
///   only where that enum is genuinely optional;
/// - boolean fields must carry an actual boolean;
/// - `default_reasoning_level` must name one of the clone base's supported
///   efforts (codex 0.147 accepts it leniently, but older codex is strict and
///   the meaningful values are per-model anyway);
/// - `model_messages` must be an object (or `null`) whose plain-text members
///   are strings (see [`model_messages_override_is_safe`]);
/// - every other field passes through (unknown keys are ignored by codex).
fn sanitized_override(key: &str, value: &Value, base: Option<&Map<String, Value>>) -> bool {
    if key == "model_messages" {
        return model_messages_override_is_safe(value);
    }
    if let Some(spec) = enum_spec_for(key) {
        if value.is_null() {
            return spec.nullable;
        }
        return value
            .as_str()
            .map(|s| spec.allowed.contains(&s))
            .unwrap_or(false);
    }
    if BOOL_FIELDS.contains(&key) {
        return value.is_boolean();
    }
    if value.is_null() {
        return true;
    }
    if key == "default_reasoning_level" {
        let Some(s) = value.as_str() else {
            return false;
        };
        return base
            .and_then(|b| b.get("supported_reasoning_levels"))
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|e| e.get("effort").and_then(Value::as_str))
                    .any(|e| e == s)
            })
            .unwrap_or(false);
    }
    true
}

/// The `ModelMessages` members that hold plain text. codex parses each as an
/// optional string, so `null` is fine and anything else is not.
const MODEL_MESSAGES_TEXT_FIELDS: &[&str] = &[
    "content_filter_guidance",
    "persistent_instructions",
    "instructions_template",
];

/// Whether a `model_messages` override is safe to write: `null`, or an object
/// whose plain-text members are strings.
///
/// codex parses the whole `model_messages` object strictly, and 0.160 added a
/// member to it: `content_filter_guidance`, the developer note it sends after
/// a content filter blocks a response. 0.159.3 ignored the key; 0.160.1 takes
/// a number there for `invalid type: integer 42, expected a string` and
/// rejects the WHOLE catalog, every model with it (probed with a temporary
/// `model_catalog_json`; a string and `null` both load). The other two
/// members, and the object itself, were already that strict: a number in
/// `persistent_instructions` or `instructions_template`, or a string for
/// `model_messages`, rejects the catalog on 0.159.3 as on 0.160.1. The editor
/// offers no control for any of this, so only an imported or hand-edited
/// override can carry one, which is the input these checks exist for. The
/// nested message tables beyond those members pass through as before.
fn model_messages_override_is_safe(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::Object(members) => MODEL_MESSAGES_TEXT_FIELDS.iter().all(|field| {
            members
                .get(*field)
                .is_none_or(|text| text.is_null() || text.is_string())
        }),
        _ => false,
    }
}

/// One user-configured **custom** codex model, stored compactly. Heavy
/// `ModelInfo` fields are cloned from `base` at expansion time; `overrides`
/// holds only the fields the user actually changed. Field names mirror the TS
/// `CodexCustomEntry`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexCustomEntry {
    pub slug: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u64>,
    /// Snapshot slug whose full `ModelInfo` is the clone template.
    pub base: String,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub overrides: Map<String, Value>,
}

/// The compact intent stored in `provider.model` / the source sidecar. The set
/// of official models is **not** stored — it is auto-included from the runtime
/// snapshot at expand time, so it tracks whatever codex ships; only the user's
/// deviations (custom additions + removed officials) are persisted.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexModelConfig {
    #[serde(default)]
    pub customs: Vec<CodexCustomEntry>,
    /// Official slugs the user removed from the picker.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub excluded_officials: Vec<String>,
    /// Slug that becomes codex's root `model` + `OPENAI_MODEL`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}

/// What the caller must inject into `config.toml` after files are written.
#[derive(Debug, Clone, PartialEq)]
pub struct CatalogInjection {
    /// Relative path to write as the `model_catalog_json` value.
    pub catalog_rel: &'static str,
    /// The default model slug to write as root `model`.
    pub default_model: Option<String>,
}

/// Parse the compiled-in offline snapshot into its `models` array (opaque
/// `Value`s), in codex's picker order (see [`sort_by_priority`]). Only used as
/// a fallback when the runtime catalog is unavailable.
pub fn bundled_snapshot_models() -> Vec<Value> {
    let mut models = serde_json::from_str::<Value>(BUNDLED_SNAPSHOT)
        .ok()
        .as_ref()
        .and_then(|v| v.get("models"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    sort_by_priority(&mut models);
    models
}

/// A catalog entry's `priority`: codex's picker position, ascending. An entry
/// without one sorts last.
fn priority_of(model: &Value) -> i64 {
    model
        .get("priority")
        .and_then(Value::as_i64)
        .unwrap_or(i64::MAX)
}

/// Put a catalog in the order codex itself presents it: ascending `priority`,
/// ties kept in catalog order (codex's `build_available_models` does a stable
/// `sort_by_key` on the same field, then marks the first visible entry the
/// default).
///
/// `codex debug models --bundled` prints entries in FILE order, and since
/// codex 0.159.1 that is no longer priority order: GPT-6.1 Sol was inserted
/// second, at priority 1, behind GPT-6 Astra's 2. Anything that reads "the
/// first entry" as "codex's first choice" has to sort first.
pub fn sort_by_priority(models: &mut [Value]) {
    models.sort_by_key(priority_of);
}

/// The safest default clone base: the highest-priority (lowest `priority`)
/// snapshot entry, so a stored `base` that codex later renames still expands to
/// a valid entry rather than one missing required fields.
pub fn fallback_base_slug(snapshot: &[Value]) -> Option<String> {
    snapshot
        .iter()
        .min_by_key(|m| priority_of(m))
        .and_then(|m| m.get("slug").and_then(Value::as_str))
        .map(str::to_owned)
}

fn slug_of(model: &Value) -> Option<&str> {
    model.get("slug").and_then(Value::as_str)
}

fn is_listable(model: &Value) -> bool {
    model.get("visibility").and_then(Value::as_str) == Some("list")
}

/// Expand a compact config into a full `{"models":[ ModelInfo, ... ]}` catalog.
///
/// Because `model_catalog_json` is a whole-table replace, the output contains
/// **all** official models (verbatim, so codex's hidden entries such as
/// `codex-auto-review` stay hidden) minus the ones the user removed, plus the
/// user's custom entries. Custom entries clone their `base` snapshot ModelInfo
/// (falling back to the highest-priority entry when `base` is unknown), apply
/// **sanitized** overrides, and are forced `visibility:"list"` +
/// `supported_in_api:true`. Priority is renumbered by final order — customs
/// first, then the officials in codex's own priority order, whatever order the
/// snapshot arrived in — so the picker ordering is deterministic without
/// colliding official priorities, and the officials keep the order (and the
/// default) codex would give them without the takeover.
pub fn expand_to_catalog(config: &CodexModelConfig, snapshot: &[Value]) -> Value {
    let excluded: HashSet<&str> = config
        .excluded_officials
        .iter()
        .map(String::as_str)
        .collect();
    let fallback = fallback_base_slug(snapshot);
    let mut out: Vec<Value> = Vec::with_capacity(config.customs.len() + snapshot.len());

    // Customs first — surface the user's own models at the top of the picker.
    for c in &config.customs {
        let base = snapshot
            .iter()
            .find(|m| slug_of(m) == Some(c.base.as_str()))
            .or_else(|| snapshot.iter().find(|m| slug_of(m) == fallback.as_deref()));
        let base_obj = base.and_then(Value::as_object);
        let mut obj = base_obj.cloned().unwrap_or_default();

        for (k, v) in &c.overrides {
            if sanitized_override(k, v, base_obj) {
                obj.insert(k.clone(), v.clone());
            }
        }

        obj.insert("slug".into(), Value::String(c.slug.clone()));
        obj.insert(
            "display_name".into(),
            Value::String(c.display_name.clone().unwrap_or_else(|| c.slug.clone())),
        );
        if let Some(cw) = c.context_window {
            obj.insert("context_window".into(), Value::from(cw));
            let max = obj
                .get("max_context_window")
                .and_then(Value::as_u64)
                .unwrap_or(0)
                .max(cw);
            obj.insert("max_context_window".into(), Value::from(max));
        }
        obj.insert("visibility".into(), Value::String("list".into()));
        obj.insert("supported_in_api".into(), Value::Bool(true));
        obj.insert("upgrade".into(), Value::Null);
        out.push(Value::Object(obj));
    }

    // Then every official verbatim, minus the ones the user removed. A removal
    // only applies to models codex actually *lists*: codex retires a model by
    // flipping it to `hide` while keeping it around as a migration stub (its
    // `upgrade` block) or for internal use (`codex-auto-review`), and dropping
    // those breaks codex rather than tidying the picker.
    //
    // Walked in priority order, not snapshot order: the renumbering below turns
    // position into priority, so a snapshot in file order (see
    // `sort_by_priority`) would otherwise hand GPT-6 Astra the slot — and the
    // default — codex gives GPT-6.1 Sol.
    let mut officials: Vec<&Value> = snapshot.iter().collect();
    officials.sort_by_key(|m| priority_of(m));
    for m in officials {
        if let Some(slug) = slug_of(m) {
            if excluded.contains(slug) && is_listable(m) {
                continue;
            }
            out.push(m.clone());
        }
    }

    // Renumber priority by final order so ordering is deterministic.
    for (i, entry) in out.iter_mut().enumerate() {
        if let Some(o) = entry.as_object_mut() {
            o.insert("priority".into(), Value::from(i as i64));
        }
    }

    Value::Object(Map::from_iter([("models".to_string(), Value::Array(out))]))
}

/// The default model slug written as codex's root `model`: the explicit
/// `default` when it names a listed model, else the first custom, else the
/// non-excluded listable official codex itself ranks first (lowest `priority`),
/// else `None`.
pub fn default_slug(config: &CodexModelConfig, snapshot: &[Value]) -> Option<String> {
    let excluded: HashSet<&str> = config
        .excluded_officials
        .iter()
        .map(String::as_str)
        .collect();
    let is_listed = |slug: &str| -> bool {
        config.customs.iter().any(|c| c.slug == slug)
            || snapshot
                .iter()
                .any(|m| slug_of(m) == Some(slug) && !excluded.contains(slug))
    };
    if let Some(d) = &config.default {
        if is_listed(d) {
            return Some(d.clone());
        }
    }
    if let Some(c) = config.customs.first() {
        return Some(c.slug.clone());
    }
    // `min_by_key` keeps the first of equal keys, matching codex's stable sort.
    snapshot
        .iter()
        .filter(|m| slug_of(m).map(|s| !excluded.contains(s)).unwrap_or(false) && is_listable(m))
        .min_by_key(|m| priority_of(m))
        .and_then(|m| slug_of(m).map(str::to_owned))
}

/// Whether the config still deviates from codex's own catalog **in a way that
/// applies** — i.e. "feature off", so codex should use its own catalog
/// untouched. Removals of models codex has since retired to `hide` are ghosts:
/// [`expand_to_catalog`] ignores them, so a config carrying only ghosts would
/// have coaxi replace the whole model table (freezing the official list) for a
/// result identical to codex's own. Treating it as empty hands control back.
pub fn is_effectively_empty(config: &CodexModelConfig, snapshot: &[Value]) -> bool {
    if !config.customs.is_empty() {
        return false;
    }
    let listable: HashSet<&str> = snapshot
        .iter()
        .filter(|m| is_listable(m))
        .filter_map(|m| slug_of(m))
        .collect();
    // An empty/unavailable snapshot can't prove a removal is stale — stay
    // conservative and keep whatever the user configured.
    if listable.is_empty() {
        return config.excluded_officials.is_empty();
    }
    !config
        .excluded_officials
        .iter()
        .any(|s| listable.contains(s.as_str()))
}

/// The default slug for `OPENAI_MODEL` / root `model` **without** a snapshot:
/// the explicit `default`, else the first custom. Officials are omitted (they
/// need the snapshot to enumerate); when neither is set the caller leaves the
/// key unset so codex picks its own default from its catalog. Used on the env /
/// provider paths that don't (and shouldn't) spawn codex.
pub fn default_slug_for_env(config: &CodexModelConfig) -> Option<String> {
    config
        .default
        .clone()
        .or_else(|| config.customs.first().map(|c| c.slug.clone()))
}

/// Map one legacy `{slug,base,…}` entry (or the old `CodexModelEntry` shape)
/// into a custom entry.
fn legacy_value_to_custom(m: &Value) -> Option<CodexCustomEntry> {
    let obj = m.as_object()?;
    let slug = obj
        .get("slug")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())?;
    let base = obj
        .get("base")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .unwrap_or(slug)
        .to_string();
    let overrides = obj
        .get("overrides")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    Some(CodexCustomEntry {
        slug: slug.to_string(),
        display_name: obj
            .get("displayName")
            .or_else(|| obj.get("display_name"))
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_owned),
        context_window: obj
            .get("contextWindow")
            .or_else(|| obj.get("context_window"))
            .and_then(Value::as_u64),
        base,
        overrides,
    })
}

fn single_custom_config(slug: &str) -> CodexModelConfig {
    let slug = slug.trim().to_string();
    if slug.is_empty() {
        return CodexModelConfig::default();
    }
    CodexModelConfig {
        customs: vec![CodexCustomEntry {
            slug: slug.clone(),
            display_name: None,
            context_window: None,
            base: slug.clone(),
            overrides: Map::new(),
        }],
        excluded_officials: Vec::new(),
        default: Some(slug),
    }
}

/// Parse the compact config from a stored value, leniently and with migration.
///
/// - `None`/blank → empty config (feature off).
/// - New shape `{"customs":[…],"excludedOfficials":[…],"default":…}` → parsed.
/// - Legacy `{"models":[…],"default":…}` → each model migrated to a custom.
/// - Any other JSON object → empty config.
/// - A bare slug (JSON-quoted or not) → a single custom cloning that slug.
pub fn parse_model_config(raw: Option<&str>) -> CodexModelConfig {
    let Some(raw) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return CodexModelConfig::default();
    };

    match serde_json::from_str::<Value>(raw) {
        Ok(Value::Object(obj)) => {
            if obj.contains_key("customs") || obj.contains_key("excludedOfficials") {
                return serde_json::from_value(Value::Object(obj)).unwrap_or_default();
            }
            if let Some(models) = obj.get("models").and_then(Value::as_array) {
                return CodexModelConfig {
                    customs: models.iter().filter_map(legacy_value_to_custom).collect(),
                    excluded_officials: Vec::new(),
                    default: obj
                        .get("default")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                };
            }
            CodexModelConfig::default()
        }
        Ok(Value::String(s)) => single_custom_config(&s),
        _ => single_custom_config(raw),
    }
}

/// Adopt a **pre-existing** `{"models":[ ModelInfo, ... ]}` catalog (one the user
/// configured by hand, or coaxi's own catalog when its source sidecar is missing)
/// into the compact config, reconciled against the live official catalog:
/// non-official models become `customs`, listable officials **absent** from the
/// foreign catalog are recorded as `excluded_officials` (the user removed them),
/// and `root_model` seeds `default`. This makes the editor show the user's real
/// intent instead of appearing empty (and being clobbered on the next save).
pub fn import_catalog(
    catalog: &Value,
    root_model: Option<&str>,
    snapshot: &[Value],
) -> CodexModelConfig {
    let official_slugs: HashSet<&str> = snapshot.iter().filter_map(|m| slug_of(m)).collect();
    let fallback = fallback_base_slug(snapshot).unwrap_or_default();
    let foreign: Vec<&Map<String, Value>> = catalog
        .get("models")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_object).collect())
        .unwrap_or_default();
    let foreign_slugs: HashSet<&str> = foreign
        .iter()
        .filter_map(|o| o.get("slug").and_then(Value::as_str))
        .collect();

    let base_obj = snapshot
        .iter()
        .find(|b| slug_of(b) == Some(fallback.as_str()))
        .and_then(Value::as_object);

    let mut customs = Vec::new();
    for obj in &foreign {
        let Some(slug) = obj
            .get("slug")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        // An official the user kept needs no storage — it is auto-included.
        if official_slugs.contains(slug) {
            continue;
        }
        let mut overrides = Map::new();
        for (k, v) in *obj {
            if IMPORT_SKIP_KEYS.contains(&k.as_str()) {
                continue;
            }
            if base_obj.and_then(|b| b.get(k)) != Some(v) {
                overrides.insert(k.clone(), v.clone());
            }
        }
        customs.push(CodexCustomEntry {
            slug: slug.to_string(),
            display_name: obj
                .get("display_name")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_owned),
            context_window: obj.get("context_window").and_then(Value::as_u64),
            base: fallback.clone(),
            overrides,
        });
    }

    // Listable officials the foreign catalog dropped = deliberately removed.
    // Hidden officials are never inferred-excluded (the user likely never saw
    // them, and they may back codex internals).
    let excluded_officials: Vec<String> = snapshot
        .iter()
        .filter(|m| is_listable(m))
        .filter_map(|m| slug_of(m))
        .filter(|s| !foreign_slugs.contains(s))
        .map(str::to_owned)
        .collect();

    let default = root_model
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned);

    CodexModelConfig {
        customs,
        excluded_officials,
        default,
    }
}
