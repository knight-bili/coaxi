//! Read-only verification of the pending migration's complete delta. Unchanged
//! Core structures remain owned by the existing database classifier.
use super::*;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Source {
    source_contract: String,
    source_schema: i64,
    source_migration: i64,
    tables: BTreeMap<String, String>,
    guards: BTreeMap<String, String>,
}
fn source() -> Result<Source> {
    let source: Source =
        serde_json::from_str(include_str!("../resources/delegation-source159.json"))?;
    anyhow::ensure!(
        source.source_contract == "v1.72"
            && source.source_schema == 159
            && source.source_migration == 209
            && source.tables.len() == 5
            && source.guards.len() == MODIFIED_GUARDS.len(),
        "Invalid compiled delegation source contract"
    );
    Ok(source)
}

// SQLite identifiers are case-insensitive, while quoted string values are not.
// Keep token boundaries and strings intact; whitespace/comments are not proof.
fn spans(sql: &str) -> Option<Vec<(String, usize, usize)>> {
    let b = sql.as_bytes();
    let mut i = 0;
    let mut out = Vec::new();
    while i < b.len() {
        if b[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if b[i..].starts_with(b"--") {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if b[i..].starts_with(b"/*") {
            i += 2;
            while i + 1 < b.len() && !b[i..].starts_with(b"*/") {
                i += 1;
            }
            if i + 1 >= b.len() {
                return None;
            }
            i += 2;
            continue;
        }
        let token_start = i;
        if matches!(b[i], b'\'' | b'"' | b'`' | b'[') {
            let open = b[i];
            let close = if open == b'[' { b']' } else { open };
            i += 1;
            let mut value = Vec::new();
            let mut closed = false;
            while i < b.len() {
                if b[i] == close {
                    i += 1;
                    if open != b'[' && i < b.len() && b[i] == close {
                        value.push(close);
                        i += 1;
                        continue;
                    }
                    closed = true;
                    break;
                }
                value.push(b[i]);
                i += 1;
            }
            if !closed {
                return None;
            }
            let value = String::from_utf8(value).ok()?;
            out.push((
                if open == b'\'' {
                    format!("s:{value}")
                } else {
                    format!("w:{}", value.to_ascii_lowercase())
                },
                token_start,
                i,
            ));
            continue;
        }
        if b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] >= 128 {
            let start = i;
            i += 1;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] >= 128) {
                i += 1;
            }
            out.push((
                format!("w:{}", sql[start..i].to_ascii_lowercase()),
                start,
                i,
            ));
            continue;
        }
        let start = i;
        i += 1;
        if i < b.len()
            && matches!(
                (b[start], b[i]),
                (b'<', b'>')
                    | (b'<', b'=')
                    | (b'>', b'=')
                    | (b'!', b'=')
                    | (b'=', b'=')
                    | (b'|', b'|')
                    | (b'<', b'<')
                    | (b'>', b'>')
            )
        {
            i += 1;
        }
        let op = &sql[start..i];
        out.push((
            match op {
                "!=" => "<>".into(),
                "==" => "=".into(),
                _ => op.into(),
            },
            start,
            i,
        ));
    }
    while out.last().is_some_and(|t| t.0 == ";") {
        out.pop();
    }
    Some(out)
}
fn tokens(sql: &str) -> Option<Vec<String>> {
    Some(spans(sql)?.into_iter().map(|t| t.0).collect())
}
pub(super) fn replace(mut source: String, from: &str, to: &str, once: bool) -> Result<String> {
    let source_tokens = spans(&source).context("Invalid migration source SQL")?;
    let pattern = tokens(from).context("Invalid migration SQL pattern")?;
    anyhow::ensure!(!pattern.is_empty(), "Empty migration SQL pattern");
    let positions = source_tokens
        .windows(pattern.len())
        .filter(|window| window.iter().zip(&pattern).all(|(a, b)| &a.0 == b))
        .map(|window| (window[0].1, window.last().unwrap().2))
        .collect::<Vec<_>>();
    anyhow::ensure!(
        !positions.is_empty() && (!once || positions.len() == 1),
        "Unexpected delegation migration source: {from}"
    );
    for (start, end) in positions.into_iter().rev() {
        source.replace_range(start..end, to);
    }
    Ok(source)
}
pub(super) fn table_end(sql: &str) -> Option<usize> {
    spans(sql)?.iter().rev().find(|t| t.0 == ")").map(|t| t.1)
}
pub(super) fn target_guard(name: &str) -> Result<String> {
    extend_guard(
        name,
        source()?
            .guards
            .remove(name)
            .context("Missing compiled source guard")?,
    )
}
pub(super) fn equivalent(left: &str, right: &str) -> bool {
    tokens(left).zip(tokens(right)).is_some_and(|(l, r)| l == r)
}
fn checks(sql: &str) -> Option<BTreeSet<Vec<String>>> {
    let tokens = tokens(sql)?;
    let mut out = BTreeSet::new();
    let mut i = 0;
    while i < tokens.len() {
        if tokens[i] != "w:check" || tokens.get(i + 1).is_none_or(|t| t != "(") {
            i += 1;
            continue;
        }
        let start = i + 2;
        let mut depth = 1;
        i = start;
        while i < tokens.len() && depth > 0 {
            match tokens[i].as_str() {
                "(" => depth += 1,
                ")" => depth -= 1,
                _ => {}
            }
            i += 1;
        }
        if depth != 0 {
            return None;
        }
        out.insert(tokens[start..i - 1].to_vec());
    }
    Some(out)
}
fn sql(connection: &Connection, kind: &str, name: &str) -> rusqlite::Result<Option<String>> {
    connection
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE type=?1 AND name=?2",
            params![kind, name],
            |r| r.get(0),
        )
        .optional()
}
fn metadata(
    connection: &Connection,
    table: &str,
) -> rusqlite::Result<BTreeMap<String, (String, bool, Option<Vec<String>>, i64, i64)>> {
    connection
        .prepare("SELECT name,type,[notnull],dflt_value,pk,hidden FROM pragma_table_xinfo(?1)")?
        .query_map([table], |r| {
            let default: Option<String> = r.get(3)?;
            Ok((
                r.get::<_, String>(0)?.to_ascii_lowercase(),
                (
                    r.get::<_, String>(1)?.to_ascii_lowercase(),
                    r.get(2)?,
                    default.as_deref().and_then(tokens),
                    r.get(4)?,
                    r.get(5)?,
                ),
            ))
        })?
        .collect()
}
fn column_items(sql: &str) -> Option<BTreeMap<String, Vec<String>>> {
    let ts = tokens(sql)?;
    let begin = ts.iter().position(|t| t == "(")? + 1;
    let mut depth = 0;
    let mut start = begin;
    let mut result = BTreeMap::new();
    for i in begin..ts.len() {
        match ts[i].as_str() {
            "(" => depth += 1,
            ")" if depth > 0 => depth -= 1,
            "," if depth == 0 => {
                if let Some(name) = ts[start].strip_prefix("w:") {
                    if !matches!(
                        name,
                        "check" | "constraint" | "foreign" | "primary" | "unique"
                    ) {
                        result.insert(name.into(), ts[start..i].to_vec());
                    }
                }
                start = i + 1;
            }
            ")" => {
                if start < i {
                    if let Some(name) = ts[start].strip_prefix("w:") {
                        if !matches!(
                            name,
                            "check" | "constraint" | "foreign" | "primary" | "unique"
                        ) {
                            result.insert(name.into(), ts[start..i].to_vec());
                        }
                    }
                }
                return Some(result);
            }
            _ => {}
        }
    }
    None
}
fn changed_columns(old: &str, new: &str) -> Option<BTreeSet<String>> {
    let before = column_items(old)?;
    let after = column_items(new)?;
    Some(
        before
            .keys()
            .chain(after.keys())
            .filter(|k| before.get(*k) != after.get(*k))
            .cloned()
            .collect(),
    )
}
fn binary_comparison(column: &[String]) -> bool {
    column
        .windows(2)
        .filter(|pair| pair[0] == "w:collate")
        .all(|pair| pair[1] == "w:binary")
}

pub(super) fn matches(connection: &Connection, target: bool) -> Result<bool> {
    let source = source()?;
    let reference = Connection::open_in_memory()?;
    reference.execute_batch("PRAGMA foreign_keys=OFF;")?;
    for (name, old) in &source.tables {
        let new = extend_table(name, old.clone())?;
        let expected = if target { &new } else { old };
        reference.execute_batch(expected)?;
        let Some(actual) = sql(connection, "table", name)? else {
            return Ok(false);
        };
        let old_checks = checks(old).context("Invalid compiled source checks")?;
        let new_checks = checks(&new).context("Invalid compiled target checks")?;
        let Some(actual_checks) = checks(&actual) else {
            return Ok(false);
        };
        let (required, forbidden) = if target {
            (&new_checks, &old_checks)
        } else {
            (&old_checks, &new_checks)
        };
        if required
            .difference(forbidden)
            .any(|c| !actual_checks.contains(c))
            || forbidden
                .difference(required)
                .any(|c| actual_checks.contains(c))
        {
            return Ok(false);
        }
        let expected_metadata = metadata(&reference, name)?;
        let actual_metadata = metadata(connection, name)?;
        let Some(actual_columns) = column_items(&actual) else {
            return Ok(false);
        };
        for column in changed_columns(old, &new).context("Invalid compiled source columns")? {
            if actual_metadata.get(&column) != expected_metadata.get(&column)
                || !actual_columns
                    .get(&column)
                    .is_some_and(|item| binary_comparison(item))
            {
                return Ok(false);
            }
        }
    }
    for name in MODIFIED_GUARDS {
        let old = source
            .guards
            .get(*name)
            .context("Missing compiled source guard")?;
        let new = extend_guard(name, old.clone())?;
        let Some(actual) = sql(connection, "trigger", name)? else {
            return Ok(false);
        };
        if !equivalent(&actual, if target { &new } else { old }) {
            return Ok(false);
        }
    }
    if target {
        reference.execute_batch(crate::delegation_settings::SCHEMA)?;
        let Some(actual) = sql(connection, "table", "delegation_settings")? else {
            return Ok(false);
        };
        let expected = sql(&reference, "table", "delegation_settings")?
            .context("Missing compiled settings schema")?;
        if !equivalent(&actual, &expected) || crate::delegation_settings::read(connection).is_err()
        {
            return Ok(false);
        }
        if !identity_schema_matches(connection)? {
            return Ok(false);
        }
    } else if !source_objects_absent(connection)? {
        return Ok(false);
    }
    Ok(true)
}
