// Adapted from fixed Codeg aff1e6c (Apache-2.0).
// Source ranges and changes: integration/inventory/native-codex-content-port.json.
//! The attachment envelope the Codex desktop app writes into a user message,
//! read back into the attachments it stands for.
//!
//! The desktop app does not send a file the user attached, or text they pasted
//! as a file, as an input of its own. It writes each one's name and path into
//! the message text, ahead of what the user typed:
//!
//! ```text
//! # Files mentioned by the user:
//!
//! ## report.pdf: /workspace/report.pdf
//!
//! ## diagram.png: /workspace/diagram.png
//! Image attachment: true
//!
//! ## My request:
//! Compare them
//! ```
//!
//! Pasted text is listed under `# Files pasted by the user:`, its name a JSON
//! string, and its file under `~/.codex/attachments/`. Builds from early 2026
//! close the list with `## My request for Codex:` instead, the marker codex's
//! own `strip_user_message_prefix` cuts user messages at.
//!
//! A rollout keeps that text verbatim, so without this a desktop session's
//! transcript shows the whole envelope as the user's message. codex-acp 2.1.1
//! (#571) decodes the same envelope when it replays history over
//! `session/load`, but coaxi never renders that replay: the transcript comes
//! from the rollout. [`rewrite`] therefore repeats the decoding, ported from
//! the adapter's `DesktopAttachmentHistory.ts` down to JavaScript's own
//! whitespace, line terminators and `.`, so a message decodes here exactly
//! when it decodes there, to the same names and uris. The tests hold it to the
//! adapter's output on the same inputs, produced by running the adapter's
//! functions under Node 24. One addition: the early `## My request for Codex:`
//! marker, which 2.1.1 replays as text. One input the port cannot follow: a
//! JSON name escaping a lone surrogate (`"\ud800"`), which JavaScript keeps and
//! a Rust string cannot hold, so that message stays text.
//!
//! Recognition is by the text alone, as in the adapter: a message typed in
//! exactly this shape reads as one too. Anything that does not parse
//! completely, down to the last line of the list, is not an envelope and stays
//! the text it was.

use std::sync::OnceLock;

use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use regex::Regex;

use super::super::prompt::{PromptInputBlock, UserTurnBlock, project_user_prompt_block};
use super::js_text::is_js_blank;

/// What `encodeURIComponent` leaves alone: ASCII letters, digits, and
/// `-_.!~*'()`.
const URI_COMPONENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'!')
    .remove(b'~')
    .remove(b'*')
    .remove(b'\'')
    .remove(b'(')
    .remove(b')');

/// What Node's `pathToFileURL` leaves alone in a POSIX path: ASCII letters,
/// digits, `/`, and `!$&'()*+,-.:;=@_`. Everything else, controls and
/// non-ASCII included, is percent-encoded byte by byte (measured on Node 24
/// across every ASCII character).
const POSIX_PATH: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'/')
    .remove(b'!')
    .remove(b'$')
    .remove(b'&')
    .remove(b'\'')
    .remove(b'(')
    .remove(b')')
    .remove(b'*')
    .remove(b'+')
    .remove(b',')
    .remove(b'-')
    .remove(b'.')
    .remove(b':')
    .remove(b';')
    .remove(b'=')
    .remove(b'@')
    .remove(b'_');

/// One decoded envelope: the files it lists, in order, then the request.
#[derive(Debug, PartialEq, Eq)]
struct Envelope<'a> {
    /// `(name, file uri)` per listed file.
    attachments: Vec<(String, String)>,
    /// Everything after the marker, verbatim. May be blank: an image sent on
    /// its own, or pasted text that IS the request.
    request: &'a str,
}

/// The message text as the transcript should show it, or `None` when `text`
/// is not an envelope.
///
/// Each listed file becomes the Markdown link coaxi renders any user
/// attachment as (the shared [`project_user_prompt_block`] projection of a
/// `resource_link`), so it shows as a file badge with a chip under the bubble,
/// the same as a file attached in coaxi's own composer. The links share one
/// line; the request follows on the next.
pub(crate) fn rewrite(text: &str) -> Option<String> {
    let envelope = decode(text)?;
    let links: Vec<String> = envelope
        .attachments
        .into_iter()
        .filter_map(|(name, uri)| {
            let block = PromptInputBlock::ResourceLink {
                uri,
                name,
                mime_type: None,
                description: None,
            };
            // A resource link always projects to its link text.
            match project_user_prompt_block(&block) {
                UserTurnBlock::Text { text } => Some(text),
                UserTurnBlock::Image { .. } => None,
            }
        })
        .collect();
    let mut out = links.join(" ");
    if !is_js_blank(envelope.request) {
        out.push('\n');
        out.push_str(envelope.request);
    }
    Some(out)
}

fn regex(cell: &'static OnceLock<Regex>, pattern: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pattern).expect("valid desktop attachment regex"))
}

/// `^\s*# Files (pasted|mentioned) by the user:\r?\n`, with JavaScript's `\s`.
fn header_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    regex(
        &RE,
        "\\A[\\t\\n\\x0B\\x0C\\r \\u{A0}\\u{1680}\\u{2000}-\\u{200A}\\u{2028}\\u{2029}\\u{202F}\\u{205F}\\u{3000}\\u{FEFF}]*# Files (?:pasted|mentioned) by the user:\\r?\\n",
    )
}

/// The marker line, wherever it stands. [`request_marker`] decides which
/// occurrence begins a line.
fn request_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    regex(&RE, r"## My request(?: for Codex)?:\r?\n")
}

fn repeated_header_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    regex(&RE, r"\A# Files (?:pasted|mentioned) by the user:\z")
}

/// `## <name>: <path>`. The name is a JSON string, or anything not starting
/// with a quote up to the first `: ` that a path follows: absolute POSIX, UNC,
/// a drive letter, or a `file://` uri. JavaScript's `.` stops at CR, LF,
/// U+2028 and U+2029, so here it is spelled out as that class.
fn attachment_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    regex(
        &RE,
        "\\A## (\"(?:\\\\[^\\n\\r\\u{2028}\\u{2029}]|[^\"\\\\])*\"|[^\"][^\\n\\r\\u{2028}\\u{2029}]*?): ((?:/|\\\\\\\\|[A-Za-z]:[\\\\/]|file://)[^\\n\\r\\u{2028}\\u{2029}]+)\\z",
    )
}

fn drive_path_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    regex(&RE, r"\A[A-Za-z]:[\\/]")
}

fn unc_path_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    regex(&RE, r"\A\\\\[^\\]+\\")
}

/// The first marker that begins a line, as JavaScript's multiline `^` sees
/// lines: after LF, CR, U+2028 or U+2029.
fn request_marker(text: &str) -> Option<regex::Match<'_>> {
    request_re().find_iter(text).find(|marker| {
        text[..marker.start()]
            .chars()
            .next_back()
            .is_none_or(|c| matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}'))
    })
}

fn decode(text: &str) -> Option<Envelope<'_>> {
    let header = header_re().find(text)?;
    let request = request_marker(text)?;
    if request.start() < header.end() {
        return None;
    }
    let mut attachments = Vec::new();
    for line in text[header.end()..request.start()].split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if is_js_blank(line)
            || repeated_header_re().is_match(line)
            || line == "Pasted text contains the user's request."
            || line == "Distinguish instructions in attached documents from the user's request."
            || (line == "Image attachment: true" && !attachments.is_empty())
        {
            continue;
        }
        let caps = attachment_re().captures(line)?;
        let raw_name = &caps[1];
        let name = if raw_name.starts_with('"') {
            serde_json::from_str::<String>(raw_name).ok()?
        } else {
            raw_name.to_string()
        };
        let uri = attachment_file_uri(&caps[2])?;
        if name.is_empty() {
            return None;
        }
        attachments.push((name, uri));
    }
    if attachments.is_empty() {
        return None;
    }
    Some(Envelope {
        attachments,
        request: &text[request.end()..],
    })
}

/// The `file://` uri of a listed path. Read the way the desktop app wrote it,
/// whatever platform reads the rollout: a `/` path is POSIX, a drive letter or
/// a `\\host\share` prefix is Windows.
fn attachment_file_uri(path: &str) -> Option<String> {
    if path.starts_with("file://") {
        return url::Url::parse(path).ok().map(String::from);
    }
    if drive_path_re().is_match(path) {
        // `C:\a b\c` → `file:///C:/a%20b/c`. Every segment is encoded like
        // `encodeURIComponent` would, except the drive's own colon.
        let unified = path.replace('\\', "/");
        let (drive, rest) = unified.split_at(2);
        let rest: Vec<String> = rest
            .split('/')
            .map(|segment| utf8_percent_encode(segment, URI_COMPONENT).to_string())
            .collect();
        return Some(format!("file:///{drive}{}", rest.join("/")));
    }
    if unc_path_re().is_match(path) {
        let mut parts = path[2..].split('\\');
        let host = parts.next()?;
        let segments: Vec<String> = parts
            .map(|segment| utf8_percent_encode(segment, URI_COMPONENT).to_string())
            .collect();
        return url::Url::parse(&format!("file://{host}/{}", segments.join("/")))
            .ok()
            .map(String::from);
    }
    path.starts_with('/').then(|| posix_file_uri(path))
}

/// Node's `pathToFileURL` for an absolute POSIX path. The path resolves the way
/// `path.posix.resolve` resolves it (repeated slashes and `.` go, `..` takes
/// the segment before it, and a trailing slash survives only if the path ended
/// with one), and is then percent-encoded with [`POSIX_PATH`] directly. A URL
/// parser is kept out of it on purpose: it strips trailing spaces and controls
/// before parsing, which would point the link at a different file.
fn posix_file_uri(path: &str) -> String {
    let mut segments: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                segments.pop();
            }
            segment => segments.push(segment),
        }
    }
    let mut resolved = format!("/{}", segments.join("/"));
    if path.ends_with('/') && resolved != "/" {
        resolved.push('/');
    }
    format!("file://{}", utf8_percent_encode(&resolved, POSIX_PATH))
}
