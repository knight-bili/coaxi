//! Test-only comparison with the unchanged fixed donor parser executable.
//! No provider, database, public method or product environment flag is added.
use super::{NativeContent, model::NativeTranscriptSummary};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

fn configuration() -> Option<(PathBuf, PathBuf)> {
    let binary = std::env::var_os("COAXI_NATIVE_PARSER_REFERENCE");
    let output = std::env::var_os("COAXI_NATIVE_PARSER_DIFF_OUTPUT");
    match (binary, output) {
        (None, None) => None,
        (Some(binary), Some(output)) => {
            let binary = PathBuf::from(binary);
            let output = PathBuf::from(output);
            assert!(binary.is_absolute() && binary.is_file());
            assert!(output.is_absolute() && output.is_dir());
            assert_eq!(
                fs::read_to_string(output.join(".native-parser-differential-root")).unwrap(),
                "controlled-native-parser-fixtures-v1"
            );
            Some((binary, output))
        }
        _ => panic!("Reference fixture requires both binary and output directory"),
    }
}

pub(super) fn enabled() -> bool {
    configuration().is_some()
}

pub(super) fn prepare_codex(label: &str, rows: &mut [Value]) -> Option<(PathBuf, PathBuf, String)> {
    let (_, output) = configuration()?;
    let input_hash = format!("{:x}", Sha256::digest(serde_json::to_vec(rows).unwrap()));
    let case = format!("{label}_{}", &input_hash[..12]);
    assert!(
        case.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    );
    let home = output.join("sources").join(&case);
    fs::create_dir_all(home.join("sessions")).unwrap();
    fs::create_dir_all(home.join("project")).unwrap();
    // Both parsers receive the same fixture-scoped metadata; native cwd never
    // authorizes reads of an actual user's project by the reference process.
    for row in rows.iter_mut() {
        if row["type"] == "session_meta" {
            row["payload"]["cwd"] = home.join("project").to_string_lossy().into_owned().into();
        }
    }
    let path = home.join("sessions/rollout-2026-10-18T00-00-00-reference.jsonl");
    fs::write(
        &path,
        rows.iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n"),
    )
    .unwrap();
    Some((home, path, case))
}

fn differences(path: &str, actual: &Value, reference: &Value, out: &mut Vec<String>) {
    if actual == reference {
        return;
    }
    match (actual, reference) {
        (Value::Object(a), Value::Object(b)) => {
            let keys: BTreeSet<_> = a.keys().chain(b.keys()).collect();
            for key in keys {
                let next = format!("{path}/{}", key.replace('~', "~0").replace('/', "~1"));
                match (a.get(key), b.get(key)) {
                    (Some(a), Some(b)) => differences(&next, a, b, out),
                    _ => out.push(next),
                }
            }
        }
        (Value::Array(a), Value::Array(b)) if a.len() == b.len() => {
            for (index, (a, b)) in a.iter().zip(b).enumerate() {
                differences(&format!("{path}/{index}"), a, b, out);
            }
        }
        _ => out.push(path.to_owned()),
    }
}

pub(super) fn compare(
    label: &str,
    home: &Path,
    adapter: &str,
    source: &Path,
    id: &str,
    turns: &[super::model::MessageTurn],
    stats: Option<&super::model::SessionStats>,
    summary: Option<&NativeTranscriptSummary>,
    full_content: Option<(&str, &str)>,
) {
    let Some((binary, output)) = configuration() else {
        return;
    };
    assert!(
        label
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    );
    let actual = json!({"turns":turns,"session_stats":stats,"summary":summary});
    let bytes = serde_json::to_vec(&actual).unwrap();
    let digest = format!("{:x}", Sha256::digest(&bytes));
    let name = format!("{label}-{}", &digest[..16]);
    let result = std::process::Command::new(&binary)
        .arg(adapter)
        .arg(home)
        .arg(source)
        .arg(id)
        .current_dir(home)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("CODEX_HOME", home)
        .env("GROK_HOME", home)
        .output()
        .unwrap();
    fs::write(
        output.join(format!("{name}-core.json")),
        serde_json::to_vec_pretty(&actual).unwrap(),
    )
    .unwrap();
    fs::write(
        output.join(format!("{name}-reference.json")),
        &result.stdout,
    )
    .unwrap();
    assert!(
        result.status.success(),
        "Reference parser failed for {label}: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let reference: Value = serde_json::from_slice(&result.stdout).unwrap();
    let mut paths = Vec::new();
    differences("", &actual, &reference, &mut paths);
    let mut expected = Vec::new();
    let mut unexpected = Vec::new();
    for path in paths {
        let core = actual.pointer(&path).unwrap_or(&Value::Null);
        let original = reference.pointer(&path).unwrap_or(&Value::Null);
        let accepted = full_content.is_some_and(|(tool_id, marker)| {
            let Some((parent, field)) = path.rsplit_once('/') else {
                return false;
            };
            if !matches!(field, "input_preview" | "output_preview") {
                return false;
            }
            let block = actual.pointer(parent).unwrap();
            block["tool_use_id"] == tool_id
                && core
                    .as_str()
                    .zip(original.as_str())
                    .is_some_and(|(a, b)| a.contains(marker) && a.len() > b.len())
        });
        let description = json!({"pointer":path,"core_string_bytes":core.as_str().map(str::len),"reference_string_bytes":original.as_str().map(str::len),
            "core_value_sha256":format!("{:x}",Sha256::digest(serde_json::to_vec(core).unwrap())),
            "reference_value_sha256":format!("{:x}",Sha256::digest(serde_json::to_vec(original).unwrap())),
            "classification":if accepted{"intentional_complete_content_retention"}else{"unexpected"}});
        if accepted {
            expected.push(description);
        } else {
            unexpected.push(description);
        }
    }
    let report = json!({"schema_version":1,"label":label,"adapter":adapter,"source":"fixed_coaxi_aff1e6c_original_parser_body",
        "exact":expected.is_empty() && unexpected.is_empty(),"intentional_differences":expected,"unexpected_differences":unexpected,
        "core_projection_sha256":digest,"reference_stdout_sha256":format!("{:x}",Sha256::digest(&result.stdout)),"provider_calls":0});
    fs::write(
        output.join(format!("{name}-result.json")),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    assert!(
        unexpected.is_empty(),
        "Unexpected original-parser differences for {label}: {}",
        serde_json::to_string(&unexpected).unwrap()
    );
}

pub(super) fn compare_native(
    label: &str,
    home: &Path,
    adapter: &str,
    source: &Path,
    id: &str,
    content: &NativeContent,
    full_content: Option<(&str, &str)>,
) {
    compare(
        label,
        home,
        adapter,
        source,
        id,
        &content.turns,
        content.session_stats.as_ref(),
        content.summary.as_ref(),
        full_content,
    );
}
