// Apache-2.0; adapted from spacering-net/codeg aff1e6c278453ed8d0b96a5f664edcdca621aa3f.
//! The driver's accessibility tree, read line by line: which lines start an
//! element, which elements can be acted on, and which are secrets.
//!
//! The driver writes values unescaped, newlines included, so a node is not a
//! line: it runs from a line that starts one (see [`Dialect::head`]) to the
//! next, and a secret's value goes with every line of it.
//!
//! Secrets are judged once, here, for two uses: the value of a secret field
//! never leaves the helper, and nothing is ever typed into one. The two
//! cannot disagree — a field the agent sees as `[redacted]` is the field it
//! cannot type into.

use std::collections::BTreeSet;

/// Words that mark a field as a secret, in the languages codeg ships in.
const SECRET_WORDS: &[&str] = &[
    "password",
    "passwd",
    "passcode",
    "passphrase",
    "secret",
    "pin code",
    "密码",
    "密碼",
    "口令",
    "パスワード",
    "비밀번호",
    "contraseña",
    "mot de passe",
    "passwort",
    "senha",
    "كلمة المرور",
];

/// A tree with every secret's value taken out, and what is known of each
/// element in it that can be acted on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Redacted {
    pub tree: String,
    /// In tree order.
    pub nodes: Vec<TreeNode>,
}

/// One element that can be acted on: a node whose line carries `[N]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeNode {
    pub index: u32,
    /// Where its line starts in [`Redacted::tree`], in bytes.
    pub offset: u32,
    /// Its role as the tree spells it: `AXTextField`, `Edit`, `password text`.
    pub role: String,
    /// A password or other secret field.
    pub secret: bool,
}

/// Take the value out of every tree node that describes a secret, and name
/// the elements that can be acted on.
///
/// The platforms already refuse to hand a secure field's text to an
/// accessibility client — macOS answers bullets for a secure text field,
/// Windows refuses a password edit's value to other processes — so this is
/// the second line, not the first: a node whose role names a password
/// control, or whose label says it is one, or whose value is nothing but
/// bullets, keeps its role and label and loses its value. The driver's tree
/// does not carry macOS subroles, so a secure field is recognised here by its
/// words; recognising it by subrole needs the driver to report one.
pub fn redact_secrets(tree: &str) -> Redacted {
    redact_tree(tree, Dialect::current())
}

/// The roles of an application's menu bars in the macOS tree — its menus
/// and its status items — and of the items on them.
pub const APP_MENU_ROLES: &[&str] = &["AXMenuBar", "AXExtrasMenuBar", "AXMenuBarItem"];

/// How much of its application's menu bars a window's tree keeps (macOS).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMenus<'a> {
    /// None of them: a window shared on its own does not reach its
    /// application's menus.
    Withheld,
    /// The application's own menus, for an application shared as a whole —
    /// but not the Apple menu and the application menu, the first two items
    /// of its menu bar, named here by their titles as Accessibility gives
    /// them (a tree cut down by a query may not show either, and a row's
    /// place in it says nothing): they restart and log out, run Services and
    /// hide every other application, reaching past this one.
    Own { protected: &'a [String; 2] },
}

/// The tree without what `keep` leaves out of its application's menu bars,
/// and the indices of the elements that were in it (macOS).
///
/// The driver's tree of a window carries its application's menu bars along
/// with it. A menu bar's row goes with every line under it, up to the next
/// row no deeper than itself — and so does each item on it, row by row,
/// since the driver leaves out a menu bar's own row when it has nothing to
/// say, keeping its items at their depth. Elsewhere a window's menus are its
/// own, and the tree is left whole.
pub fn without_app_menus(
    tree: &str,
    dialect: Dialect,
    keep: AppMenus<'_>,
) -> (String, BTreeSet<u32>) {
    let mut withheld = BTreeSet::new();
    if dialect != Dialect::Mac {
        return (tree.to_string(), withheld);
    }
    let mut out = String::with_capacity(tree.len());
    // The depth of what is being left out, while something is.
    let mut leaving: Option<usize> = None;
    for line in tree.split_inclusive('\n') {
        if let Some(head) = dialect.head(line) {
            let depth = line.len() - line.trim_start_matches(' ').len();
            if leaving.is_some_and(|start| depth <= start) {
                leaving = None;
            }
            if leaving.is_none() {
                let leave = match (head.role, keep) {
                    ("AXMenuBar" | "AXExtrasMenuBar" | "AXMenuBarItem", AppMenus::Withheld) => true,
                    ("AXMenuBarItem", AppMenus::Own { protected }) => protected
                        .iter()
                        .any(|title| line.contains(&format!("AXMenuBarItem \"{title}\""))),
                    _ => false,
                };
                if leave {
                    leaving = Some(depth);
                }
            }
            if leaving.is_some() {
                withheld.extend(head.index);
            }
        }
        if leaving.is_none() {
            out.push_str(line);
        }
    }
    (out, withheld)
}

/// The shape of a node line in the driver's tree on each platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    /// `- [3] AXTextField "Title" = "value" (description) [attrs]`
    Mac,
    /// `- [3] Edit "Name" [value="…" id=… actions=[…]]`, and `- Text "Name" = "…"`
    Windows,
    /// `- [3] password text "name" value="…" [actions=[…]]`, and `- label = "name"`
    Linux,
}

/// What a node's first line says about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeHead<'a> {
    /// The `N` of `[N]`: only elements that can be acted on carry one.
    pub index: Option<u32>,
    pub role: &'a str,
}

impl Dialect {
    /// Where a node's value starts in this platform's tree: after ` = ` on
    /// every macOS node and on the other platforms' plain nodes; in
    /// `[value="…"` (Windows) or ` value="…"` (Linux) on their addressable
    /// ones. Only this platform's: another's marker in a title is just text.
    fn value_markers(self) -> &'static [&'static str] {
        match self {
            Dialect::Mac => &[" = \""],
            Dialect::Windows => &[" = \"", " [value=\""],
            Dialect::Linux => &[" = \"", " value=\""],
        }
    }

    /// What can only follow the quote that closes a value in this platform's
    /// tree — a description or an attribute block (macOS), the next attribute
    /// in the block the value sits in (Windows), the attribute block (Linux) —
    /// besides the end of the node.
    fn after_value(self) -> &'static [&'static str] {
        match self {
            Dialect::Mac => &["\" (", "\" ["],
            Dialect::Windows => &["\" id=", "\" help=", "\" actions=", "\"]"],
            Dialect::Linux => &["\" [actions="],
        }
    }

    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Dialect::Mac
        } else if cfg!(windows) {
            Dialect::Windows
        } else {
            Dialect::Linux
        }
    }

    /// Whether `line` starts a node rather than continuing the value of the
    /// one above it.
    pub fn starts_node(self, line: &str) -> bool {
        self.head(line).is_some()
    }

    /// The head of the node `line` starts, if it starts one: indentation in
    /// whole steps, `- `, an optional `[index] `, then a role as this
    /// platform's tree spells one — an `AX` role on macOS, one of UI
    /// Automation's control types on Windows, an AT-SPI role name followed by
    /// the quoted name every Linux node carries. A value's own lines are the
    /// user's text; the stricter this is, the less of that text can pass for
    /// a node and escape its node's redaction.
    pub fn head(self, line: &str) -> Option<NodeHead<'_>> {
        let line = line.trim_end_matches('\n');
        let rest = line.trim_start_matches(' ');
        if !(line.len() - rest.len()).is_multiple_of(2) {
            return None;
        }
        let mut rest = rest.strip_prefix("- ")?;
        let mut index = None;
        if let Some(inner) = rest.strip_prefix('[') {
            let close = inner.find("] ")?;
            if close == 0 || !inner[..close].bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            index = Some(inner[..close].parse::<u32>().ok()?);
            rest = &inner[close + 2..];
        }
        // What may follow a role in a node line: nothing, a quoted title or
        // name, a value, a description, an attribute block.
        let follows = |after: &str, allowed: &[&str]| {
            after.is_empty() || allowed.iter().any(|a| after.starts_with(a))
        };
        let role = match self {
            Dialect::Mac => {
                let end = rest
                    .bytes()
                    .position(|b| !b.is_ascii_alphanumeric())
                    .unwrap_or(rest.len());
                (rest.starts_with("AX")
                    && end > 2
                    && follows(&rest[end..], &[" \"", " = \"", " (", " ["]))
                    .then(|| &rest[..end])
            }
            Dialect::Windows => {
                let end = rest
                    .bytes()
                    .position(|b| !b.is_ascii_alphabetic())
                    .unwrap_or(rest.len());
                (UIA_CONTROL_TYPES.contains(&&rest[..end])
                    && follows(&rest[end..], &[" \"", " = \"", " ["]))
                    .then(|| &rest[..end])
            }
            Dialect::Linux => {
                // `- [3] push button "name" …` and `- label = "name"`: an
                // AT-SPI node always carries its name, quoted.
                let end = rest
                    .bytes()
                    .position(|b| !(b.is_ascii_lowercase() || b == b' '))
                    .unwrap_or(rest.len());
                let role = rest[..end].trim_end();
                let after = &rest[role.len()..];
                (!role.is_empty()
                    && after.starts_with(if index.is_some() { " \"" } else { " = \"" }))
                .then_some(role)
            }
        }?;
        Some(NodeHead { index, role })
    }
}

/// UI Automation's control types, as cua-driver names them in its Windows
/// tree.
const UIA_CONTROL_TYPES: &[&str] = &[
    "AppBar", "Button", "Calendar", "CheckBox", "ComboBox", "Custom", "DataGrid", "DataItem",
    "Document", "Edit", "Group", "Header", "HeaderItem", "Hyperlink", "Image", "List",
    "ListItem", "Menu", "MenuBar", "MenuItem", "Pane", "ProgressBar", "RadioButton",
    "ScrollBar", "SemanticZoom", "Separator", "Slider", "Spinner", "SplitButton", "StatusBar",
    "Tab", "TabItem", "Table", "Text", "Thumb", "TitleBar", "ToolBar", "ToolTip", "Tree",
    "TreeItem", "Unknown", "Window",
];

pub fn redact_tree(tree: &str, dialect: Dialect) -> Redacted {
    let mut out = Redacted {
        tree: String::with_capacity(tree.len()),
        nodes: Vec::new(),
    };
    let mut node = String::new();
    for line in tree.split_inclusive('\n') {
        if !node.is_empty() && dialect.starts_node(line) {
            push_node(&mut out, &node, dialect);
            node.clear();
        }
        node.push_str(line);
    }
    push_node(&mut out, &node, dialect);
    out
}

fn push_node(out: &mut Redacted, node: &str, dialect: Dialect) {
    let offset = u32::try_from(out.tree.len()).unwrap_or(u32::MAX);
    let (text, secret) = redact_node(node, dialect);
    let first_line = node.split_inclusive('\n').next().unwrap_or("");
    if let Some(NodeHead {
        index: Some(index),
        role,
    }) = dialect.head(first_line)
    {
        out.nodes.push(TreeNode {
            index,
            offset,
            role: role.to_string(),
            secret,
        });
    }
    out.tree.push_str(&text);
}

/// One node, redacted if it is a secret, and whether it is.
fn redact_node(node: &str, dialect: Dialect) -> (String, bool) {
    // The earliest marker: what is kept of a redacted node ends there, so it
    // can never hold any of the value.
    let marker = dialect
        .value_markers()
        .iter()
        .filter_map(|m| node.find(m).map(|i| (i, *m)))
        .min_by_key(|(i, _)| *i);
    // Judged on what names the node — its role and title before the value,
    // its description and attributes after it — never on the value itself: a
    // document that mentions a password is not a password field, and a
    // field's secret is not what says it is one. The one exception is a value
    // that is nothing but bullets, which is how a secure field shows its
    // contents.
    let (label, value) = match marker {
        Some((start, m)) => {
            let from = start + m.len();
            let after = after_value(node, from, dialect).max(from);
            (
                format!("{}{}", &node[..start], &node[after..]),
                Some(&node[from..after]),
            )
        }
        None => (node.to_string(), None),
    };
    let masked = value.is_some_and(is_masked);
    let secret = names_a_secret(&label) || masked;
    let Some((start, _)) = marker.filter(|_| secret) else {
        return (node.to_string(), secret);
    };
    // Everything from the value on goes, not just the value: the driver writes
    // values unescaped, so a value can contain `" (` or `" [` or a newline and
    // there is no telling where it ends. What stays — the index, the role and
    // the title — is what names the field.
    let newline = if node.ends_with('\n') { "\n" } else { "" };
    (
        format!(
            "{} = \"[redacted]\"{newline}",
            node[..start].trim_end_matches([' ', '['])
        ),
        true,
    )
}

/// Whether `text` — a node's role and label, never its value — says it is a
/// password or other secret field.
pub fn names_a_secret(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("securetextfield")
        || lower.contains("password text")
        || SECRET_WORDS.iter().any(|w| lower.contains(w))
}

/// A value that is nothing but the bullets a secure field shows in place of
/// its text.
pub fn is_masked(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty() && value.chars().all(|c| matches!(c, '•' | '●' | '∙' | '⦁'))
}

/// Where the text after a node's value begins, as near as can be told: the
/// driver does not escape the quote that closes a value, so this is the
/// earliest quote on the node's last line (a value ends on the line its node
/// does) that is followed by what only comes after one in this platform's
/// tree, or the node's closing quote. Earlier is the safe side — more of the
/// node is read as label.
fn after_value(node: &str, value_from: usize, dialect: Dialect) -> usize {
    let body = node.trim_end_matches('\n');
    let last_line = body.rfind('\n').map_or(0, |i| i + 1);
    let from = last_line.max(value_from).min(body.len());
    let tail = &body[from..];
    dialect
        .after_value()
        .iter()
        .filter_map(|m| tail.find(m))
        .chain(tail.ends_with('"').then(|| tail.len() - 1))
        .min()
        .map_or(body.len(), |i| from + i)
}
