// Adapted from fixed Codeg aff1e6c (Apache-2.0).
// Source ranges and changes: integration/inventory/native-grok-content-port.json.
use super::model::ContentBlock;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PromptInputBlock {
    Text {
        text: String,
    },
    Image {
        data: String,
        mime_type: String,
        #[serde(default)]
        uri: Option<String>,
    },
    Resource {
        uri: String,
        #[serde(default)]
        mime_type: Option<String>,
        #[serde(default)]
        text: Option<String>,
        #[serde(default)]
        blob: Option<String>,
    },
    ResourceLink {
        uri: String,
        name: String,
        #[serde(default)]
        mime_type: Option<String>,
        #[serde(default)]
        description: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum UserTurnBlock {
    Text {
        text: String,
    },
    Image {
        data: String,
        mime_type: String,
        uri: Option<String>,
    },
}
pub fn project_user_prompt_block(block: &PromptInputBlock) -> UserTurnBlock {
    match block {
        PromptInputBlock::Text { text } => UserTurnBlock::Text { text: text.clone() },
        PromptInputBlock::Image {
            data,
            mime_type,
            uri,
        } => UserTurnBlock::Image {
            data: data.clone(),
            mime_type: mime_type.clone(),
            uri: uri.clone(),
        },
        // An image-mime embedded resource carries a pasted image for agents
        // that reject native image blocks (an `image:false` +
        // `embedded_context:true` agent), and for a format the agent cannot
        // decode. Promote it to `Image` so viewers render the thumbnail;
        // non-image resources still collapse to a link.
        PromptInputBlock::Resource {
            uri,
            mime_type,
            blob,
            text,
        } => match (mime_type, blob, text) {
            (Some(mt), Some(b), _) if mt.starts_with("image/") => UserTurnBlock::Image {
                data: b.clone(),
                mime_type: mt.clone(),
                // A pasted image has no path; `""` would read as a filename of
                // nothing rather than as "unnamed".
                uri: (!uri.is_empty()).then(|| uri.clone()),
            },
            // An empty body is no body: the wire reader drops it, so a prompt
            // re-read from a transcript has to project the way it did live.
            (_, _, Some(body)) if !body.is_empty() && context_ref_is_readable(uri) => {
                UserTurnBlock::Text {
                    text: embedded_context_text(uri, body),
                }
            }
            _ => UserTurnBlock::Text {
                text: attachment_marker(uri, uri),
            },
        },
        PromptInputBlock::ResourceLink { uri, name, .. } => UserTurnBlock::Text {
            text: attachment_marker(name, uri),
        },
    }
}
fn attachment_marker(label: &str, uri: &str) -> String {
    format!(
        "[{}]({})",
        escape_markdown_text(label),
        escape_link_destination(uri)
    )
}

/// A text resource as the attachment it is shown as: the uri, then a
/// `<context>` block naming it on the lines below — the shape codex-acp writes
/// into its own record, and one the transcript already reads back into the
/// composer's badge with the uri listed under the message (`splitUserTextAndResources`
/// in `src/lib/adapters/ai-elements-adapter.ts`). A bare `[uri](uri)` link,
/// which is what this used to be, showed a viewer and a re-read custom-agent
/// transcript an address where the sender saw "Page screenshot".
///
/// What the block carries is only what names the badge. For a block the
/// built-in browser wrote, that is its facts — the lines before the first
/// fence, which is where the page's own markup and console output start, and
/// the only part of it that can span lines. Anything else is named after its
/// uri and carries nothing: a pasted file's body can run to megabytes, and this
/// projection is broadcast to every viewer and rebuilt on every history load.
fn embedded_context_text(uri: &str, body: &str) -> String {
    let facts = if body.starts_with(HANDOFF_BLOCK_HEADER) {
        let end = body
            .match_indices('\n')
            .map(|(at, _)| at + 1)
            .find(|&start| body[start..].starts_with("```"))
            .unwrap_or(body.len());
        // The page wrote parts of these lines too — its title, an element's
        // id — and the transcript reads a block only up to the next
        // `<context ref="` or line-leading `</context>` it meets. Either one
        // copied through would stop the attachment being read back as one.
        neutralize_context_tags(body[..end].trim_end())
    } else {
        String::new()
    };
    format!("{uri}\n<context ref=\"{uri}\">\n{facts}\n</context>")
}

/// `<context` and `</context` with the angle bracket swapped for a lookalike
/// (`‹`), so text quoted inside a `<context>` block can neither open nor close
/// one. Only ever read to name the badge, where a page that writes these into
/// its title or an id loses nothing it could have meant.
fn neutralize_context_tags(text: &str) -> String {
    text.replace("</context", "‹/context")
        .replace("<context", "‹context")
}

/// Whether the transcript can read `uri` back out of `<context ref="…">`: it
/// takes the ref up to the first quote or line break, and a block naming
/// nothing is left in the prose rather than lifted.
fn context_ref_is_readable(uri: &str) -> bool {
    !uri.is_empty() && !uri.contains(['"', '\r', '\n'])
}

/// Backslash-escape every inline-significant ASCII punctuation char, so a label
/// cannot inject Markdown structure (a nested link, emphasis, a code span…).
/// Mirrors `collapseNewlines` + `escapeMarkdownText` in
/// `src/components/chat/composer/reference-text.ts`: a newline run collapses to
/// one space first, because a marker has to stay a single inline token. GFM
/// does not autolink inside link text, so escaping alone is enough here.
fn escape_markdown_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        // `\s*[\r\n]+\s*` → " ": only a run that CONTAINS a line break
        // collapses, so ordinary spaces in a file name survive.
        if ch.is_whitespace() {
            let mut run = String::from(ch);
            while chars.peek().is_some_and(|c| c.is_whitespace()) {
                run.push(chars.next().expect("peeked"));
            }
            if run.contains(['\r', '\n']) {
                out.push(' ');
            } else {
                out.push_str(&run);
            }
            continue;
        }
        if matches!(
            ch,
            '\\' | '`' | '*' | '_' | '~' | '[' | ']' | '(' | ')' | '<' | '>'
        ) {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

/// Mirrors `escapeLinkDestination` in the same file: newlines are stripped, and
/// a destination containing whitespace, parentheses, angle brackets or a
/// backslash is wrapped in `<…>` with `\`, `<` and `>` escaped inside. Clean
/// URLs stay bare, so the overwhelmingly common case is byte-identical to what
/// this emitted before.
fn escape_link_destination(uri: &str) -> String {
    let cleaned: String = uri.chars().filter(|c| *c != '\r' && *c != '\n').collect();
    if !cleaned
        .chars()
        .any(|c| c.is_whitespace() || matches!(c, '(' | ')' | '<' | '>' | '\\'))
    {
        return cleaned;
    }
    let mut out = String::with_capacity(cleaned.len() + 2);
    out.push('<');
    for ch in cleaned.chars() {
        if matches!(ch, '\\' | '<' | '>') {
            out.push('\\');
        }
        out.push(ch);
    }
    out.push('>');
    out
}

pub fn prompt_block_from_wire(item: &serde_json::Value) -> Option<PromptInputBlock> {
    let string = |v: &serde_json::Value, key: &str| {
        v.get(key)
            .and_then(|x| x.as_str())
            .map(str::to_string)
            .filter(|s| !s.is_empty())
    };
    // NOT filtered for emptiness: `mime_type` is a plain `Option<String>` here
    // and the typed reader on the replay side keeps a `""` as `Some("")`, so
    // dropping it would make the two readers disagree on the same content.
    let mime = |v: &serde_json::Value| {
        v.get("mimeType")
            .or_else(|| v.get("mime_type"))
            .and_then(|m| m.as_str())
            .map(str::to_string)
    };
    match item.get("type").and_then(|t| t.as_str()) {
        Some("image") => Some(PromptInputBlock::Image {
            data: string(item, "data")?,
            // ACP requires `mimeType`; a record MISSING it is old enough that
            // png was the only thing codeg ever pasted. A record that carries
            // an empty one keeps it, which is what the typed reader sees.
            mime_type: mime(item).unwrap_or_else(|| "image/png".to_string()),
            uri: string(item, "uri"),
        }),
        Some("resource") => {
            let resource = item.get("resource")?;
            let uri = string(resource, "uri");
            let mime_type = mime(resource);
            let blob = string(resource, "blob");
            let is_image = mime_type
                .as_deref()
                .is_some_and(|m| m.starts_with("image/"));
            // Nothing to show: no uri to name it by, and no bytes to draw it
            // from. (Kept identical to `acp_native::prompt_block_from_content`,
            // the typed reader for the same content off the replay channel.)
            if uri.is_none() && !(is_image && blob.is_some()) {
                return None;
            }
            Some(PromptInputBlock::Resource {
                uri: uri.unwrap_or_default(),
                mime_type,
                text: string(resource, "text"),
                blob,
            })
        }
        Some("resource_link") => {
            let uri = string(item, "uri")?;
            Some(PromptInputBlock::ResourceLink {
                name: string(item, "name").unwrap_or_else(|| uri.clone()),
                uri,
                mime_type: mime(item),
                description: string(item, "description"),
            })
        }
        // `text`, and any kind this build does not know: a future block that
        // still carries a top-level `text` shows as that text, which is the ACP
        // guidance for unknown content.
        _ => Some(PromptInputBlock::Text {
            text: string(item, "text")?,
        }),
    }
}
pub fn user_turn_block(block: &PromptInputBlock) -> ContentBlock {
    match project_user_prompt_block(block) {
        UserTurnBlock::Text { text } => ContentBlock::Text { text },
        UserTurnBlock::Image {
            data,
            mime_type,
            uri,
        } => ContentBlock::Image {
            data,
            mime_type,
            uri,
        },
    }
}

const HANDOFF_BLOCK_HEADER: &str = "Captured from a web page in the built-in browser";
