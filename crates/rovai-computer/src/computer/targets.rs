// Apache-2.0; adapted from spacering-net/codeg aff1e6c278453ed8d0b96a5f664edcdca621aa3f.
//! codeg's table of the windows it has named to an agent, with the grant on
//! each.
//!
//! A browser tab is an object codeg owns, so its grant can live on the tab. A
//! native window is not — it belongs to another process and codeg only ever
//! sees it through the helper — so this table is the object the grant lives
//! on: one entry per window codeg has handed out a `targetId` for, keyed by
//! the window's identity, with the grant, the grant epoch and the read counter
//! under the same lock. "Is this still the window that was shared" and "is it
//! still shared" are then one question asked in one place.
//!
//! **Identity is `(pid, process start time, window id)`.** A pid alone is
//! reused; an application relaunched is a new process whose windows were never
//! shared, even when they look the same. A window whose identity no longer
//! turns up in a listing is gone, and so is its grant. Where another process
//! draws what is inside a window — a packaged application inside the frame
//! Windows draws for it — that process's run is part of the identity too:
//! the frame is that run's window, and another run of it in the same frame is
//! another window.
//!
//! **An application can be shared as a whole** ([`AppIdentity`]): every window
//! of it then carries the application's grant ([`GrantScope::App`]) — the
//! ones it opens later too, as listings find them — and the application's
//! one clock, which any of them being used keeps running. Ending the
//! application's grant ends every window's share of it; a window's share
//! cannot be changed on its own while the application is shared.
//!
//! **So can the entire screen** ([`ScreenShare`]), the same way one level up:
//! every window the rules allow carries the screen's grant
//! ([`GrantScope::Screen`]) and its clock, and the screen itself is a target
//! of its own ([`SCREEN_TARGET_ID`]) — one picture of it, and pointer actions
//! at points on it. Sharing the screen takes over whatever was shared before
//! it; nothing else is shared or unshared while it is.

use std::collections::{BTreeSet, HashMap};
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::agent::{
    generation, grantable, level_of, visible_title, Blocklist, ComputerGrant, ComputerGrantPayload,
    GrantChange, GrantLevel, GrantScope, NotGrantable, SelfIdentity,
};
use super::keys::{classify, classify_for_app, classify_for_screen, Chord, ChordClass, Platform};
use super::protocol::{
    DriverTarget, ElementRef, ProcessRun, RawAct, RawApp, RawWindow, ScreenGeometry, WindowAction,
    WindowPoint,
};
use super::types::{
    AgentAppRef, AgentTarget, AgentWindowSummary, ComputerActRequest, ElementTarget, PointTarget,
    Rect,
};

/// Which window, exactly. See the module note.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowIdentity {
    pub pid: u32,
    /// `None` when the platform would not say; such a window is still
    /// listed, and matched on the other two fields alone.
    pub started_at: Option<u64>,
    pub window_id: u64,
    /// The run of the process drawing inside the window, where that is not
    /// its owner (`RawWindow::content`). See the module note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<ProcessRun>,
}

impl WindowIdentity {
    fn of(window: &RawWindow) -> Self {
        Self {
            pid: window.pid,
            started_at: window.app.started_at,
            window_id: window.window_id,
            content: window.content,
        }
    }
}

/// One window codeg has named.
#[derive(Debug, Clone, PartialEq)]
pub struct TargetEntry {
    pub target_id: String,
    pub identity: WindowIdentity,
    pub app: RawApp,
    /// The title as last seen. Handed to an agent only through
    /// [`visible_title`]; shown to the person in codeg's own UI.
    pub title: String,
    pub bounds: Rect,
    pub on_screen: bool,
    pub minimized: Option<bool>,
    /// macOS: its application is hidden (⌘H).
    pub hidden: Option<bool>,
    pub on_current_space: Option<bool>,
    pub grant: Option<ComputerGrant>,
    /// Moves on every transition into or out of a grant, so a generation
    /// minted under one grant never names a read made under another.
    pub epoch: u64,
    /// Reads completed under the current epoch.
    pub reads: u64,
    /// The latest snapshot read under the current grant: what a ref is
    /// resolved against.
    pub snapshot_mark: Option<SnapshotMark>,
    /// The latest screenshot read under the current grant: what a point is
    /// read in.
    pub capture_mark: Option<CaptureMark>,
    /// The window stopped turning up while it was shared. Kept, grant-less,
    /// so a later call on its id is told "not shared" — the same answer as a
    /// window nobody shared, as the browser answers for a tab that navigated
    /// away — rather than "no such window", which would make an id that was
    /// once valid look like one the agent invented.
    pub gone: bool,
}

impl TargetEntry {
    /// Whether this window belongs in a listing: on screen, minimized, hidden
    /// with its application, on another Space, or shared. What that leaves out is the invisible
    /// furniture every desktop is full of — an application's hidden helper
    /// windows, the Finder's off-screen desktop strips — which nobody means to
    /// share and a picker full of would hide the ones they do. A shared window
    /// stays listed whatever its visibility (its application may just be
    /// hidden), so that its sharing is never out of sight.
    pub fn worth_listing(&self) -> bool {
        self.on_screen
            || self.minimized == Some(true)
            || self.hidden == Some(true)
            || self.on_current_space == Some(false)
            || self.grant.is_some()
    }

    /// The window as an agent may see it.
    pub fn agent_summary(&self, me: &SelfIdentity, blocklist: &Blocklist) -> AgentWindowSummary {
        let level = level_of(self.grant.as_ref());
        AgentWindowSummary {
            target_id: self.target_id.clone(),
            app: AgentAppRef {
                key: self.app.key().unwrap_or_default().to_string(),
                name: self.app.name.clone(),
                pid: self.app.pid,
            },
            bounds: self.bounds,
            on_screen: self.on_screen,
            minimized: self.minimized,
            hidden: self.hidden,
            level,
            whole_app: self
                .grant
                .as_ref()
                .is_some_and(|g| g.scope == GrantScope::App),
            whole_screen: self
                .grant
                .as_ref()
                .is_some_and(|g| g.scope == GrantScope::Screen),
            title: visible_title(level, &self.title),
            note: grantable(&self.app, me, blocklist)
                .err()
                .map(|why| why.note().to_string()),
        }
    }
}

/// A shared window, for codeg's own UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedWindow {
    pub target_id: String,
    pub app_name: String,
    pub app_key: String,
    pub title: String,
    pub level: GrantLevel,
    pub granted_at: i64,
    pub last_used_at: i64,
    /// Shared with its whole application ([`SharedApp`]), not on its own.
    #[serde(default)]
    pub whole_app: bool,
    /// The share of that application, when it is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
    /// Shared with the entire screen ([`SharedScreen`]).
    #[serde(default)]
    pub whole_screen: bool,
}

/// Which application, exactly: the run of the process that owns its
/// windows, which application that is (a frame host is several), and the run
/// drawing inside its frames where another process does. An application
/// relaunched is another one, never shared.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AppIdentity {
    pub pid: u32,
    pub started_at: u64,
    pub key: String,
    pub content: Option<ProcessRun>,
}

impl AppIdentity {
    /// The application `entry` is a window of; `None` for one that cannot be
    /// told (see [`NotGrantable::Unidentified`]).
    pub fn of(entry: &TargetEntry) -> Option<Self> {
        Some(Self {
            pid: entry.identity.pid,
            started_at: entry.identity.started_at?,
            key: entry.app.key()?.to_string(),
            content: entry.identity.content,
        })
    }

    /// Whether `app`, as a listing of applications names it, is this one.
    fn names(&self, app: &RawApp) -> bool {
        self.content.is_none()
            && app.pid == self.pid
            && app.started_at == Some(self.started_at)
            && app.key() == Some(self.key.as_str())
    }
}

/// An application shared as a whole.
#[derive(Debug, Clone, PartialEq)]
pub struct AppShare {
    /// codeg's own name for the share, for the panel to change or end it by.
    pub app_id: String,
    /// Its clock is the application's: any window of it being used moves it.
    pub grant: ComputerGrant,
    pub app: RawApp,
}

/// An application shared as a whole, for codeg's own UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedApp {
    pub app_id: String,
    pub app_name: String,
    pub app_key: String,
    pub level: GrantLevel,
    pub granted_at: i64,
    pub last_used_at: i64,
    /// How many of its windows are shared with it now.
    pub windows: u32,
}

/// The id the entire screen goes by among targets: what an agent reads and
/// points at while the person shares the screen as a whole.
pub const SCREEN_TARGET_ID: &str = "d1";

/// The entire screen, shared as a whole.
#[derive(Debug, Clone)]
pub struct ScreenShare {
    /// Its clock is the screen's: the screen, or any window shared with it,
    /// being used moves it.
    pub grant: ComputerGrant,
    /// See [`TargetEntry::epoch`]: a new one for every sharing of the
    /// screen, so a picture read under one never names a point under the
    /// next.
    pub epoch: u64,
    /// Pictures of the screen read under this sharing.
    pub reads: u64,
    /// The latest picture of the screen read under it: what a point on the
    /// screen is read in.
    pub capture_mark: Option<CaptureMark>,
    /// The rules as they stood when the sharing last followed them — shared,
    /// or swept: what a window a later listing finds is judged by before it
    /// takes the screen's grant.
    me: SelfIdentity,
    blocklist: Blocklist,
}

/// The entire screen shared, for codeg's own UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedScreen {
    pub level: GrantLevel,
    pub granted_at: i64,
    pub last_used_at: i64,
    /// How many windows are shared with it now.
    pub windows: u32,
}

/// Permission for one read of the entire screen, taken before the read and
/// checked again after.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenReadTicket {
    pub epoch: u64,
}

/// Permission for one action on the entire screen, with the action as the
/// helper is to carry it out: every point resolved against the latest
/// picture of the screen, as the agent was given it.
#[derive(Debug, Clone, PartialEq)]
pub struct ScreenActTicket {
    /// The sharing the action was let through under.
    pub epoch: u64,
    pub action: WindowAction,
    /// The screen as the picture the points were read off was taken.
    pub geometry: ScreenGeometry,
}

/// The latest snapshot an agent read of a window: the generation that named
/// it, and which of its elements the agent may now act on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotMark {
    pub generation: String,
    /// The driver's id for the snapshot; `None` when it kept none, and
    /// nothing in the tree can be acted on.
    pub snapshot_id: Option<String>,
    /// The refs whose lines the agent was given.
    pub shown: BTreeSet<u32>,
    /// The refs the tree had and the agent's copy was cut short of
    /// (`maxChars`): told apart from refs that never were, so the refusal can
    /// say which.
    pub cut: BTreeSet<u32>,
    /// The refs of secret fields.
    pub secret: BTreeSet<u32>,
}

/// The latest screenshot an agent read of a window: the generation that
/// named it, and the geometry a point read off it is mapped back through.
#[derive(Debug, Clone, PartialEq)]
pub struct CaptureMark {
    pub generation: String,
    /// The image as the agent got it.
    pub width: u32,
    pub height: u32,
    /// The window's own pixels, which the image was shrunk from.
    pub native_width: u32,
    pub native_height: u32,
    /// Whether the native size is known to be the window's full size (see
    /// `RawCapture::full_size`). Points need it.
    pub full_size: bool,
    pub window_bounds: Rect,
}

/// What a read leaves behind for later actions, before it has a generation.
#[derive(Debug, Clone, PartialEq)]
pub enum ReadMark {
    Snapshot {
        snapshot_id: Option<String>,
        shown: BTreeSet<u32>,
        cut: BTreeSet<u32>,
        secret: BTreeSet<u32>,
    },
    Capture {
        width: u32,
        height: u32,
        native_width: u32,
        native_height: u32,
        full_size: bool,
        window_bounds: Rect,
    },
}

/// Why a read may not go ahead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadRefusal {
    /// codeg never named a window by this id.
    NoSuchTarget,
    /// The window is not shared — never was, no longer is, or has gone.
    GrantRequired,
    /// The window can never be shared.
    NotGrantable(NotGrantable),
}

/// Permission for one read, taken before the read and checked again after.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadTicket {
    pub target_id: String,
    pub identity: WindowIdentity,
    pub epoch: u64,
    pub app: RawApp,
    pub bounds: Rect,
    /// Shared on its own, or with its application — whose menus a read then
    /// takes in too.
    pub scope: GrantScope,
}

/// Why an action may not go ahead, before anything is sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActDenied {
    /// codeg never named a window by this id.
    NoSuchTarget,
    /// Not shared — never was, no longer is, or gone.
    GrantRequired,
    /// Shared for reading only.
    ControlRequired,
    /// The window can never be shared.
    NotGrantable(NotGrantable),
    /// A ref or point that is not from the window's latest snapshot or
    /// screenshot, or not in it.
    Stale(Staleness),
    /// A point outside the image it was read off.
    OutOfImage,
    /// Text into a secret field.
    Secret,
    /// A key a window grant does not reach — it acts on the application or
    /// the desktop.
    ChordBeyond,
    /// A paste: the clipboard is the user's, and its source is not tracked.
    Paste,
    /// A character key with no element named to type it into.
    NeedsElement,
    /// The screenshot the point came from cannot be mapped back to the
    /// window's pixels.
    NoPointing,
    /// Keys held over a drag where the driver would drag without them.
    DragModifiers,
    /// Keys held over a double-click in a window, which the driver's
    /// double-click does not hold (macOS and Linux refuse them, Windows
    /// double-clicks without them).
    DoubleClickModifiers,
    /// A key that is the desktop's own, which not even a grant on the whole
    /// application reaches.
    DesktopChord,
    /// Something only an application shared as a whole allows — its menus.
    AppGrantRequired,
    /// A menu command on a system whose driver cannot choose one (Windows).
    MenusUnavailable,
    /// A frame no window can have: a side under the smallest, or a number
    /// out of range.
    BadFrame,
    /// A key that locks the screen or logs out, which no sharing reaches —
    /// not even the entire screen's.
    SessionChord,
    /// Asked of the entire screen, something other than a click, a drag or a
    /// scroll at a point of its picture: keys, typing and the rest go to a
    /// window.
    ScreenPointerOnly,
}

/// How a ref or point is out of date.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Staleness {
    /// No snapshot has been read under the current sharing.
    NoSnapshot,
    /// The generation is not the latest snapshot's.
    OldSnapshot,
    /// The driver kept no snapshot of the window: nothing in the tree can be
    /// acted on.
    NotActionable,
    /// The ref was in the tree, past where the agent's copy was cut.
    CutAway(u32),
    /// The latest snapshot has no such ref.
    NoSuchRef(u32),
    /// No screenshot has been read under the current sharing.
    NoCapture,
    /// The generation is not the latest screenshot's.
    OldCapture,
}

/// Permission for one action, with the action as the helper is to carry it
/// out: every ref and point resolved against what the agent last read.
#[derive(Debug, Clone, PartialEq)]
pub struct ActTicket {
    pub target_id: String,
    pub identity: WindowIdentity,
    /// The sharing the action was let through under (see
    /// [`TargetEntry::epoch`]): a key pressed again and again is held to the
    /// sharing its first press went out under.
    pub epoch: u64,
    pub app: RawApp,
    pub action: WindowAction,
    pub aim: Aim,
}

/// Where on the screen an action lands, as far as codeg can place it once
/// the helper says where it aimed. For the marker that shows the person
/// where an agent acted; nothing is decided by it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Aim {
    /// Wherever the window's focus is: a key with no element, a scroll with
    /// no target.
    Focus,
    /// At the element, where its snapshot found it.
    Element,
    /// This far from the window's top-left corner, in desktop units.
    Offset { x: f64, y: f64 },
}

impl Aim {
    /// Where `action` lands, from what the agent last read of the window:
    /// for a point, the screenshot's pixels scaled to the window's units.
    fn of(entry: &TargetEntry, action: &WindowAction) -> Aim {
        if action.element().is_some() {
            return Aim::Element;
        }
        match (action.point(), entry.capture_mark.as_ref()) {
            (Some(point), Some(mark)) if mark.native_width > 0 && mark.native_height > 0 => {
                Aim::Offset {
                    x: point.x * point.window_width / f64::from(mark.native_width),
                    y: point.y * point.window_height / f64::from(mark.native_height),
                }
            }
            _ => Aim::Focus,
        }
    }

    /// The point on the screen, in desktop units, from what the helper
    /// reported of the action: the middle of the element's frame, or the
    /// offset from where the window was measured to be.
    pub fn landing(&self, act: &RawAct) -> Option<(f64, f64)> {
        let placed = |r: &Rect| !r.is_empty() && r.x.is_finite() && r.y.is_finite();
        match *self {
            Aim::Focus => None,
            Aim::Element => act
                .element_frame
                .filter(placed)
                .map(|f| (f.x + f.width / 2.0, f.y + f.height / 2.0)),
            Aim::Offset { x, y } => act.window_frame.filter(placed).map(|w| (w.x + x, w.y + y)),
        }
    }
}

/// Why a share did not happen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShareError {
    NoSuchTarget,
    Gone,
    NotGrantable(NotGrantable),
    /// The window is shared with its whole application: what it is shared
    /// for is the application's, and changes with it.
    AppShared,
    /// The entire screen is shared: what any window or application is shared
    /// for is the screen's, and changes with it.
    ScreenShared,
}

/// What sharing an application or the entire screen, or ending its share,
/// changed: the windows whose share moved with it, and whether a share beyond
/// the windows' own — an application's, or the screen's — did.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AppChange {
    pub windows: Vec<ComputerGrantPayload>,
    pub app_changed: bool,
}

#[derive(Default)]
struct Inner {
    next_id: u64,
    entries: HashMap<String, TargetEntry>,
    by_identity: HashMap<WindowIdentity, String>,
    next_app_id: u64,
    apps: HashMap<AppIdentity, AppShare>,
    screen: Option<ScreenShare>,
    /// Sharings of the screen so far: each one's epoch.
    screen_epochs: u64,
}

/// See the module note.
#[derive(Default)]
pub struct TargetTable {
    inner: Mutex<Inner>,
}

impl TargetTable {
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        // A panic while holding this lock cannot leave a half-written entry
        // (every mutation is a field store), so a poisoned lock is still a
        // consistent table.
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Take in a fresh listing: name every window in it, update what codeg
    /// knows about the ones it had named, and let go of the rest.
    ///
    /// `scope_pid` is the filter the listing was made with. Only windows in
    /// that scope can be judged missing from it — a listing of one
    /// application's windows says nothing about anyone else's.
    ///
    /// Returns the entries for the listed windows, in listing order, and the
    /// grants that changed: ended because their window is gone, or begun
    /// because the window's application is shared as a whole.
    pub fn observe(
        &self,
        windows: &[RawWindow],
        scope_pid: Option<u32>,
    ) -> (Vec<TargetEntry>, Vec<ComputerGrantPayload>) {
        let mut inner = self.lock();
        let mut seen: Vec<String> = Vec::with_capacity(windows.len());
        for window in windows {
            let identity = WindowIdentity::of(window);
            let target_id = match inner.by_identity.get(&identity) {
                Some(id) => id.clone(),
                None => {
                    inner.next_id += 1;
                    let id = format!("w{}", inner.next_id);
                    inner.by_identity.insert(identity, id.clone());
                    inner.entries.insert(
                        id.clone(),
                        TargetEntry {
                            target_id: id.clone(),
                            identity,
                            app: window.app.clone(),
                            title: String::new(),
                            bounds: Rect::default(),
                            on_screen: false,
                            minimized: None,
                            hidden: None,
                            on_current_space: None,
                            grant: None,
                            epoch: 0,
                            reads: 0,
                            snapshot_mark: None,
                            capture_mark: None,
                            gone: false,
                        },
                    );
                    id
                }
            };
            if let Some(entry) = inner.entries.get_mut(&target_id) {
                entry.app = window.app.clone();
                // An empty title is the platform withholding it (no Screen
                // Recording yet), not the window losing its name: keep the
                // last one the person could have seen.
                if !window.title.is_empty() {
                    entry.title = window.title.clone();
                }
                entry.bounds = window.bounds;
                entry.on_screen = window.on_screen;
                entry.minimized = window.minimized;
                entry.hidden = window.hidden;
                entry.on_current_space = window.on_current_space;
            }
            seen.push(target_id);
        }

        let in_scope = |entry: &TargetEntry| scope_pid.is_none_or(|pid| entry.identity.pid == pid);
        let missing: Vec<String> = inner
            .entries
            .values()
            .filter(|e| !e.gone && in_scope(e) && !seen.contains(&e.target_id))
            .map(|e| e.target_id.clone())
            .collect();
        let mut ended = Vec::new();
        for id in missing {
            if let Some(payload) = Self::retire(&mut inner, &id, GrantChange::TargetChanged) {
                ended.push(payload);
            }
        }
        // A window of an application shared as a whole is shared with it as
        // soon as a listing finds it a window a person could mean; with the
        // entire screen shared, so is any window the rules allow.
        {
            let Inner {
                entries,
                apps,
                screen,
                ..
            } = &mut *inner;
            for id in &seen {
                let Some(entry) = entries.get_mut(id) else {
                    continue;
                };
                if entry.grant.is_some() || entry.gone || !entry.worth_listing() {
                    continue;
                }
                if let Some(share) = AppIdentity::of(entry).and_then(|app| apps.get(&app)) {
                    ended.extend(Self::grant_with(entry, &share.grant));
                } else if let Some(share) = screen.as_ref() {
                    if grantable(&entry.app, &share.me, &share.blocklist).is_ok() {
                        ended.extend(Self::grant_with(entry, &share.grant));
                    }
                }
            }
        }

        let entries = seen
            .iter()
            .filter_map(|id| inner.entries.get(id).cloned())
            .collect();
        (entries, ended)
    }

    /// A window is gone. A shared one keeps a grant-less entry (see
    /// [`TargetEntry::gone`]); an unshared one is forgotten.
    ///
    /// Idempotent, and it touches only what is this entry's own: a late
    /// "window gone" for an id that has already been retired — an operation
    /// that started before a listing retired it — leaves the entry, and the
    /// identity now named by a newer id, alone.
    fn retire(
        inner: &mut Inner,
        target_id: &str,
        change: GrantChange,
    ) -> Option<ComputerGrantPayload> {
        let entry = inner.entries.get_mut(target_id)?;
        if entry.gone {
            return None;
        }
        let identity = entry.identity;
        if inner.by_identity.get(&identity).map(String::as_str) == Some(target_id) {
            inner.by_identity.remove(&identity);
        }
        let entry = inner.entries.get_mut(target_id)?;
        if entry.grant.is_some() {
            entry.grant = None;
            entry.gone = true;
            entry.epoch += 1;
            entry.snapshot_mark = None;
            entry.capture_mark = None;
            Some(ComputerGrantPayload {
                target_id: target_id.to_string(),
                change,
                level: GrantLevel::None,
            })
        } else {
            inner.entries.remove(target_id);
            None
        }
    }

    /// The window a target id names, if codeg named one.
    pub fn get(&self, target_id: &str) -> Option<TargetEntry> {
        self.lock().entries.get(target_id).cloned()
    }

    /// Share a window at `level`, or stop sharing it at [`GrantLevel::None`].
    ///
    /// `Ok(None)` when nothing changed — the window was already at that level
    /// — so the caller neither emits an event nor restarts the idle clock. A
    /// window shared with its whole application changes only with it
    /// ([`ShareError::AppShared`]); every window, while the entire screen is
    /// shared, only with the screen ([`ShareError::ScreenShared`]).
    pub fn share(
        &self,
        target_id: &str,
        level: GrantLevel,
        now: i64,
        me: &SelfIdentity,
        blocklist: &Blocklist,
    ) -> Result<Option<ComputerGrantPayload>, ShareError> {
        let mut inner = self.lock();
        let Inner {
            entries,
            apps,
            screen,
            ..
        } = &mut *inner;
        if screen.is_some() {
            return Err(ShareError::ScreenShared);
        }
        let entry = entries.get_mut(target_id).ok_or(ShareError::NoSuchTarget)?;
        let with_app = entry
            .grant
            .as_ref()
            .is_some_and(|g| g.scope == GrantScope::App)
            || (!entry.gone && AppIdentity::of(entry).is_some_and(|app| apps.contains_key(&app)));
        if with_app {
            return Err(ShareError::AppShared);
        }
        if level == GrantLevel::None {
            return Ok(Self::revoke_entry(entry, GrantChange::Revoked));
        }
        if entry.gone {
            return Err(ShareError::Gone);
        }
        grantable(&entry.app, me, blocklist).map_err(ShareError::NotGrantable)?;
        if level_of(entry.grant.as_ref()) == level {
            return Ok(None);
        }
        match entry.grant.as_mut() {
            // A change of level on a live grant is the same grant: the reads
            // already made under it stay valid, and the clock keeps running
            // from the last one.
            Some(grant) => grant.level = level,
            None => {
                entry.grant = Some(ComputerGrant::new(level, now));
                entry.epoch += 1;
                entry.reads = 0;
                entry.snapshot_mark = None;
                entry.capture_mark = None;
            }
        }
        Ok(Some(ComputerGrantPayload {
            target_id: target_id.to_string(),
            change: GrantChange::Granted,
            level,
        }))
    }

    /// Share an application as a whole at `level` — the one `target` names —
    /// or end its share at [`GrantLevel::None`]. Every window of it codeg has
    /// named and a person could mean takes the application's grant at its
    /// level, one shared on its own before included; the ones it opens later
    /// take it as listings find them. Ending it ends every window's share of
    /// it. Nothing changes while the entire screen is shared
    /// ([`ShareError::ScreenShared`]).
    pub fn share_app(
        &self,
        target: AppTarget<'_>,
        level: GrantLevel,
        now: i64,
        me: &SelfIdentity,
        blocklist: &Blocklist,
    ) -> Result<AppChange, ShareError> {
        let mut inner = self.lock();
        let Inner {
            entries,
            apps,
            next_app_id,
            screen,
            ..
        } = &mut *inner;
        if screen.is_some() {
            return Err(ShareError::ScreenShared);
        }
        let (identity, app) = match target {
            AppTarget::Window(target_id) => {
                let entry = entries.get(target_id).ok_or(ShareError::NoSuchTarget)?;
                if entry.gone {
                    return Err(ShareError::Gone);
                }
                let identity = AppIdentity::of(entry)
                    .ok_or(ShareError::NotGrantable(NotGrantable::Unidentified))?;
                (identity, entry.app.clone())
            }
            AppTarget::Share(app_id) => match apps.iter().find(|(_, s)| s.app_id == app_id) {
                Some((identity, share)) => (identity.clone(), share.app.clone()),
                None if level == GrantLevel::None => return Ok(AppChange::default()),
                None => return Err(ShareError::NoSuchTarget),
            },
        };
        if level == GrantLevel::None {
            return Ok(Self::end_app(
                entries,
                apps,
                &identity,
                GrantChange::Revoked,
            ));
        }
        grantable(&app, me, blocklist).map_err(ShareError::NotGrantable)?;
        let app_changed = match apps.get_mut(&identity) {
            Some(share) if share.grant.level == level => false,
            // The same share at another level: its clock keeps running.
            Some(share) => {
                share.grant.level = level;
                true
            }
            None => {
                *next_app_id += 1;
                apps.insert(
                    identity.clone(),
                    AppShare {
                        app_id: format!("a{next_app_id}"),
                        grant: ComputerGrant::of_app(level, now),
                        app,
                    },
                );
                true
            }
        };
        let Some(share) = apps.get(&identity) else {
            return Ok(AppChange::default());
        };
        let windows = entries
            .values_mut()
            .filter(|e| !e.gone && (e.grant.is_some() || e.worth_listing()))
            .filter(|e| AppIdentity::of(e).as_ref() == Some(&identity))
            .filter_map(|e| Self::grant_with(e, &share.grant))
            .collect();
        Ok(AppChange {
            windows,
            app_changed,
        })
    }

    /// Give `entry` its share of a grant on more than itself — its whole
    /// application's, or the entire screen's — at that grant's level, in its
    /// scope and on its clock. A window already shared keeps the reads made
    /// under its grant, as a change of level does.
    fn grant_with(entry: &mut TargetEntry, shared: &ComputerGrant) -> Option<ComputerGrantPayload> {
        let (level, scope) = (shared.level, shared.scope);
        match entry.grant.as_mut() {
            Some(grant) if grant.scope == scope && grant.level == level => return None,
            Some(grant) => {
                grant.level = level;
                grant.scope = scope;
            }
            None => {
                entry.grant = Some(shared.clone());
                entry.epoch += 1;
                entry.reads = 0;
                entry.snapshot_mark = None;
                entry.capture_mark = None;
            }
        }
        Some(ComputerGrantPayload {
            target_id: entry.target_id.clone(),
            change: GrantChange::Granted,
            level,
        })
    }

    /// End the share of application `identity`, and every window's share of
    /// it.
    fn end_app(
        entries: &mut HashMap<String, TargetEntry>,
        apps: &mut HashMap<AppIdentity, AppShare>,
        identity: &AppIdentity,
        change: GrantChange,
    ) -> AppChange {
        let app_changed = apps.remove(identity).is_some();
        let windows = entries
            .values_mut()
            .filter(|e| AppIdentity::of(e).as_ref() == Some(identity))
            .filter_map(|e| Self::revoke_entry(e, change))
            .collect();
        AppChange {
            windows,
            app_changed,
        }
    }

    /// Share the entire screen at `level`, or end its share at
    /// [`GrantLevel::None`]. Every window codeg has named that a person could
    /// mean and the rules allow takes the screen's grant at its level — one
    /// shared on its own, or with its application, included: the
    /// applications shared as a whole are shared with the screen from then
    /// on, and their own shares end. The windows that come up later take it
    /// as listings find them. Ending it ends every window's share of it.
    pub fn share_screen(
        &self,
        level: GrantLevel,
        now: i64,
        me: &SelfIdentity,
        blocklist: &Blocklist,
    ) -> AppChange {
        let mut inner = self.lock();
        let Inner {
            entries,
            apps,
            screen,
            screen_epochs,
            ..
        } = &mut *inner;
        if level == GrantLevel::None {
            return Self::end_screen(entries, screen, GrantChange::Revoked);
        }
        let mut out = AppChange::default();
        match screen.as_mut() {
            Some(share) => {
                // The same sharing at another level: its clock keeps running.
                if share.grant.level != level {
                    share.grant.level = level;
                    out.app_changed = true;
                }
                share.me = me.clone();
                share.blocklist = blocklist.clone();
            }
            None => {
                *screen_epochs += 1;
                *screen = Some(ScreenShare {
                    grant: ComputerGrant::of_screen(level, now),
                    epoch: *screen_epochs,
                    reads: 0,
                    capture_mark: None,
                    me: me.clone(),
                    blocklist: blocklist.clone(),
                });
                out.app_changed = true;
            }
        }
        if !apps.is_empty() {
            apps.clear();
            out.app_changed = true;
        }
        let Some(share) = screen.as_ref() else {
            return out;
        };
        for entry in entries.values_mut() {
            let shareable = !entry.gone
                && (entry.grant.is_some() || entry.worth_listing())
                && grantable(&entry.app, me, blocklist).is_ok();
            if shareable {
                out.windows.extend(Self::grant_with(entry, &share.grant));
            }
        }
        out
    }

    /// End the share of the entire screen, for `change` — the switch for it
    /// turned off — and every window's share of it.
    pub fn end_screen_share(&self, change: GrantChange) -> AppChange {
        let mut inner = self.lock();
        let Inner {
            entries, screen, ..
        } = &mut *inner;
        Self::end_screen(entries, screen, change)
    }

    /// End the share of the entire screen, and every window's share of it.
    fn end_screen(
        entries: &mut HashMap<String, TargetEntry>,
        screen: &mut Option<ScreenShare>,
        change: GrantChange,
    ) -> AppChange {
        let app_changed = screen.take().is_some();
        let windows = entries
            .values_mut()
            .filter(|e| {
                e.grant
                    .as_ref()
                    .is_some_and(|g| g.scope == GrantScope::Screen)
            })
            .filter_map(|e| Self::revoke_entry(e, change))
            .collect();
        AppChange {
            windows,
            app_changed,
        }
    }

    /// End the grant `target_id` holds: its own, or — for a window shared
    /// with its whole application — the application's, with every window's
    /// share of it. A window shared with the entire screen loses its share of
    /// it alone, unless the screen's own time is up (`Expired`), which ends
    /// the screen's share.
    fn end_grant(
        entries: &mut HashMap<String, TargetEntry>,
        apps: &mut HashMap<AppIdentity, AppShare>,
        screen: &mut Option<ScreenShare>,
        target_id: &str,
        change: GrantChange,
    ) -> Vec<ComputerGrantPayload> {
        let Some(entry) = entries.get_mut(target_id) else {
            return Vec::new();
        };
        match entry.grant.as_ref().map(|g| g.scope) {
            Some(GrantScope::App) => match AppIdentity::of(entry) {
                Some(app) => Self::end_app(entries, apps, &app, change).windows,
                None => Self::revoke_entry(entry, change).into_iter().collect(),
            },
            Some(GrantScope::Screen) if change == GrantChange::Expired => {
                Self::end_screen(entries, screen, change).windows
            }
            _ => Self::revoke_entry(entry, change).into_iter().collect(),
        }
    }

    fn revoke_entry(entry: &mut TargetEntry, change: GrantChange) -> Option<ComputerGrantPayload> {
        entry.grant.take()?;
        entry.epoch += 1;
        entry.snapshot_mark = None;
        entry.capture_mark = None;
        Some(ComputerGrantPayload {
            target_id: entry.target_id.clone(),
            change,
            level: GrantLevel::None,
        })
    }

    /// End every grant, for one reason — the applications shared as a whole
    /// and the entire screen with them. Used when the user switches computer
    /// use off, which is a statement about every window at once, and for
    /// Stop.
    pub fn revoke_all(&self, change: GrantChange) -> AppChange {
        let mut inner = self.lock();
        let app_changed = !inner.apps.is_empty() || inner.screen.is_some();
        inner.apps.clear();
        inner.screen = None;
        let windows = inner
            .entries
            .values_mut()
            .filter_map(|entry| Self::revoke_entry(entry, change))
            .collect();
        AppChange {
            windows,
            app_changed,
        }
    }

    /// The window is not the one that was shared any more (it closed, or its
    /// process is gone). Ends its grant, if it had one.
    pub fn target_changed(&self, target_id: &str) -> Option<ComputerGrantPayload> {
        let mut inner = self.lock();
        Self::retire(&mut inner, target_id, GrantChange::TargetChanged)
    }

    /// End every grant that no longer holds by the rules as they are now: gone
    /// unused for `ttl`, or on a window that can no longer be shared (its
    /// application joined the blocklist). Run before anything is listed, when
    /// the settings change and on a timer, so what an agent sees of a window
    /// never reflects a grant that has already ended. An application shared
    /// as a whole goes on its own clock, which its windows share; so does the
    /// entire screen, which follows the rules as they are now from here on.
    pub fn sweep(
        &self,
        now: i64,
        ttl: Option<Duration>,
        me: &SelfIdentity,
        blocklist: &Blocklist,
    ) -> AppChange {
        let mut inner = self.lock();
        let Inner {
            entries,
            apps,
            screen,
            ..
        } = &mut *inner;
        let ending: Vec<(AppIdentity, GrantChange)> = apps
            .iter()
            .filter_map(|(identity, share)| {
                if grantable(&share.app, me, blocklist).is_err() {
                    Some((identity.clone(), GrantChange::Revoked))
                } else if share.grant.lapsed(now, ttl) {
                    Some((identity.clone(), GrantChange::Expired))
                } else {
                    None
                }
            })
            .collect();
        let mut out = AppChange::default();
        for (identity, change) in ending {
            out.absorb(Self::end_app(entries, apps, &identity, change));
        }
        if screen.as_ref().is_some_and(|s| s.grant.lapsed(now, ttl)) {
            out.absorb(Self::end_screen(entries, screen, GrantChange::Expired));
        } else if let Some(share) = screen.as_mut() {
            share.me = me.clone();
            share.blocklist = blocklist.clone();
        }
        for entry in entries.values_mut() {
            let Some(grant) = entry.grant.as_ref() else {
                continue;
            };
            let change = if grantable(&entry.app, me, blocklist).is_err() {
                Some(GrantChange::Revoked)
            } else if grant.scope == GrantScope::App {
                // A share left of an application whose own has ended.
                let shared = AppIdentity::of(entry).is_some_and(|app| apps.contains_key(&app));
                (!shared).then_some(GrantChange::Revoked)
            } else if grant.scope == GrantScope::Screen {
                // A share left of a screen whose own has ended.
                screen.is_none().then_some(GrantChange::Revoked)
            } else if grant.lapsed(now, ttl) {
                Some(GrantChange::Expired)
            } else {
                None
            };
            if let Some(change) = change {
                out.windows.extend(Self::revoke_entry(entry, change));
            }
        }
        out
    }

    /// End the shares of applications that no longer run — `alive` says
    /// whether a process run still does. Their windows have gone with them;
    /// this is the application's own share, which would otherwise wait out
    /// its clock.
    pub fn prune_apps(&self, alive: impl Fn(u32, u64) -> bool) -> AppChange {
        let mut inner = self.lock();
        let Inner { entries, apps, .. } = &mut *inner;
        let quit: Vec<AppIdentity> = apps
            .keys()
            .filter(|app| {
                !alive(app.pid, app.started_at)
                    || app
                        .content
                        .is_some_and(|run| !alive(run.pid, run.started_at))
            })
            .cloned()
            .collect();
        let mut out = AppChange::default();
        for identity in quit {
            out.absorb(Self::end_app(
                entries,
                apps,
                &identity,
                GrantChange::TargetChanged,
            ));
        }
        out
    }

    /// Whether `entry`'s grant has lapsed: on its own clock, or — shared
    /// with its whole application, or the entire screen — on the
    /// application's or the screen's (a share whose own has ended has ended
    /// with it).
    fn lapsed(
        entry: &TargetEntry,
        apps: &HashMap<AppIdentity, AppShare>,
        screen: &Option<ScreenShare>,
        now: i64,
        ttl: Option<Duration>,
    ) -> bool {
        match entry.grant.as_ref() {
            None => false,
            Some(grant) if grant.scope == GrantScope::App => AppIdentity::of(entry)
                .and_then(|app| apps.get(&app))
                .is_none_or(|share| share.grant.lapsed(now, ttl)),
            Some(grant) if grant.scope == GrantScope::Screen => screen
                .as_ref()
                .is_none_or(|share| share.grant.lapsed(now, ttl)),
            Some(grant) => grant.lapsed(now, ttl),
        }
    }

    /// A read or an action used `entry`'s grant now: its clock, and its
    /// application's or the screen's when it is shared with it, start again.
    fn used(
        entry: &mut TargetEntry,
        apps: &mut HashMap<AppIdentity, AppShare>,
        screen: &mut Option<ScreenShare>,
        now: i64,
    ) {
        let Some(grant) = entry.grant.as_mut() else {
            return;
        };
        grant.last_used_at = now;
        match grant.scope {
            GrantScope::App => {
                if let Some(share) = AppIdentity::of(entry).and_then(|app| apps.get_mut(&app)) {
                    share.grant.last_used_at = now;
                }
            }
            GrantScope::Screen => {
                if let Some(share) = screen.as_mut() {
                    share.grant.last_used_at = now;
                }
            }
            GrantScope::Window => {}
        }
    }

    /// Check a read may start: the window is one codeg named, it is shared,
    /// and the grant has not lapsed. A lapsed grant is ended here and its
    /// payloads returned alongside the refusal, because the caller is the one
    /// holding an emitter.
    pub fn begin_read(
        &self,
        target_id: &str,
        now: i64,
        ttl: Option<Duration>,
        me: &SelfIdentity,
        blocklist: &Blocklist,
    ) -> Result<ReadTicket, (ReadRefusal, Vec<ComputerGrantPayload>)> {
        let mut inner = self.lock();
        let Inner {
            entries,
            apps,
            screen,
            ..
        } = &mut *inner;
        let Some(entry) = entries.get(target_id) else {
            return Err((ReadRefusal::NoSuchTarget, Vec::new()));
        };
        // Checked even for a window that holds a grant: the blocklist can grow
        // while a window is shared, and the list is what the user said last.
        if let Err(why) = grantable(&entry.app, me, blocklist) {
            let ended = Self::end_grant(entries, apps, screen, target_id, GrantChange::Revoked);
            return Err((ReadRefusal::NotGrantable(why), ended));
        }
        let Some(grant) = entry.grant.as_ref() else {
            return Err((ReadRefusal::GrantRequired, Vec::new()));
        };
        let (level, scope) = (grant.level, grant.scope);
        if Self::lapsed(entry, apps, screen, now, ttl) {
            let ended = Self::end_grant(entries, apps, screen, target_id, GrantChange::Expired);
            return Err((ReadRefusal::GrantRequired, ended));
        }
        if !level.allows(GrantLevel::Read) {
            return Err((ReadRefusal::GrantRequired, Vec::new()));
        }
        let Some(entry) = entries.get_mut(target_id) else {
            return Err((ReadRefusal::NoSuchTarget, Vec::new()));
        };
        Self::used(entry, apps, screen, now);
        Ok(ReadTicket {
            target_id: entry.target_id.clone(),
            identity: entry.identity,
            epoch: entry.epoch,
            app: entry.app.clone(),
            bounds: entry.bounds,
            scope,
        })
    }

    /// Check a read that has finished may be handed over: the same grant is
    /// still in force on the same window, and the window may still be shared
    /// by the rules as they are now. The person may have taken the grant back,
    /// or put the application on the blocklist, while the capture was in
    /// flight, and what the capture holds is exactly what they took back. A
    /// grant the blocklist now forbids is ended here, its payloads returned
    /// for the caller to announce.
    ///
    /// Returns the generation that names this read. `mark`, when the read
    /// leaves one, becomes the window's latest snapshot or screenshot under
    /// that generation — what later actions resolve refs and points against.
    pub fn finish_read(
        &self,
        ticket: &ReadTicket,
        me: &SelfIdentity,
        blocklist: &Blocklist,
        mark: Option<ReadMark>,
    ) -> Result<String, (ReadRefusal, Vec<ComputerGrantPayload>)> {
        let mut inner = self.lock();
        let Inner {
            entries,
            apps,
            screen,
            ..
        } = &mut *inner;
        let Some(entry) = entries.get(&ticket.target_id) else {
            return Err((ReadRefusal::GrantRequired, Vec::new()));
        };
        let still = entry.identity == ticket.identity
            && entry.epoch == ticket.epoch
            && level_of(entry.grant.as_ref()).allows(GrantLevel::Read);
        if !still {
            return Err((ReadRefusal::GrantRequired, Vec::new()));
        }
        if let Err(why) = grantable(&entry.app, me, blocklist) {
            let ended = Self::end_grant(
                entries,
                apps,
                screen,
                &ticket.target_id,
                GrantChange::Revoked,
            );
            return Err((ReadRefusal::NotGrantable(why), ended));
        }
        let Some(entry) = entries.get_mut(&ticket.target_id) else {
            return Err((ReadRefusal::GrantRequired, Vec::new()));
        };
        entry.reads += 1;
        let generation = generation(entry.epoch, entry.reads);
        match mark {
            Some(ReadMark::Snapshot {
                snapshot_id,
                shown,
                cut,
                secret,
            }) => {
                entry.snapshot_mark = Some(SnapshotMark {
                    generation: generation.clone(),
                    snapshot_id,
                    shown,
                    cut,
                    secret,
                })
            }
            Some(ReadMark::Capture {
                width,
                height,
                native_width,
                native_height,
                full_size,
                window_bounds,
            }) => {
                entry.capture_mark = Some(CaptureMark {
                    generation: generation.clone(),
                    width,
                    height,
                    native_width,
                    native_height,
                    full_size,
                    window_bounds,
                })
            }
            None => {}
        }
        Ok(generation)
    }

    /// Check an action may go ahead, and resolve it for the helper.
    ///
    /// In this order, each answered before the next is asked: the window is
    /// one codeg named; it may still be shared at all; it is shared; the grant
    /// has not lapsed; it is shared for control — and only then anything about
    /// the action itself: keys and menus the grant does not reach (a window's,
    /// or its whole application's), then every ref against the window's
    /// latest snapshot and every point against its latest screenshot, as the
    /// agent was given them. A refusal therefore never says more about a
    /// window than the agent was allowed to know.
    ///
    /// Counts as use of the grant, like a read.
    // One argument per thing the decision reads, as `begin_read` takes them,
    // and whether a paste may go (`resolve`).
    #[allow(clippy::too_many_arguments)]
    pub fn begin_act(
        &self,
        target_id: &str,
        now: i64,
        ttl: Option<Duration>,
        me: &SelfIdentity,
        blocklist: &Blocklist,
        request: &ComputerActRequest,
        paste_ok: bool,
    ) -> Result<ActTicket, (ActDenied, Vec<ComputerGrantPayload>)> {
        let mut inner = self.lock();
        let Inner {
            entries,
            apps,
            screen,
            ..
        } = &mut *inner;
        let Some(entry) = entries.get(target_id) else {
            return Err((ActDenied::NoSuchTarget, Vec::new()));
        };
        if let Err(why) = grantable(&entry.app, me, blocklist) {
            let ended = Self::end_grant(entries, apps, screen, target_id, GrantChange::Revoked);
            return Err((ActDenied::NotGrantable(why), ended));
        }
        let Some(grant) = entry.grant.as_ref() else {
            return Err((ActDenied::GrantRequired, Vec::new()));
        };
        let level = grant.level;
        if Self::lapsed(entry, apps, screen, now, ttl) {
            let ended = Self::end_grant(entries, apps, screen, target_id, GrantChange::Expired);
            return Err((ActDenied::GrantRequired, ended));
        }
        if !level.allows(GrantLevel::Read) {
            return Err((ActDenied::GrantRequired, Vec::new()));
        }
        if !level.allows(GrantLevel::Control) {
            return Err((ActDenied::ControlRequired, Vec::new()));
        }
        let action = resolve(entry, request, paste_ok).map_err(|why| (why, Vec::new()))?;
        let Some(entry) = entries.get_mut(target_id) else {
            return Err((ActDenied::NoSuchTarget, Vec::new()));
        };
        Self::used(entry, apps, screen, now);
        Ok(ActTicket {
            target_id: entry.target_id.clone(),
            identity: entry.identity,
            epoch: entry.epoch,
            app: entry.app.clone(),
            aim: Aim::of(entry, &action),
            action,
        })
    }

    /// Every window with a grant in force, oldest grant first.
    pub fn shared(&self) -> Vec<SharedWindow> {
        let inner = self.lock();
        let mut out: Vec<SharedWindow> = inner
            .entries
            .values()
            .filter_map(|e| {
                let grant = e.grant.as_ref()?;
                Some(SharedWindow {
                    target_id: e.target_id.clone(),
                    app_name: e.app.name.clone(),
                    app_key: e.app.key().unwrap_or_default().to_string(),
                    title: e.title.clone(),
                    level: grant.level,
                    granted_at: grant.granted_at,
                    last_used_at: grant.last_used_at,
                    whole_app: grant.scope == GrantScope::App,
                    app_id: (grant.scope == GrantScope::App)
                        .then(|| AppIdentity::of(e))
                        .flatten()
                        .and_then(|app| inner.apps.get(&app))
                        .map(|share| share.app_id.clone()),
                    whole_screen: grant.scope == GrantScope::Screen,
                })
            })
            .collect();
        out.sort_by(|a, b| {
            a.granted_at
                .cmp(&b.granted_at)
                .then(a.target_id.cmp(&b.target_id))
        });
        out
    }

    /// Every application shared as a whole, oldest share first.
    pub fn shared_apps(&self) -> Vec<SharedApp> {
        let inner = self.lock();
        let mut out: Vec<SharedApp> = inner
            .apps
            .iter()
            .map(|(identity, share)| SharedApp {
                app_id: share.app_id.clone(),
                app_name: share.app.name.clone(),
                app_key: identity.key.clone(),
                level: share.grant.level,
                granted_at: share.grant.granted_at,
                last_used_at: share.grant.last_used_at,
                windows: inner
                    .entries
                    .values()
                    .filter(|e| {
                        e.grant.as_ref().is_some_and(|g| g.scope == GrantScope::App)
                            && AppIdentity::of(e).as_ref() == Some(identity)
                    })
                    .count() as u32,
            })
            .collect();
        out.sort_by(|a, b| {
            a.granted_at
                .cmp(&b.granted_at)
                .then(a.app_id.cmp(&b.app_id))
        });
        out
    }

    /// The entire screen, when it is shared.
    pub fn shared_screen(&self) -> Option<SharedScreen> {
        let inner = self.lock();
        let share = inner.screen.as_ref()?;
        Some(SharedScreen {
            level: share.grant.level,
            granted_at: share.grant.granted_at,
            last_used_at: share.grant.last_used_at,
            windows: inner
                .entries
                .values()
                .filter(|e| {
                    e.grant
                        .as_ref()
                        .is_some_and(|g| g.scope == GrantScope::Screen)
                })
                .count() as u32,
        })
    }

    /// Check a read of the entire screen may start: it is shared, and its
    /// grant has not lapsed. A lapsed one is ended here, what that ended
    /// returned alongside the refusal. Counts as use of the grant.
    pub fn begin_screen_read(
        &self,
        now: i64,
        ttl: Option<Duration>,
    ) -> Result<ScreenReadTicket, (ReadRefusal, AppChange)> {
        let mut inner = self.lock();
        let Inner {
            entries, screen, ..
        } = &mut *inner;
        let Some(share) = screen.as_ref() else {
            return Err((ReadRefusal::GrantRequired, AppChange::default()));
        };
        if share.grant.lapsed(now, ttl) {
            let ended = Self::end_screen(entries, screen, GrantChange::Expired);
            return Err((ReadRefusal::GrantRequired, ended));
        }
        let Some(share) = screen.as_mut() else {
            return Err((ReadRefusal::GrantRequired, AppChange::default()));
        };
        if !share.grant.level.allows(GrantLevel::Read) {
            return Err((ReadRefusal::GrantRequired, AppChange::default()));
        }
        share.grant.last_used_at = now;
        Ok(ScreenReadTicket { epoch: share.epoch })
    }

    /// Check a read of the entire screen that has finished may be handed
    /// over: the same sharing of it is still in force. Returns the
    /// generation that names the read; `mark` becomes the screen's latest
    /// picture under it — what later points on the screen are read in.
    pub fn finish_screen_read(
        &self,
        ticket: &ScreenReadTicket,
        mark: ReadMark,
    ) -> Result<String, ReadRefusal> {
        let mut inner = self.lock();
        let Some(share) = inner
            .screen
            .as_mut()
            .filter(|s| s.epoch == ticket.epoch && s.grant.level.allows(GrantLevel::Read))
        else {
            return Err(ReadRefusal::GrantRequired);
        };
        share.reads += 1;
        let generation = generation(share.epoch, share.reads);
        if let ReadMark::Capture {
            width,
            height,
            native_width,
            native_height,
            full_size,
            window_bounds,
        } = mark
        {
            share.capture_mark = Some(CaptureMark {
                generation: generation.clone(),
                width,
                height,
                native_width,
                native_height,
                full_size,
                window_bounds,
            });
        }
        Ok(generation)
    }

    /// Check an action on the entire screen may go ahead, and resolve it for
    /// the helper: the screen is shared for control, its grant has not
    /// lapsed, and the action is a click, a drag or a scroll at points of its
    /// latest picture as the agent was given it. Counts as use of the grant.
    pub fn begin_screen_act(
        &self,
        now: i64,
        ttl: Option<Duration>,
        request: &ComputerActRequest,
    ) -> Result<ScreenActTicket, (ActDenied, AppChange)> {
        let mut inner = self.lock();
        let Inner {
            entries, screen, ..
        } = &mut *inner;
        let Some(share) = screen.as_ref() else {
            return Err((ActDenied::GrantRequired, AppChange::default()));
        };
        if share.grant.lapsed(now, ttl) {
            let ended = Self::end_screen(entries, screen, GrantChange::Expired);
            return Err((ActDenied::GrantRequired, ended));
        }
        let Some(share) = screen.as_mut() else {
            return Err((ActDenied::GrantRequired, AppChange::default()));
        };
        if !share.grant.level.allows(GrantLevel::Read) {
            return Err((ActDenied::GrantRequired, AppChange::default()));
        }
        if !share.grant.level.allows(GrantLevel::Control) {
            return Err((ActDenied::ControlRequired, AppChange::default()));
        }
        let mark = share.capture_mark.as_ref();
        let action = resolve_on_screen(mark, request).map_err(|why| (why, AppChange::default()))?;
        // A point resolved, so the picture it was read in is there.
        let geometry = mark
            .map(|m| ScreenGeometry {
                scale: f64::from(m.native_width) / m.window_bounds.width,
                width: m.window_bounds.width,
                height: m.window_bounds.height,
            })
            .filter(|g| g.scale.is_finite() && g.scale > 0.0)
            .ok_or((ActDenied::NoPointing, AppChange::default()))?;
        share.grant.last_used_at = now;
        Ok(ScreenActTicket {
            epoch: share.epoch,
            action,
            geometry,
        })
    }

    /// Whether the sharing of the entire screen an action was let through
    /// under (`epoch`) is still in force, for control.
    pub fn screen_controlled(&self, epoch: u64) -> bool {
        self.lock()
            .screen
            .as_ref()
            .is_some_and(|s| s.epoch == epoch && s.grant.level.allows(GrantLevel::Control))
    }

    /// What `app` — as a listing of applications names it — is shared for as
    /// a whole; [`GrantLevel::None`] when it is not.
    pub fn app_level(&self, app: &RawApp) -> GrantLevel {
        self.lock()
            .apps
            .iter()
            .find(|(identity, _)| identity.names(app))
            .map_or(GrantLevel::None, |(_, share)| share.grant.level)
    }

    /// The share of the application `target_id` is a window of, if it is
    /// shared as a whole.
    pub fn app_share_of(&self, target_id: &str) -> Option<AppShare> {
        let inner = self.lock();
        let entry = inner.entries.get(target_id)?;
        let identity = AppIdentity::of(entry)?;
        inner.apps.get(&identity).cloned()
    }
}

/// Which application [`TargetTable::share_app`] is to share: the one a window
/// is of, or one already shared, by its share's id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppTarget<'a> {
    Window(&'a str),
    Share(&'a str),
}

impl AppChange {
    pub fn absorb(&mut self, other: AppChange) {
        self.windows.extend(other.windows);
        self.app_changed |= other.app_changed;
    }
}

/// The action as the helper carries it out: keys judged for a window grant,
/// refs and points resolved against what the agent last read of the window.
/// `paste_ok`: the clipboard holds what an agent put there itself, which a
/// paste may then write into the window (see `commands::computer`).
fn resolve(
    entry: &TargetEntry,
    request: &ComputerActRequest,
    paste_ok: bool,
) -> Result<WindowAction, ActDenied> {
    let scope = entry
        .grant
        .as_ref()
        .map_or(GrantScope::Window, |grant| grant.scope);
    Ok(match request {
        ComputerActRequest::Click {
            target,
            button,
            count,
            modifiers,
        } => {
            check_pointer_modifiers(*modifiers, scope)?;
            if *count == 2 && !modifiers.is_empty() {
                return Err(ActDenied::DoubleClickModifiers);
            }
            WindowAction::Click {
                at: resolve_target(entry, target)?,
                button: *button,
                count: *count,
                modifiers: *modifiers,
            }
        }
        ComputerActRequest::Drag {
            from,
            to,
            button,
            modifiers,
            duration_ms,
        } => {
            check_pointer_modifiers(*modifiers, scope)?;
            if !super::keys::drag_carries_modifiers(*modifiers, Platform::current()) {
                return Err(ActDenied::DragModifiers);
            }
            WindowAction::Drag {
                from: resolve_point(entry, from)?,
                to: resolve_point(entry, to)?,
                button: *button,
                modifiers: *modifiers,
                duration_ms: duration_ms
                    .unwrap_or(DEFAULT_DRAG_MS)
                    .min(super::types::MAX_DRAG_MS),
            }
        }
        ComputerActRequest::Scroll {
            target,
            direction,
            amount,
            unit,
        } => WindowAction::Scroll {
            at: target
                .as_ref()
                .map(|t| resolve_target(entry, t))
                .transpose()?,
            direction: *direction,
            amount: *amount,
            unit: *unit,
        },
        ComputerActRequest::Type {
            target,
            text,
            submit,
        } => WindowAction::Type {
            element: resolve_element(entry, target, true)?,
            text: text.clone(),
            submit: *submit,
        },
        ComputerActRequest::Key { target, chord, .. } => {
            check_chord(chord, target.is_some(), scope, paste_ok)?;
            WindowAction::Key {
                element: target
                    .as_ref()
                    .map(|t| resolve_element(entry, t, chord.types_text()))
                    .transpose()?,
                chord: *chord,
            }
        }
        // A held key is the key pressed, again and again: each press is an
        // action of its own (see `commands::computer`).
        ComputerActRequest::HoldKey { target, chord, .. } => {
            check_chord(chord, target.is_some(), scope, paste_ok)?;
            WindowAction::Key {
                element: target
                    .as_ref()
                    .map(|t| resolve_element(entry, t, chord.types_text()))
                    .transpose()?,
                chord: *chord,
            }
        }
        ComputerActRequest::SetValue { target, value } => WindowAction::SetValue {
            element: resolve_element(entry, target, true)?,
            value: value.clone(),
        },
        ComputerActRequest::Restore => WindowAction::Restore,
        ComputerActRequest::InvokeMenu { path } => {
            if scope == GrantScope::Window {
                return Err(ActDenied::AppGrantRequired);
            }
            if Platform::current() == Platform::Windows {
                return Err(ActDenied::MenusUnavailable);
            }
            // A paste by its menu is a paste: it writes the person's
            // clipboard into the window ("Paste Special", "Unformatted Text"
            // included). The helper checks the command's own shortcut too.
            if !paste_ok && path.iter().any(|title| super::keys::names_paste(title)) {
                return Err(ActDenied::Paste);
            }
            WindowAction::InvokeMenu {
                path: path.iter().map(|title| title.trim().to_string()).collect(),
            }
        }
        ComputerActRequest::SetFrame {
            x,
            y,
            width,
            height,
        } => {
            check_frame(*x, *y, *width, *height)?;
            WindowAction::SetFrame {
                x: *x,
                y: *y,
                width: *width,
                height: *height,
            }
        }
    })
}

/// Whether what is given of a window's frame could be one: every number
/// finite, neither side under [`MIN_WINDOW_SIDE`], nothing beyond
/// [`MAX_WINDOW_EXTENT`]. What is left out the helper takes from the window as
/// it finds it just before, not from a listing that may be out of date.
fn check_frame(
    x: Option<f64>,
    y: Option<f64>,
    width: Option<f64>,
    height: Option<f64>,
) -> Result<(), ActDenied> {
    use super::types::{MAX_WINDOW_EXTENT, MIN_WINDOW_SIDE};
    let place = |n: f64| n.is_finite() && n.abs() <= MAX_WINDOW_EXTENT;
    let side = |n: f64| n.is_finite() && (MIN_WINDOW_SIDE..=MAX_WINDOW_EXTENT).contains(&n);
    let given = x.is_some() || y.is_some() || width.is_some() || height.is_some();
    let fits = x.is_none_or(place)
        && y.is_none_or(place)
        && width.is_none_or(side)
        && height.is_none_or(side);
    if given && fits {
        Ok(())
    } else {
        Err(ActDenied::BadFrame)
    }
}

/// How long a drag's path takes when the agent does not say: the driver's
/// own default.
const DEFAULT_DRAG_MS: u32 = 500;

/// An action on the entire screen as the helper carries it out: a click, a
/// drag or a scroll, every point resolved against the screen's latest
/// picture (`mark`) as the agent was given it. Anything else goes to a
/// window.
fn resolve_on_screen(
    mark: Option<&CaptureMark>,
    request: &ComputerActRequest,
) -> Result<WindowAction, ActDenied> {
    Ok(match request {
        ComputerActRequest::Click {
            target: AgentTarget::Point(point),
            button,
            count,
            modifiers,
        } => {
            check_pointer_modifiers(*modifiers, GrantScope::Screen)?;
            WindowAction::Click {
                at: DriverTarget::Point(point_in(mark, point)?),
                button: *button,
                count: *count,
                modifiers: *modifiers,
            }
        }
        ComputerActRequest::Drag {
            from,
            to,
            button,
            modifiers,
            duration_ms,
        } => {
            check_pointer_modifiers(*modifiers, GrantScope::Screen)?;
            if !super::keys::drag_carries_modifiers(*modifiers, Platform::current()) {
                return Err(ActDenied::DragModifiers);
            }
            WindowAction::Drag {
                from: point_in(mark, from)?,
                to: point_in(mark, to)?,
                button: *button,
                modifiers: *modifiers,
                duration_ms: duration_ms
                    .unwrap_or(DEFAULT_DRAG_MS)
                    .min(super::types::MAX_DRAG_MS),
            }
        }
        ComputerActRequest::Scroll {
            target: Some(AgentTarget::Point(point)),
            direction,
            amount,
            unit,
        } => WindowAction::Scroll {
            at: Some(DriverTarget::Point(point_in(mark, point)?)),
            direction: *direction,
            amount: *amount,
            unit: *unit,
        },
        _ => return Err(ActDenied::ScreenPointerOnly),
    })
}

/// Whether the grant reaches `modifiers` held over a click or a drag: a
/// window's (see `keys::pointer_modifiers_allowed`), a whole application's
/// (`keys::pointer_modifiers_allowed_for_app`), or the entire screen's —
/// which reaches every one.
fn check_pointer_modifiers(
    modifiers: super::keys::Modifiers,
    scope: GrantScope,
) -> Result<(), ActDenied> {
    let platform = Platform::current();
    let allowed = match scope {
        GrantScope::Window => super::keys::pointer_modifiers_allowed(modifiers, platform),
        GrantScope::App => super::keys::pointer_modifiers_allowed_for_app(modifiers, platform),
        GrantScope::Screen => true,
    };
    match (allowed, scope) {
        (true, _) => Ok(()),
        (false, GrantScope::Window) => Err(ActDenied::ChordBeyond),
        (false, _) => Err(ActDenied::DesktopChord),
    }
}

/// Whether the grant — a window's, a whole application's, or the entire
/// screen's — reaches `chord`, and, for a key that types a character, that
/// it is aimed at a named element.
fn check_chord(
    chord: &Chord,
    names_element: bool,
    scope: GrantScope,
    paste_ok: bool,
) -> Result<(), ActDenied> {
    let platform = Platform::current();
    let class = match scope {
        GrantScope::Window => classify(chord, platform),
        GrantScope::App => classify_for_app(chord, platform),
        GrantScope::Screen => classify_for_screen(chord, platform),
    };
    match class {
        ChordClass::Beyond if scope == GrantScope::Window => Err(ActDenied::ChordBeyond),
        ChordClass::Beyond if scope == GrantScope::App => Err(ActDenied::DesktopChord),
        ChordClass::Beyond => Err(ActDenied::SessionChord),
        ChordClass::Paste if paste_ok => Ok(()),
        ChordClass::Paste => Err(ActDenied::Paste),
        ChordClass::Window if chord.types_text() && !names_element => Err(ActDenied::NeedsElement),
        ChordClass::Window => Ok(()),
    }
}

fn resolve_target(entry: &TargetEntry, target: &AgentTarget) -> Result<DriverTarget, ActDenied> {
    Ok(match target {
        AgentTarget::Element(e) => DriverTarget::Element(resolve_element(entry, e, false)?),
        AgentTarget::Point(p) => DriverTarget::Point(resolve_point(entry, p)?),
    })
}

/// A ref, against the window's latest snapshot as the agent was given it.
/// `writes`: the action puts text into the element, which a secret field
/// never takes.
fn resolve_element(
    entry: &TargetEntry,
    target: &ElementTarget,
    writes: bool,
) -> Result<ElementRef, ActDenied> {
    let mark = entry
        .snapshot_mark
        .as_ref()
        .ok_or(ActDenied::Stale(Staleness::NoSnapshot))?;
    if mark.generation != target.generation {
        return Err(ActDenied::Stale(Staleness::OldSnapshot));
    }
    let snapshot_id = mark
        .snapshot_id
        .clone()
        .ok_or(ActDenied::Stale(Staleness::NotActionable))?;
    if !mark.shown.contains(&target.index) {
        return Err(ActDenied::Stale(if mark.cut.contains(&target.index) {
            Staleness::CutAway(target.index)
        } else {
            Staleness::NoSuchRef(target.index)
        }));
    }
    if writes && mark.secret.contains(&target.index) {
        return Err(ActDenied::Secret);
    }
    Ok(ElementRef {
        snapshot_id,
        index: target.index,
    })
}

/// A point, in the pixels of the window's latest screenshot, mapped back to
/// the window's own pixels.
fn resolve_point(entry: &TargetEntry, target: &PointTarget) -> Result<WindowPoint, ActDenied> {
    point_in(entry.capture_mark.as_ref(), target)
}

/// A point, in the pixels of the screenshot `mark` names, mapped back to the
/// pixels it was shrunk from.
fn point_in(mark: Option<&CaptureMark>, target: &PointTarget) -> Result<WindowPoint, ActDenied> {
    let mark = mark.ok_or(ActDenied::Stale(Staleness::NoCapture))?;
    if mark.generation != target.generation {
        return Err(ActDenied::Stale(Staleness::OldCapture));
    }
    if !mark.full_size || mark.width == 0 || mark.height == 0 || mark.window_bounds.is_empty() {
        return Err(ActDenied::NoPointing);
    }
    let (x, y) = (target.x, target.y);
    let inside = x.is_finite()
        && y.is_finite()
        && x >= 0.0
        && y >= 0.0
        && x < f64::from(mark.width)
        && y < f64::from(mark.height);
    if !inside {
        return Err(ActDenied::OutOfImage);
    }
    Ok(WindowPoint {
        x: x * f64::from(mark.native_width) / f64::from(mark.width),
        y: y * f64::from(mark.native_height) / f64::from(mark.height),
        window_width: mark.window_bounds.width,
        window_height: mark.window_bounds.height,
    })
}
