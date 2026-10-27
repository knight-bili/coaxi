// Apache-2.0; 固定 Codeg aff1e6c 原输出类型，按客户确认 r1 移植。
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserTabsOutcome {
    pub tabs: Vec<AgentTabSummary>,
    /// Why the list is empty, when it is empty for a reason other than "the
    /// user has no browser tabs open". Without it an agent cannot tell "you
    /// have nothing open" from "this build has no built-in browser".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentTabSummary {
    pub tab_id: String,
    /// `None` for a tab that has committed no document yet, or one whose
    /// document has an opaque origin. Such a tab cannot be shared at all (see
    /// [`grantable_origin`]); it is listed anyway, because a page that is
    /// merely still loading would otherwise drop out of the listing and
    /// reappear a moment later.
    pub origin: Option<String>,
    pub level: GrantLevel,
    /// Present only from [`GrantLevel::Read`] upwards.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum GrantLevel {
    #[default]
    None,
    Read,
    Control,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserSnapshotOutcome {
    /// Echoed back so a refusal names the tab it is about — the agent quotes
    /// it to the user when asking them to share it.
    pub tab_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<PageSnapshot>,
    /// One of the `browser_*` slugs above. `None` exactly when `snapshot` is
    /// `Some`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// The refusal in words, for the agent to relay.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageSnapshot {
    /// The token a later ref must quote, opaque to everyone but the world
    /// that issued it: its own generation and the host's epoch, joined.
    pub generation: String,
    /// The address the world was at when it walked the page. Not necessarily
    /// the tab's `url`: a snapshot is taken at a moment, and the host checks
    /// this one against the grant rather than trusting what it last heard.
    pub url: String,
    pub title: String,
    pub viewport: SnapshotViewport,
    /// The aria tree, in Playwright's `ai` rendering.
    pub tree: String,
    pub refs_count: usize,
    /// The tree stops at `max_chars` rather than at the end of the page.
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotViewport {
    pub width: f64,
    pub height: f64,
    pub dpr: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserActOutcome {
    pub tab_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<ActionOutcome>,
    /// One of the `browser_*` slugs above. `None` exactly when `action` is
    /// `Some`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionOutcome {
    pub fidelity: Fidelity,
    /// Where the page was when the action was done. A click that navigates
    /// has not navigated yet by then; the agent takes a snapshot to see what
    /// came of it.
    pub url: String,
    /// Only for a key press that scrolled something.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scrolled: Option<ScrollReport>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Fidelity {
    /// Events dispatched by script in the isolated world.
    Synthetic,
    /// A real input event the platform delivered (WebView2's CDP `Input.*`).
    Trusted,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScrollReport {
    /// How far it moved. Negative upwards, `0` when it did not.
    pub by: f64,
    /// Where the box is now.
    pub top: f64,
    /// The furthest it can go: `top == max` is the end of it.
    pub max: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserConsoleOutcome {
    pub tab_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub console: Option<ConsoleReadout>,
    /// One of the `browser_*` slugs above. `None` exactly when `console` is
    /// `Some`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleReadout {
    /// Where the tab was when the lines were read.
    pub url: String,
    /// Oldest first.
    pub entries: Vec<ConsoleEntry>,
    /// Lines this document printed that are not in the ring any more, or
    /// never reached it: evicted by newer ones, or over the sender's budget.
    pub dropped: u64,
    /// The `since` to pass next time to see only what came after this read.
    pub next_since: u64,
    /// Whether lines matching the query remain beyond `limit`.
    pub more: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleEntry {
    /// Position in this tab's stream, increasing across clears. A reader
    /// that remembers the last one it saw asks for `since` it.
    pub seq: u64,
    /// Unix milliseconds, host clock, when the line arrived.
    pub at: i64,
    pub level: ConsoleLevel,
    pub source: ConsoleSource,
    pub text: String,
    /// The script that printed or threw it, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
    /// Printed by the top-level document rather than a frame in it.
    pub top: bool,
    /// The origin of the document that printed it — what a read is filtered
    /// by. Kept off the wire: the agent only ever sees lines from the origin
    /// it was granted, so it would say the same thing on every line.
    #[serde(skip)]
    pub origin: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConsoleLevel {
    Debug,
    Log,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConsoleSource {
    /// A `console.*` call.
    Console,
    /// An exception nothing caught.
    Exception,
    /// A promise rejection nothing handled.
    Rejection,
    /// A resource (`<img>`, `<script>`, `<link>`) that failed to load.
    Resource,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserCaptureOutcome {
    pub tab_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture: Option<CaptureOutcome>,
    /// One of the `browser_*` slugs above. `None` exactly when `capture` is
    /// `Some`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureOutcome {
    pub mime: String,
    /// The image, base64.
    pub data: String,
    /// Pixel size of the image as delivered.
    pub width: u32,
    pub height: u32,
    /// Where the page was when the pixels were taken.
    pub url: String,
    /// The part of the viewport the image shows, in CSS pixels — the whole
    /// viewport, or the element's visible box — so the agent can relate it
    /// to a snapshot's coordinates.
    pub region: CaptureRegion,
    /// Whether `region` is an element rather than the viewport.
    pub clipped: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureRegion {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserEvalOutcome {
    pub tab_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<EvalOutcome>,
    /// One of the `browser_*` slugs above. `None` exactly when `result` is
    /// `Some`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvalOutcome {
    /// What the value looked like: `"string"`, `"number"`, `"object"`,
    /// `"undefined"`, `"node"`, … As reported by the page, and useful rather
    /// than authoritative.
    pub kind: String,
    /// The value rendered as text, at most [`MAX_EVAL_VALUE_CHARS`].
    pub value: String,
    /// The render stopped at the cap.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub truncated: bool,
    /// Where the page was, read in coaxi's own world after the code ran — not
    /// from [`EvalAnswer`], which the snippet could have written itself.
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserTabOutcome {
    /// The tab this is about. Absent only when an open was refused before any
    /// tab existed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tab_id: Option<String>,
    /// The tab as it stands now. Absent for a close, and for every refusal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tab: Option<AgentTabSummary>,
    /// The tab is showing an error page instead of the address it was sent to,
    /// and which kind (`dns`, `tls`, `blocked`, `failed`).
    ///
    /// Not a refusal — the tab opened, and it is the agent's to retry or close
    /// — but without it the answer is "this tab has no address and is not
    /// shared with you", which reads like a permission problem and sends the
    /// agent to ask the user for something that would not help. A dev server
    /// that is not up yet is the ordinary shape of this.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub load_error: Option<String>,
    /// One of the `browser_*` slugs above. `None` exactly when the call did
    /// what it was asked to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputerAppsOutcome {
    pub apps: Vec<AgentAppSummary>,
    /// One of the slugs above when the list is empty for a reason.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentAppSummary {
    #[serde(flatten)]
    pub app: AgentAppRef,
    /// Whether it is the frontmost application.
    pub active: bool,
    /// What it is shared for as a whole application — every window of it,
    /// its menus and its own shortcuts — when the user shared it so.
    #[serde(default, skip_serializing_if = "not_shared")]
    pub level: GrantLevel,
    /// Why none of its windows can be shared, when that is so — coaxi itself,
    /// or an application on the blocklist.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentAppRef {
    /// The stable name of the application: its bundle identifier on macOS,
    /// its executable path elsewhere. What a blocklist entry matches.
    pub key: String,
    /// What the application calls itself, for a person to recognise.
    pub name: String,
    pub pid: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputerLaunchOutcome {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app: Option<AgentAppRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputerClipboardOutcome {
    /// What was read.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// The text was put on the clipboard.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub written: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputerWindowsOutcome {
    pub windows: Vec<AgentWindowSummary>,
    /// The entire screen, while the user shares it as a whole.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screen: Option<AgentScreen>,
    /// How actions reach the windows, as the person has it set now.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<InputPolicy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentWindowSummary {
    /// coaxi's own name for this window. Stable for as long as the window and
    /// the process that owns it are the same ones; a new process — even the
    /// same application relaunched — gets a new id.
    pub target_id: String,
    pub app: AgentAppRef,
    pub bounds: Rect,
    pub on_screen: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimized: Option<bool>,
    /// Its application is hidden (macOS ⌘H): off the screen as a whole,
    /// with nothing of the window's own changed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,
    pub level: GrantLevel,
    /// Shared with its whole application: its menus and its own shortcuts
    /// are in reach too.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub whole_app: bool,
    /// Shared with the entire screen: its application's menus, and the
    /// desktop's own shortcuts but locking and logging out, are in reach too.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub whole_screen: bool,
    /// Present only from [`GrantLevel::Read`] upwards.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Why this window cannot be shared, when that is so.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentScreen {
    pub target_id: String,
    pub level: GrantLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputPolicy {
    /// What an action gets when the agent does not ask (`delivery`).
    pub default: ActDelivery,
    /// Whether an agent may ask for the front.
    pub foreground_allowed: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActDelivery {
    /// The window is not brought to the front, and the person's own pointer
    /// and keyboard focus stay where they are.
    #[default]
    Background,
    /// The window is brought to the front for the one action, which then
    /// goes in as real input, and the window the person was in is brought
    /// back after it. Some applications take keys and typing no other way —
    /// on Windows, every one built on Chromium. Only where the person has
    /// allowed it in the settings.
    Foreground,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputerActOutcome {
    pub target_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<ActReport>,
    /// One of the slugs above. `None` exactly when `action` is `Some`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActReport {
    pub target_id: String,
    pub effect: ActEffect,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route: Option<ActRoute>,
    pub delivery: ActDelivery,
    /// For a key pressed more than once: how many presses went out.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presses: Option<u32>,
    /// For typing with `submit`: whether return was pressed after it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub submitted: Option<bool>,
    /// For typing with `submit` whose return was not pressed: why.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub submit_note: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActEffect {
    /// Read back from the window: the value changed, the selection moved.
    Confirmed,
    /// Only part of it happened (some of the text was typed).
    Partial,
    /// Delivered, and nothing could be read back to prove it landed. Not a
    /// failure and not a success: look at the window to know.
    Unverifiable,
    /// Delivered, and by every sign it did nothing.
    SuspectedNoop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActRoute {
    Accessibility,
    SyntheticEvents,
    GlobalInput,
    SystemApi,
    Dom,
    TrustedInput,
    /// A route this coaxi does not know by name.
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputerCaptureOutcome {
    pub target_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture: Option<WindowCapture>,
    /// `None` exactly when `capture` is `Some`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowCapture {
    pub target_id: String,
    /// Names this capture: `<grant epoch>.<read number>`. Coordinates read
    /// off this image mean something only in this image's pixel space, and
    /// the actions that arrive later will carry the generation of the image
    /// their coordinates came from.
    pub generation: String,
    /// `image/png`.
    pub mime: String,
    /// The image, base64.
    pub data: String,
    /// The image's size in pixels — the space coordinates read off it are in.
    pub width: u32,
    pub height: u32,
    /// Where the window was when it was captured, in desktop coordinates.
    pub window_bounds: Rect,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputerSnapshotOutcome {
    pub target_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<WindowSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowSnapshot {
    pub target_id: String,
    /// See [`WindowCapture::generation`].
    pub generation: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_bounds: Option<Rect>,
    /// The tree, as indented text, one element per line.
    pub tree: String,
    /// How many actionable elements the driver found, before any cut.
    pub element_count: u64,
    /// The tree was cut short, by `maxChars` or by the driver's own bounds.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub truncated: bool,
    /// Why the tree is empty or partial when the window is not (a canvas, a
    /// window whose accessibility surface the driver could not resolve), in
    /// the driver's words.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub degraded: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputerVerifyOutcome {
    pub target_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verify: Option<VerifyOutcome>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyOutcome {
    pub target_id: String,
    pub status: VerifyStatus,
    pub stable: bool,
    pub samples: u64,
    pub elapsed_ms: u64,
    pub predicates: Vec<PredicateResult>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerifyStatus {
    Satisfied,
    Unsatisfied,
    /// Could not be decided. Never a success: an agent that reads "unknown"
    /// as "probably fine" is exactly the failure this tool exists to prevent.
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PredicateResult {
    pub index: u32,
    pub status: VerifyStatus,
    /// Why a predicate is `unknown`, in the driver's vocabulary
    /// (`target_missing`, `stability_unproven`, …).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unknown_reason: Option<String>,
}

fn not_shared(level: &GrantLevel) -> bool {
    *level == GrantLevel::None
}
fn canonical<T: for<'de> Deserialize<'de> + Serialize>(value: Value) -> Result<Value> {
    Ok(serde_json::to_value(serde_json::from_value::<T>(value)?)?)
}
pub fn normalize(name: &str, value: Value) -> Result<Value> {
    match name {
        "browser_list_tabs" => canonical::<BrowserTabsOutcome>(value),
        "browser_open_tab" | "browser_navigate" | "browser_close_tab" => {
            canonical::<BrowserTabOutcome>(value)
        }
        "browser_snapshot" => canonical::<BrowserSnapshotOutcome>(value),
        "browser_console_messages" => canonical::<BrowserConsoleOutcome>(value),
        "browser_screenshot" => canonical::<BrowserCaptureOutcome>(value),
        "browser_eval" => canonical::<BrowserEvalOutcome>(value),
        "browser_click"
        | "browser_hover"
        | "browser_type"
        | "browser_press_key"
        | "browser_select_option" => canonical::<BrowserActOutcome>(value),
        "computer_list_apps" => canonical::<ComputerAppsOutcome>(value),
        "computer_list_windows" => canonical::<ComputerWindowsOutcome>(value),
        "computer_launch_app" => canonical::<ComputerLaunchOutcome>(value),
        "computer_screenshot" => canonical::<ComputerCaptureOutcome>(value),
        "computer_snapshot" => canonical::<ComputerSnapshotOutcome>(value),
        "computer_verify" => canonical::<ComputerVerifyOutcome>(value),
        "computer_clipboard_read" | "computer_clipboard_write" => {
            canonical::<ComputerClipboardOutcome>(value)
        }
        _ => canonical::<ComputerActOutcome>(value),
    }
}
