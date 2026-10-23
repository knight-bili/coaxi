// Apache-2.0; adapted from spacering-net/codeg aff1e6c278453ed8d0b96a5f664edcdca621aa3f.
//! Each read op, as exactly one driver call and a translation of its answer.
//!
//! This, with [`super::act`] for the ops that change a window, is the
//! whitelist. The driver advertises several dozen tools; the ones named here
//! (`list_apps`, `list_windows`, `get_window_state`, `verify_state`) and
//! there are the only ones the helper ever calls, with arguments built from
//! typed fields — never a tool name or an argument object that came from
//! codeg as-is. (What the driver's listing leaves unsaid — which windows are
//! minimized, and on macOS which applications hidden — the helper asks the
//! system itself: see `mark_out_of_sight`.)

use std::collections::HashMap;
use std::time::Duration;

use serde_json::{json, Map, Value};

use super::act::{ElementFacts, SnapshotFacts};
use super::driver_proc::DriverProc;
use super::mcp::ToolCallResult;
use super::tree::{
    is_masked, names_a_secret, redact_secrets, without_app_menus, AppMenus, Dialect, TreeNode,
    APP_MENU_ROLES,
};
use crate::computer::procinfo::process_start;
use crate::computer::protocol::{
    HelperError, HelperErrorCode, InstalledApp, OsPermission, RawApp, RawCapture, RawClipboard,
    RawLaunch, RawSnapshot, RawVerify, RawWindow, SnapshotRef,
};
use crate::computer::types::{
    PredicateResult, Rect, VerifyPredicate, VerifyRequest, VerifyStatus, MAX_VERIFY_PREDICATES,
};

const LIST_TIMEOUT: Duration = Duration::from_secs(30);
/// A window-state read: an accessibility walk of at most [`WALK_BUDGET_MS`],
/// the capture, and the encode.
const WINDOW_STATE_TIMEOUT: Duration = Duration::from_secs(60);
/// How long the driver may walk a window's accessibility tree for a
/// snapshot, in milliseconds. Its own default is a second, which cuts a
/// large application's tree short; this is the twenty seconds macOS had
/// before it, and within [`WINDOW_STATE_TIMEOUT`] even where the driver
/// waits twice that and five seconds more for an application to answer.
const WALK_BUDGET_MS: u64 = 20_000;
/// Added to the caller's own `timeoutMs` for `verify_state`.
const VERIFY_OVERHEAD: Duration = Duration::from_secs(30);
/// The driver's own bounds on a verify.
const MAX_VERIFY_TIMEOUT_MS: u32 = 10_000;
const MAX_STABLE_SAMPLES: u32 = 5;

/// Turn a refused driver call into the helper's error, by the driver's own
/// refusal code where it gave one.
pub fn tool_error(tool: &str, result: &ToolCallResult) -> HelperError {
    let code = result.code().unwrap_or("");
    let text = result.text();
    let words = if text.is_empty() {
        format!("{tool} failed")
    } else {
        text
    };
    match code {
        "screen_recording_permission_denied" => {
            HelperError::permission_missing(OsPermission::ScreenRecording)
        }
        "permission_denied" | "accessibility_permission_denied" | "tcc_permission_denied" => {
            HelperError::permission_missing(OsPermission::Accessibility)
        }
        "window_id_not_found" | "window_owner_pid_mismatch" => {
            HelperError::new(HelperErrorCode::NoSuchWindow, words)
        }
        _ => HelperError::failed(words),
    }
}

async fn call(
    driver: &DriverProc,
    tool: &str,
    arguments: Value,
    timeout: Duration,
) -> Result<ToolCallResult, HelperError> {
    let result = driver.call(tool, arguments, timeout).await?;
    if result.is_error {
        return Err(tool_error(tool, &result));
    }
    Ok(result)
}

pub(super) fn structured<'a>(
    tool: &str,
    result: &'a ToolCallResult,
) -> Result<&'a Value, HelperError> {
    result
        .structured
        .as_ref()
        .ok_or_else(|| HelperError::failed(format!("{tool} answered without structured content")))
}

pub(super) fn rect(value: &Value) -> Option<Rect> {
    Some(Rect {
        x: value.get("x")?.as_f64()?,
        y: value.get("y")?.as_f64()?,
        width: value.get("width")?.as_f64()?,
        height: value.get("height")?.as_f64()?,
    })
}

fn string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Running applications, each stamped with its start time: the driver's own
/// list. On macOS and Windows that list is not read — on macOS it is frozen
/// at the driver's first call, and on Windows it knows most processes by
/// their executable's file name alone (see `appident`) — and the
/// applications are the owners of the windows instead ([`apps_of`]).
#[cfg(not(any(target_os = "macos", windows)))]
pub async fn list_apps(
    driver: &DriverProc,
    _cache: &tokio::sync::Mutex<AppCache>,
) -> Result<Vec<RawApp>, HelperError> {
    driver_apps(driver).await
}

/// The identified applications among the owners of `windows` a person could
/// mean — on screen, minimized, hidden with their application, or on another
/// desktop or Space — once each,
/// with the one whose window is frontmost on screen marked active. One
/// process can be several: the frame host is the application in each of its
/// frames. A window nobody can see on this desktop names no application: an
/// application with only such windows has nothing to share, and the one a
/// minimized frame shows is already named by the frame — its own window,
/// standing outside the frame meanwhile, would name it twice.
///
/// On macOS and Windows these are the running applications, each identified
/// by the helper itself (see [`list_windows`]), from the listing — minimized
/// and hidden windows marked — that a window list is made from.
#[cfg(any(test, target_os = "macos", windows))]
pub fn apps_of(windows: Vec<RawWindow>) -> Vec<RawApp> {
    let app_of = |w: &RawWindow| (w.pid, w.app.started_at, w.app.key().map(str::to_string));
    let meant = |w: &RawWindow| {
        w.on_screen
            || w.minimized == Some(true)
            || w.hidden == Some(true)
            || w.on_current_space == Some(false)
    };
    let front = windows
        .iter()
        .filter(|w| w.on_screen)
        .max_by_key(|w| w.z_index.unwrap_or(i64::MIN))
        .map(app_of);
    let mut seen = std::collections::HashSet::new();
    windows
        .into_iter()
        .filter(|w| meant(w) && w.app.key().is_some() && seen.insert(app_of(w)))
        .map(|w| {
            let active = front == Some(app_of(&w));
            RawApp { active, ..w.app }
        })
        .collect()
}

/// How long starting an application may take before the driver answers.
const LAUNCH_TIMEOUT: Duration = Duration::from_secs(30);

/// The installed application listed under `key` — its bundle identifier or
/// its launch path, as the driver's application list gives them — or else
/// `name`, any case: the one application, running or not, that goes by it.
/// A name two applications go by names neither: the answer lists their
/// keys.
///
/// What it is, for codeg to judge by, is kept apart from the command that
/// starts it ([`InstalledApp`]): its bundle identifier where it has one that
/// is not a path, and its executable — the path the driver gives, or else the
/// first word of its launch command.
pub async fn find_app(
    driver: &DriverProc,
    name: Option<&str>,
    key: Option<&str>,
) -> Result<InstalledApp, HelperError> {
    let result = call(driver, "list_apps", json!({}), LIST_TIMEOUT).await?;
    let listed = required_array("list_apps", structured("list_apps", &result)?, "apps")?;
    let apps = listed.iter().filter_map(|a| {
        let bundle = string(a, "bundle_id");
        let launch_path = string(a, "launch_path");
        // On Windows a "bundle identifier" can be the executable's path.
        let (bundle_id, path) = match bundle {
            Some(b) if b.contains(['/', '\\']) => (None, Some(b)),
            other => (
                other,
                launch_path
                    .as_deref()
                    .and_then(|l| crate::computer::agent::command_words(l).into_iter().next()),
            ),
        };
        Some((
            InstalledApp {
                app: RawApp {
                    pid: a
                        .get("pid")
                        .and_then(Value::as_u64)
                        .and_then(|p| u32::try_from(p).ok())
                        .unwrap_or(0),
                    name: string(a, "name")?,
                    bundle_id,
                    path,
                    active: false,
                    started_at: None,
                },
                launch_path: launch_path.clone(),
            },
            (string(a, "bundle_id"), launch_path),
        ))
    });
    let wanted = |listed: &(Option<String>, Option<String>), app: &RawApp| match (
        key.map(str::trim),
        name.map(str::trim),
    ) {
        (Some(key), _) => listed.0.as_deref() == Some(key) || listed.1.as_deref() == Some(key),
        (None, Some(name)) => app.name.eq_ignore_ascii_case(name),
        (None, None) => false,
    };
    let mut found: Vec<InstalledApp> = Vec::new();
    for (installed, listed) in apps {
        if !wanted(&listed, &installed.app) {
            continue;
        }
        let same = |f: &InstalledApp| {
            f.app.key() == installed.app.key() && f.launch_path == installed.launch_path
        };
        if installed.app.key().is_some() && !found.iter().any(same) {
            found.push(installed);
        }
    }
    let asked = key.or(name).unwrap_or_default();
    match found.len() {
        1 => Ok(found.remove(0)),
        0 => Err(HelperError::new(
            HelperErrorCode::ActionFailed,
            format!(
                "No installed application goes by \"{asked}\". Name it as the system lists \
                 it, or by its key (a bundle identifier or path)."
            ),
        )),
        _ => Err(HelperError::new(
            HelperErrorCode::ActionFailed,
            format!(
                "Several installed applications go by \"{asked}\"; name one by its key: {}.",
                found
                    .iter()
                    .filter_map(|f| f.launch_path.as_deref().or(f.app.key()))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        )),
    }
}

/// Start `app` — found by [`find_app`] — in the background, by what the
/// driver listed it under: its bundle identifier on macOS; on Windows its
/// launch path, or its application id; on Linux its launch command, which
/// the driver splits into a program and its arguments as its desktop file
/// gave them.
pub async fn launch_app(driver: &DriverProc, app: &InstalledApp) -> Result<RawLaunch, HelperError> {
    let missing = || HelperError::new(HelperErrorCode::BadRequest, "nothing to start it by");
    let args = match crate::computer::keys::Platform::current() {
        crate::computer::keys::Platform::Mac => {
            json!({ "bundle_id": app.app.bundle_id.as_deref().ok_or_else(missing)? })
        }
        crate::computer::keys::Platform::Windows => match (&app.launch_path, &app.app.bundle_id) {
            (Some(path), _) => json!({ "launch_path": path }),
            (None, Some(id)) => json!({ "bundle_id": id }),
            (None, None) => return Err(missing()),
        },
        crate::computer::keys::Platform::Linux => {
            json!({ "launch_path": app.launch_path.as_deref().ok_or_else(missing)? })
        }
    };
    let result = call(driver, "launch_app", args, LAUNCH_TIMEOUT).await?;
    let said = result.structured.as_ref();
    Ok(RawLaunch {
        pid: said
            .and_then(|s| s.get("pid"))
            .and_then(Value::as_u64)
            .and_then(|p| u32::try_from(p).ok())
            .filter(|p| *p > 0),
        name: said
            .and_then(|s| string(s, "name"))
            .unwrap_or_else(|| app.app.name.clone()),
    })
}

/// The most clipboard text handed back to an agent at once.
const MAX_CLIPBOARD_CHARS: usize = 100_000;

/// The clipboard's text, while the clipboard is still as `expect` names it —
/// what an agent put there — and holds nothing concealed.
pub async fn clipboard_read(driver: &DriverProc, expect: u64) -> Result<RawClipboard, HelperError> {
    let not_yours = || {
        HelperError::new(
            HelperErrorCode::PasteRefused,
            "The clipboard holds what the user put there, not what you copied from a window you \
             may read or wrote yourself, so it is not read for you.",
        )
    };
    let now = super::clipboard::stamp(driver).await?;
    if now.value != expect || now.concealed {
        return Err(not_yours());
    }
    let result = call(
        driver,
        "clipboard_read",
        json!({ "include_text": true }),
        LIST_TIMEOUT,
    )
    .await?;
    // What was read is what was checked only if nothing came between.
    let after = super::clipboard::stamp(driver).await?;
    if after.value != expect || after.concealed {
        return Err(not_yours());
    }
    let text = result
        .structured
        .as_ref()
        .and_then(|s| string(s, "text"))
        .map(|text| text.chars().take(MAX_CLIPBOARD_CHARS).collect());
    Ok(RawClipboard { text })
}

/// Put `text` on the clipboard; answers with the clipboard's stamp after —
/// what the agent itself put there, until anything else does. The stamp is
/// the agent's only once the clipboard is read back holding that text, with
/// the stamp the same before and after the reading: something copied in the
/// moment between would otherwise be taken for the agent's.
pub async fn clipboard_write(driver: &DriverProc, text: &str) -> Result<u64, HelperError> {
    call(
        driver,
        "clipboard_write",
        json!({ "text": text }),
        LIST_TIMEOUT,
    )
    .await?;
    let before = super::clipboard::stamp(driver).await?;
    let result = call(
        driver,
        "clipboard_read",
        json!({ "include_text": true }),
        LIST_TIMEOUT,
    )
    .await?;
    let after = super::clipboard::stamp(driver).await?;
    let read = result
        .structured
        .as_ref()
        .and_then(|s| s.get("text"))
        .and_then(Value::as_str)
        .map(|t| t.replace("\r\n", "\n"));
    let ours = read.as_deref() == Some(text.replace("\r\n", "\n").as_str());
    // Plain text and nothing else: the same text with HTML or an image
    // alongside is someone else's copy.
    if !ours || before != after || after.concealed || !after.plain_text {
        return Err(HelperError::new(
            HelperErrorCode::ActionFailed,
            "The text was put on the clipboard, but something else was copied before it could \
             be checked, so nothing on it is held as yours: write it again before pasting.",
        ));
    }
    Ok(after.value)
}

/// The array `key` of a successful answer. Missing is a malformed answer, not
/// an empty one: an application list read as empty would leave every window
/// without the application that names it.
fn required_array<'a>(tool: &str, value: &'a Value, key: &str) -> Result<&'a [Value], HelperError> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| HelperError::failed(format!("{tool} answered without `{key}`")))
}

#[cfg(any(test, not(any(target_os = "macos", windows))))]
fn parse_apps(value: &Value) -> Result<Vec<RawApp>, HelperError> {
    required_array("list_apps", value, "apps").map(|apps| {
        apps.iter()
            // The driver also lists installed applications that are not
            // running (pid 0); only running ones have windows.
            .filter(|a| a.get("running").and_then(Value::as_bool) == Some(true))
            .filter_map(|a| {
                let pid = u32::try_from(a.get("pid")?.as_u64()?)
                    .ok()
                    .filter(|p| *p > 0)?;
                Some(RawApp {
                    pid,
                    name: string(a, "name").unwrap_or_default(),
                    bundle_id: string(a, "bundle_id"),
                    path: string(a, "launch_path"),
                    active: a.get("active").and_then(Value::as_bool).unwrap_or(false),
                    started_at: process_start(pid),
                })
            })
            .collect()
    })
}

/// Remembers which application each running process is, so listing windows
/// does not re-list every installed application each time. Keyed by pid AND
/// start time: a reused pid is a cache miss, never a stale hit. Not used on
/// macOS or Windows, where each listing reads the owners afresh (see
/// [`list_windows`]).
#[derive(Default)]
pub struct AppCache {
    #[cfg_attr(any(target_os = "macos", windows), allow(dead_code))]
    apps: HashMap<(u32, Option<u64>), RawApp>,
}

impl AppCache {
    #[cfg(not(any(target_os = "macos", windows)))]
    fn lookup(&self, pid: u32, started_at: Option<u64>) -> Option<&RawApp> {
        self.apps.get(&(pid, started_at))
    }

    #[cfg(not(any(target_os = "macos", windows)))]
    fn refill(&mut self, apps: Vec<RawApp>) {
        self.apps = apps
            .into_iter()
            .map(|app| ((app.pid, app.started_at), app))
            .collect();
    }
}

/// Normal windows, each joined with its application.
pub async fn list_windows(
    driver: &DriverProc,
    cache: &tokio::sync::Mutex<AppCache>,
    pid: Option<u32>,
) -> Result<Vec<RawWindow>, HelperError> {
    let mut args = json!({ "on_screen_only": false });
    if let Some(pid) = pid {
        args["pid"] = json!(pid);
    }
    let result = call(driver, "list_windows", args, LIST_TIMEOUT).await?;
    let windows = parse_windows(structured("list_windows", &result)?)?;

    let stamps: Vec<Option<u64>> = windows.iter().map(|w| process_start(w.pid)).collect();
    #[cfg(target_os = "macos")]
    {
        let _ = cache;
        Ok(join_identified(windows, stamps))
    }
    // Naming a packaged application the first time can take the Start menu
    // a fifth of a second: off the runtime's two threads.
    #[cfg(windows)]
    {
        let _ = cache;
        tokio::task::spawn_blocking(move || join_identified(windows, stamps))
            .await
            .map_err(|e| HelperError::failed(format!("the windows' owners could not be read: {e}")))
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let mut cache = cache.lock().await;
        let missing = windows
            .iter()
            .zip(&stamps)
            .any(|(w, started)| cache.lookup(w.pid, *started).is_none());
        if missing {
            cache.refill(driver_apps(driver).await?);
            // A process the application list does not know — a background
            // helper, an agent's own window — keeps the name the window list
            // gave it and has no key a blocklist could match. Remembered like
            // any other, so it does not send every later listing back to the
            // slow application list.
            for (window, started_at) in windows.iter().zip(&stamps) {
                cache
                    .apps
                    .entry((window.pid, *started_at))
                    .or_insert_with(|| RawApp {
                        started_at: *started_at,
                        ..window.app.clone()
                    });
            }
        }
        Ok(windows
            .into_iter()
            .zip(stamps)
            .map(|(mut window, started_at)| {
                if let Some(app) = cache.lookup(window.pid, started_at) {
                    window.app = app.clone();
                }
                window.app.started_at = started_at;
                window
            })
            .collect())
    }
}

/// The driver's list of running applications (Linux, where it is read afresh
/// on every call).
#[cfg(not(any(target_os = "macos", windows)))]
async fn driver_apps(driver: &DriverProc) -> Result<Vec<RawApp>, HelperError> {
    let result = call(driver, "list_apps", json!({}), LIST_TIMEOUT).await?;
    parse_apps(structured("list_apps", &result)?)
}

/// macOS: join each window with its application as the helper reads it off
/// the owning process now (`appident`) — once per process per listing, and
/// never remembered past it: reading it is a few system calls and a small
/// file, and a process that has since run another program (`exec` keeps the
/// pid and the start time) is that program now.
#[cfg(target_os = "macos")]
fn join_identified(windows: Vec<RawWindow>, stamps: Vec<Option<u64>>) -> Vec<RawWindow> {
    let mut seen: HashMap<(u32, Option<u64>), RawApp> = HashMap::new();
    windows
        .into_iter()
        .zip(stamps)
        .map(|(mut window, started_at)| {
            window.app = seen
                .entry((window.pid, started_at))
                .or_insert_with(|| identified(window.pid, started_at, &window.app.name))
                .clone();
            window
        })
        .collect()
}

/// The application `pid` runs, read off the process and then checked to be
/// the process the window list named — a pid reused in between would lend
/// the window another application's identity — and named as the Finder names
/// it (`owner` is the window list's name for the process; see `appident`).
/// Unidentified (no bundle, no path, the window list's name) when it is no
/// application, or when that cannot be told.
#[cfg(target_os = "macos")]
pub(super) fn identified(pid: u32, started_at: Option<u64>, owner: &str) -> RawApp {
    let identity = started_at
        .and_then(|_| crate::computer::appident::identify(pid))
        .filter(|_| process_start(pid) == started_at);
    let unidentified = RawApp {
        pid,
        name: owner.to_string(),
        bundle_id: None,
        path: None,
        active: false,
        started_at,
    };
    match identity {
        Some(identity) => RawApp {
            name: identity.name(owner),
            bundle_id: Some(identity.bundle_id),
            path: Some(identity.path),
            ..unidentified
        },
        None => unidentified,
    }
}

/// Windows: join each window with its application, and say of it what the
/// listing leaves unsaid (see `super::hwnd`): a window the compositor hides
/// is off the screen — on another virtual desktop, or out of sight on this
/// one; one the system will not place stays as the listing had it. What the
/// system says of a window counts only while its handle is still the listed
/// process's: a window closed since, its handle handed on, says nothing of
/// the one listed.
///
/// The application is the owning process's executable, read off the process
/// with its start time through one handle, which must still be the start time
/// the window list's owner had (a pid reused in between would lend the window
/// another application's identity) — or, for a frame, the executable of the
/// process drawing inside it, read the same way. Each is named as Windows
/// names it to the person (see `appident`), and read once per listing.
/// Unidentified (no path, the window list's name) when that cannot be read,
/// or when the process runs no application: a host other than the frame
/// host, or one of the system's own agents.
#[cfg(windows)]
pub(super) fn join_identified(windows: Vec<RawWindow>, stamps: Vec<Option<u64>>) -> Vec<RawWindow> {
    use crate::computer::appident::{windows_application, windows_owner, WindowsApp, WindowsOwner};
    use crate::computer::protocol::ProcessRun;

    let desktop = super::hwnd::Desktop::open();
    let mut owners: HashMap<(u32, u64), WindowsOwner> = HashMap::new();
    let mut contents: HashMap<ProcessRun, Option<WindowsApp>> = HashMap::new();
    windows
        .into_iter()
        .zip(stamps)
        .map(|(mut window, started_at)| {
            let held = desktop.owner(window.window_id) == Some(window.pid);
            if held && desktop.cloaked(window.window_id) {
                if let Some(here) = desktop.on_current_desktop(window.window_id) {
                    window.on_screen = false;
                    window.on_current_space = Some(here);
                }
            }
            let owner = started_at.map(|started_at| {
                owners
                    .entry((window.pid, started_at))
                    .or_insert_with(|| windows_owner(window.pid, started_at))
                    .clone()
            });
            let app = match owner {
                Some(WindowsOwner::Application(app)) => Some(app),
                Some(WindowsOwner::FrameHost) if held => {
                    window.content = desktop.frame_content(window.window_id, window.pid);
                    window.content.and_then(|run| {
                        contents
                            .entry(run)
                            .or_insert_with(|| windows_application(run.pid, run.started_at))
                            .clone()
                    })
                }
                _ => None,
            };
            let (name, path) = match app {
                Some(app) => (app.name, Some(app.path)),
                None => (std::mem::take(&mut window.app.name), None),
            };
            window.app = RawApp {
                pid: window.pid,
                name,
                bundle_id: None,
                path,
                active: false,
                started_at,
            };
            window
        })
        .collect()
}

/// The windows in a `list_windows` answer that could be someone's window at
/// all: the normal layer, with an area. Visible or not — whether a window is
/// worth *showing* is codeg's call, made after it has matched the listing
/// against the windows it has already named. A shared window whose
/// application is hidden (⌘H) is off screen and still the same window, and a
/// listing that dropped it would read as the window closing and end the
/// grant.
pub(super) fn parse_windows(value: &Value) -> Result<Vec<RawWindow>, HelperError> {
    let flag = |w: &Value, key: &str| w.get(key).and_then(Value::as_bool);
    required_array("list_windows", value, "windows").map(|windows| {
        windows
            .iter()
            .filter(|w| w.get("layer").and_then(Value::as_i64).unwrap_or(0) == 0)
            .filter_map(|w| {
                let bounds = rect(w.get("bounds")?)?;
                if bounds.is_empty() {
                    return None;
                }
                let pid = u32::try_from(w.get("pid")?.as_u64()?).ok()?;
                Some(RawWindow {
                    window_id: w.get("window_id")?.as_u64()?,
                    pid,
                    title: string(w, "title").unwrap_or_default(),
                    bounds,
                    on_screen: flag(w, "is_on_screen").unwrap_or(false),
                    minimized: flag(w, "minimized"),
                    hidden: None,
                    on_current_space: flag(w, "on_current_space"),
                    z_index: w.get("z_index").and_then(Value::as_i64),
                    content: None,
                    app: RawApp {
                        pid,
                        name: string(w, "app_name").unwrap_or_default(),
                        bundle_id: None,
                        path: None,
                        active: false,
                        started_at: None,
                    },
                })
            })
            .collect()
    })
}

/// macOS: mark which of `windows` are minimized, and which belong to a
/// hidden (⌘H) application — neither of which the driver's listing says.
/// There such a window is only off screen, as are the hidden windows
/// applications keep — so codeg, which lists the one and not the other, would
/// list neither. Accessibility tells them apart (see `super::axwin`), and is
/// asked about those applications alone ([`owners_to_ask`]). Only called while
/// the helper may ask it.
#[cfg(target_os = "macos")]
pub async fn mark_out_of_sight(windows: &mut [RawWindow]) {
    let (listed, maybe) = owners_to_ask(windows);
    if listed.is_empty() && maybe.is_empty() {
        return;
    }
    let said = super::axwin::window_states(listed, maybe).await;
    settle(windows, &said);
}

/// X11: mark which of `windows` the window manager has minimized (iconified),
/// and which are on another of its desktops — neither of which the driver's
/// listing says: to it all of them are only not mapped.
#[cfg(all(target_os = "linux", feature = "computer-helper"))]
pub async fn mark_out_of_sight(windows: &mut [RawWindow]) {
    let open: Vec<u64> = windows
        .iter()
        .filter(|w| !w.on_screen && w.minimized.is_none())
        .map(|w| w.window_id)
        .collect();
    if open.is_empty() {
        return;
    }
    let said = tokio::task::spawn_blocking(move || super::x11win::window_states(&open))
        .await
        .unwrap_or_default();
    for window in windows.iter_mut() {
        if let Some(state) = said.get(&window.window_id) {
            window.minimized = Some(state.minimized);
            if state.elsewhere {
                window.on_current_space = Some(false);
            }
        }
    }
}

/// What one application says of its windows (macOS, see `super::axwin`).
#[cfg(any(test, target_os = "macos"))]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AppWindows {
    /// Whether the application is hidden (⌘H); `None` when it would not say.
    pub hidden: Option<bool>,
    /// Each window it lists, by window id: whether it is minimized — `None`
    /// for one that would not say. Empty when its windows were not asked
    /// for.
    pub minimized: HashMap<u64, Option<bool>>,
}

/// Whether the listing leaves open if `window` is minimized when it could be:
/// off screen, yet on a Space. (One on screen is not; one on no Space at all
/// is the furniture every application keeps.)
#[cfg(any(test, target_os = "macos"))]
fn unexplained(window: &RawWindow) -> bool {
    !window.on_screen && window.minimized.is_none() && window.on_current_space.is_some()
}

/// Which applications Accessibility is asked about, and how much: those
/// with a window [`unexplained`], about all their windows (`listed`); those
/// with nothing on the screen and a window off it, only whether they are
/// hidden — and their windows only if they are (`maybe`): a hidden
/// application's windows may be on no Space, like the furniture every
/// application keeps.
#[cfg(any(test, target_os = "macos"))]
fn owners_to_ask(
    windows: &[RawWindow],
) -> (
    std::collections::BTreeSet<u32>,
    std::collections::BTreeSet<u32>,
) {
    let listed: std::collections::BTreeSet<u32> = windows
        .iter()
        .filter(|w| unexplained(w))
        .map(|w| w.pid)
        .collect();
    let showing: std::collections::BTreeSet<u32> = windows
        .iter()
        .filter(|w| w.on_screen)
        .map(|w| w.pid)
        .collect();
    let maybe = windows
        .iter()
        .filter(|w| !w.on_screen && w.minimized.is_none())
        .map(|w| w.pid)
        .filter(|pid| !showing.contains(pid) && !listed.contains(pid))
        .collect();
    (listed, maybe)
}

/// Write down what each application said of its windows (`said`, by pid) for
/// the windows the listing left open — off screen, and not said to be
/// minimized or not. A window its application did not list stays unsaid: not
/// one a person can bring up. One on no Space is written down only when its
/// application is hidden — otherwise it is furniture, whatever is said of it.
#[cfg(any(test, target_os = "macos"))]
fn settle(windows: &mut [RawWindow], said: &HashMap<u32, AppWindows>) {
    for window in windows
        .iter_mut()
        .filter(|w| !w.on_screen && w.minimized.is_none())
    {
        let Some(app) = said.get(&window.pid) else {
            continue;
        };
        let Some(minimized) = app.minimized.get(&window.window_id) else {
            continue;
        };
        let hidden = app.hidden == Some(true);
        if window.on_current_space.is_none() && !hidden {
            continue;
        }
        window.minimized = *minimized;
        if hidden {
            window.hidden = Some(true);
        }
    }
}

/// The answer to a screenshot of a window that is minimized, or whose
/// application is hidden. Such a window shows nothing to capture, and
/// whatever a capture gave would not be what it shows when it is back.
#[cfg(target_os = "macos")]
pub fn out_of_sight_capture(why: super::axwin::OutOfSight) -> HelperError {
    let what = match why {
        super::axwin::OutOfSight::Minimized => "The window is minimized",
        super::axwin::OutOfSight::AppHidden => "The window's application is hidden",
    };
    HelperError::new(
        HelperErrorCode::Occluded,
        format!(
            "{what}, so there is no picture of it to take. computer_snapshot still reads it as it \
             is, and actions by ref still reach it. To see it, it has to be back on the screen: \
             computer_restore does that if it is shared with you for control; otherwise ask the \
             user to bring it back."
        ),
    )
}

/// A screenshot of one window, and nothing around it: the driver captures the
/// window's own pixels, so nothing of the windows beside or under it can end
/// up in the image.
///
/// Always captured at the window's own size — the driver is configured with
/// no ceiling, and asked for none (see `driver_proc`'s module note: a capture
/// at any other size would change how the driver maps every later click's
/// coordinates) — and shrunk here to `max_dimension`.
///
/// Taken without making it the window's snapshot wherever that can be done.
/// The driver keeps one snapshot of each window, and a capture taken as one
/// replaces the snapshot the agent's refs name, leaving nothing for them to
/// name. On macOS and Windows `verify_state` reads the window that way; the
/// window's bounds and title then come from its listing — read before the
/// capture and after it, and a point is not aimed by a capture the window
/// changed size around — and the capture's scale from its size against them,
/// as the driver reckons it. On Linux that
/// read is cut down to a model-sized image, so the capture is a snapshot of
/// its own there, at the window's size, and replaces the window's snapshot
/// ([`capture_replaces_snapshot`]); one the driver took from the screen
/// rather than from the window — for a pop-up of the application over it —
/// could hold other windows, and is not handed on.
pub async fn capture(
    driver: &DriverProc,
    pid: u32,
    window_id: u64,
    max_dimension: Option<u32>,
) -> Result<RawCapture, HelperError> {
    let taken = take_capture(driver, pid, window_id).await?;
    let data = taken.png_base64;
    let shrunk = tokio::task::spawn_blocking(move || shrink_png(&data, max_dimension))
        .await
        .map_err(|e| HelperError::failed(format!("the capture could not be scaled: {e}")))?
        .map_err(|e| HelperError::failed(format!("the capture could not be scaled: {e}")))?;
    let scale = taken.scale.unwrap_or_else(|| {
        reckoned_scale(
            shrunk.native_width,
            &taken.bounds,
            cfg!(target_os = "macos"),
        )
    });
    let full_size = driver.full_size_captures()
        && taken.steady
        && is_whole_window(
            shrunk.native_width,
            shrunk.native_height,
            &taken.bounds,
            scale,
        );
    Ok(RawCapture {
        png_base64: shrunk.png_base64,
        width: shrunk.width,
        height: shrunk.height,
        native_width: shrunk.native_width,
        native_height: shrunk.native_height,
        full_size,
        window_bounds: taken.bounds,
        title: taken.title,
    })
}

/// Give the driver a capture of the window to aim points by, without a walk
/// of its tree: a snapshot holding the capture alone, which takes the place
/// of the window's snapshot. `Ok(true)`: it did, so the refs from the one
/// before name nothing any more. `Ok(false)`: the window could not be
/// captured, and its snapshot is as it was.
pub async fn publish_capture(
    driver: &DriverProc,
    pid: u32,
    window_id: u64,
) -> Result<bool, HelperError> {
    let args = json!({
        "pid": pid,
        "window_id": window_id,
        "include_screenshot": true,
        "include_accessibility_tree": false,
        "max_image_dimension": 0,
    });
    let result = call(driver, "get_window_state", args, WINDOW_STATE_TIMEOUT).await?;
    Ok(result.image().is_some())
}

/// Whether capturing a window replaces the driver's snapshot of it — the one
/// its refs name: only on Linux (see [`capture`]).
pub fn capture_replaces_snapshot() -> bool {
    cfg!(target_os = "linux")
}

/// What a capture brought back, before it is scaled.
struct Taken {
    png_base64: String,
    bounds: Rect,
    /// The backing scale the driver said the capture was taken at; `None`
    /// where it said nothing of it.
    scale: Option<f64>,
    title: Option<String>,
    /// Whether `bounds` are the window's as it was captured: read with the
    /// capture, or the same before it and after it.
    steady: bool,
}

/// macOS and Windows: one read through `verify_state`, which leaves the
/// window's snapshot as it was, between two of the window's listings.
#[cfg(not(target_os = "linux"))]
async fn take_capture(driver: &DriverProc, pid: u32, window_id: u64) -> Result<Taken, HelperError> {
    let before = super::act::listed(driver, pid, window_id)
        .await?
        .get("bounds")
        .and_then(rect);
    let args = json!({
        "pid": pid,
        "window_id": window_id,
        "expect": [{ "window": { "exists": true } }],
        "timeout_ms": 0,
        "stable_samples": 1,
        "include_screenshot": true,
    });
    let result = call(driver, "verify_state", args, WINDOW_STATE_TIMEOUT).await?;
    let Some((data, mime)) = result.image() else {
        // `verify_state` says nothing of why there is no picture; the
        // listing says whether the window is still there.
        super::act::listed(driver, pid, window_id).await?;
        return Err(HelperError::failed(
            "the window could not be captured: no image came back",
        ));
    };
    if mime != "image/png" {
        return Err(HelperError::failed(format!(
            "the capture came back as {mime}"
        )));
    }
    let png_base64 = data.to_string();
    let window = super::act::listed(driver, pid, window_id).await?;
    let bounds = window.get("bounds").and_then(rect);
    Ok(Taken {
        png_base64,
        steady: same_size(before.as_ref(), bounds.as_ref()),
        bounds: bounds.unwrap_or_default(),
        scale: None,
        title: string(&window, "title"),
    })
}

/// Whether a window listed with `before` and then `after` kept its size —
/// within a pixel, as a point's own check allows (`act::check_points`).
#[cfg(any(not(target_os = "linux"), test))]
fn same_size(before: Option<&Rect>, after: Option<&Rect>) -> bool {
    match (before, after) {
        (Some(before), Some(after)) => {
            (before.width - after.width).abs() <= 1.0 && (before.height - after.height).abs() <= 1.0
        }
        _ => false,
    }
}

/// Linux: a `get_window_state` at the window's own size, which becomes the
/// window's snapshot.
#[cfg(target_os = "linux")]
async fn take_capture(driver: &DriverProc, pid: u32, window_id: u64) -> Result<Taken, HelperError> {
    let args = json!({
        "pid": pid,
        "window_id": window_id,
        "include_screenshot": true,
        "include_accessibility_tree": false,
        "max_image_dimension": 0,
    });
    let result = call(driver, "get_window_state", args, WINDOW_STATE_TIMEOUT).await?;
    let meta = structured("get_window_state", &result)?;
    if meta.get("screenshot_composited").and_then(Value::as_bool) == Some(true) {
        return Err(composited_capture());
    }
    let Some((data, mime)) = result.image() else {
        // The capture half failed and the driver said why beside an
        // otherwise successful answer.
        let why = meta
            .pointer("/screenshot_error/reason")
            .or_else(|| meta.get("screenshot_error"))
            .map(|e| e.to_string())
            .unwrap_or_else(|| "no image came back".to_string());
        return Err(HelperError::failed(format!(
            "the window could not be captured: {why}"
        )));
    };
    if mime != "image/png" {
        return Err(HelperError::failed(format!(
            "the capture came back as {mime}"
        )));
    }
    Ok(Taken {
        png_base64: data.to_string(),
        steady: true,
        bounds: meta.get("window_bounds").and_then(rect).unwrap_or_default(),
        scale: meta
            .get("screenshot_scale")
            .and_then(Value::as_f64)
            .filter(|s| s.is_finite() && *s > 0.0),
        title: string(meta, "window_title"),
    })
}

/// A Linux capture the driver took from the screen, not from the window.
#[cfg(target_os = "linux")]
fn composited_capture() -> HelperError {
    HelperError::new(
        HelperErrorCode::Occluded,
        "A pop-up of the window's application is over it, and a picture of it now would be \
         taken from the screen, where other windows could be in it — so none was taken. Close \
         the pop-up, or wait for it to close, then take the screenshot again.",
    )
}

/// The scale of a capture `width` pixels wide of a window whose bounds are
/// `bounds`, as the driver reckons it. In points (`points`: macOS) a window
/// is captured at 1× or 2× — whichever its size is nearer; elsewhere bounds
/// are in the pixels captured.
fn reckoned_scale(width: u32, bounds: &Rect, points: bool) -> f64 {
    if !points || bounds.width <= 0.0 {
        return 1.0;
    }
    let ratio = f64::from(width) / bounds.width;
    if (ratio - 1.0).abs() <= (ratio - 2.0).abs() {
        1.0
    } else {
        2.0
    }
}

/// A capture, shrunk to the size asked for.
#[derive(Debug)]
pub(super) struct Shrunk {
    pub png_base64: String,
    pub width: u32,
    pub height: u32,
    pub native_width: u32,
    pub native_height: u32,
}

/// Decode `png_base64`, and if its long edge is over `max_dimension`, scale it
/// down to that (aspect kept, never up) and encode it again. An image already
/// within bounds goes back byte for byte.
pub(super) fn shrink_png(png_base64: &str, max_dimension: Option<u32>) -> Result<Shrunk, String> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use image::{imageops::FilterType, ImageFormat};

    let bytes = STANDARD
        .decode(png_base64)
        .map_err(|e| format!("not base64: {e}"))?;
    let image = image::load_from_memory_with_format(&bytes, ImageFormat::Png)
        .map_err(|e| format!("not a PNG: {e}"))?;
    let (native_width, native_height) = (image.width(), image.height());
    let long_edge = native_width.max(native_height);
    let max = max_dimension.filter(|m| *m > 0).unwrap_or(u32::MAX);
    if long_edge <= max {
        return Ok(Shrunk {
            png_base64: png_base64.to_string(),
            width: native_width,
            height: native_height,
            native_width,
            native_height,
        });
    }
    let factor = f64::from(max) / f64::from(long_edge);
    let width = ((f64::from(native_width) * factor).round() as u32).max(1);
    let height = ((f64::from(native_height) * factor).round() as u32).max(1);
    let resized = image.resize_exact(width, height, FilterType::Triangle);
    let mut out = std::io::Cursor::new(Vec::new());
    resized
        .write_to(&mut out, ImageFormat::Png)
        .map_err(|e| format!("png encoding failed: {e}"))?;
    Ok(Shrunk {
        png_base64: STANDARD.encode(out.into_inner()),
        width,
        height,
        native_width,
        native_height,
    })
}

/// Whether a capture of `width` × `height` pixels is the whole window at its
/// own resolution: the window's bounds times the backing scale, give or take
/// the frame the platforms crop differently (a few pixels, or a few percent).
/// A capture the driver had shrunk would be far smaller.
fn is_whole_window(width: u32, height: u32, bounds: &Rect, scale: f64) -> bool {
    if bounds.is_empty() {
        return false;
    }
    let near = |got: u32, expected: f64| {
        let got = f64::from(got);
        let slack = (expected * 0.05).max(16.0);
        (got - expected).abs() <= slack
    };
    near(width, bounds.width * scale) && near(height, bounds.height * scale)
}

/// A window's accessibility tree, with the values of anything that looks like
/// a secret taken out — and, for the helper to hold on to, what it knows of
/// each element that can be acted on (see [`SnapshotFacts`]).
///
/// It becomes the window's snapshot in the driver, which aims a point only
/// for a window whose snapshot holds a capture of it: so the window is
/// captured too, at its own size, and the picture thrown away. Where it
/// cannot be captured (no Screen Recording, or out of sight) the tree comes
/// back all the same. The walk has [`WALK_BUDGET_MS`]; past it the tree comes
/// back cut short, and says so.
pub async fn snapshot(
    driver: &DriverProc,
    pid: u32,
    window_id: u64,
    max_depth: Option<u32>,
    max_elements: Option<u32>,
    query: Option<String>,
    app_menus: bool,
) -> Result<(RawSnapshot, Option<SnapshotFacts>), HelperError> {
    let mut args = json!({
        "pid": pid,
        "window_id": window_id,
        "include_screenshot": true,
        "include_accessibility_tree": true,
        "max_image_dimension": 0,
        "timeout_ms": WALK_BUDGET_MS,
    });
    if let Some(depth) = max_depth.filter(|d| *d > 0) {
        args["max_depth"] = json!(depth);
    }
    if let Some(elements) = max_elements.filter(|e| *e > 0) {
        args["max_elements"] = json!(elements);
    }
    if let Some(query) = query.filter(|q| !q.trim().is_empty()) {
        args["query"] = json!(query);
    }
    let result = call(driver, "get_window_state", args, WINDOW_STATE_TIMEOUT).await?;
    let meta = structured("get_window_state", &result)?;
    let tree = meta
        .get("tree_markdown")
        .and_then(Value::as_str)
        .ok_or_else(|| HelperError::failed("get_window_state answered without a tree"))?;
    // Without its application's menu bars unless the application is shared
    // as a whole — and then without the Apple menu and the application menu:
    // not shown, and nothing in them can be acted on. By where the tree puts
    // them, and — for a window shared on its own — by their roles wherever
    // it puts them.
    let protected = if app_menus {
        protected_menu_titles(pid).await
    } else {
        None
    };
    // Where the titles cannot be had, nothing of the menu bars is kept.
    let keep = match &protected {
        Some(protected) => AppMenus::Own { protected },
        None => AppMenus::Withheld,
    };
    let (kept, withheld) = without_app_menus(tree, Dialect::current(), keep);
    let redacted = redact_secrets(&kept);
    let listed = meta.get("elements").map(|elements| {
        if Dialect::current() != Dialect::Mac {
            return elements.clone();
        }
        let in_reach = |element: &&Value| {
            let index = element.get("element_index").and_then(Value::as_u64);
            let role = element.get("role").and_then(Value::as_str).unwrap_or("");
            !index.is_some_and(|i| u32::try_from(i).is_ok_and(|i| withheld.contains(&i)))
                && (keep != AppMenus::Withheld || !APP_MENU_ROLES.contains(&role))
        };
        Value::Array(
            elements
                .as_array()
                .into_iter()
                .flatten()
                .filter(in_reach)
                .cloned()
                .collect(),
        )
    });
    // No id when the driver kept no snapshot of the window (it could not
    // match its accessibility surface): the tree is still worth reading, and
    // nothing in it can be acted on.
    let snapshot_id = string(meta, "snapshot_id");
    let (refs, elements) = element_refs(&redacted.nodes, listed.as_ref());
    let facts = snapshot_id.clone().map(|id| SnapshotFacts {
        snapshot_id: id,
        elements,
    });
    let raw = RawSnapshot {
        tree: redacted.tree,
        element_count: meta
            .get("element_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        truncated: meta
            .get("truncated")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            || tree.contains("AX tree truncated"),
        degraded: string(meta, "degraded_reason"),
        window_bounds: meta.get("window_bounds").and_then(rect),
        title: string(meta, "window_title"),
        snapshot_id,
        refs,
    };
    Ok((raw, facts))
}

/// The titles of the Apple menu and the application menu of `pid` (macOS):
/// what an application shared as a whole still does not reach.
#[cfg(target_os = "macos")]
async fn protected_menu_titles(pid: u32) -> Option<[String; 2]> {
    super::axwin::protected_menu_titles(pid).await
}

/// Elsewhere a window's menus are its own (see `without_app_menus`).
#[cfg(not(target_os = "macos"))]
async fn protected_menu_titles(_pid: u32) -> Option<[String; 2]> {
    None
}

/// The elements of a snapshot that can be acted on, and what is known of each.
///
/// The driver's structured `elements` are the authority on which elements
/// exist and what their roles are; the tree lines only say where each is and
/// how the redaction judged it. The two are joined by index, and the tree is
/// not trusted on its own: the driver writes values unescaped, so a line of
/// some field's text can look exactly like `- [7] AXButton "OK"`. So:
///
/// * a ref is offered only for an element the driver listed whose index
///   heads exactly one tree line — an index that heads two (one of them
///   forged by a value) cannot be told apart, and is offered for neither;
/// * an element is secret if ANY line carrying its index was judged secret,
///   or its own role, label or value say so — a forged line can add secrecy,
///   never take it away.
///
/// Whether an element is in web content is the driver's own marker on it
/// (`in_web_content`, macOS): a web area above it — the same judgment by
/// which the driver distrusts what typing there reads back.
fn element_refs(
    nodes: &[TreeNode],
    elements: Option<&Value>,
) -> (Vec<SnapshotRef>, HashMap<u32, ElementFacts>) {
    let mut lines: HashMap<u32, Vec<&TreeNode>> = HashMap::new();
    for node in nodes {
        lines.entry(node.index).or_default().push(node);
    }
    let mut refs = Vec::new();
    let mut facts = HashMap::new();
    for element in elements.and_then(Value::as_array).into_iter().flatten() {
        let Some(index) = element
            .get("element_index")
            .and_then(Value::as_u64)
            .and_then(|i| u32::try_from(i).ok())
        else {
            continue;
        };
        let text = |key: &str| element.get(key).and_then(Value::as_str).unwrap_or("");
        let role = text("role");
        let own_lines = lines.get(&index).map(Vec::as_slice).unwrap_or(&[]);
        let secret = own_lines.iter().any(|n| n.secret)
            || names_a_secret(&format!("{role} {}", text("label")))
            || is_masked(text("value"))
            // An index the tree cannot place is not trusted to be harmless.
            || own_lines.len() > 1;
        if let [line] = own_lines {
            refs.push(SnapshotRef {
                index,
                offset: line.offset,
                secret,
            });
        }
        facts.insert(
            index,
            ElementFacts {
                role: role.to_string(),
                secret,
                paste: names_a_paste_control(role, text("label")),
                web: element.get("in_web_content").and_then(Value::as_bool) == Some(true),
                frame: element_frame(element),
            },
        );
    }
    refs.sort_by_key(|r| r.offset);
    (refs, facts)
}

/// Whether an element is a menu command or a button named for pasting: one
/// pressed writes the person's clipboard into the window, as ⌘V / Ctrl+V
/// does. Roles as the three platforms' trees spell them.
fn names_a_paste_control(role: &str, label: &str) -> bool {
    // Exactly these: a tab is a radio button, and "Pastebin" one to select.
    const PRESSED: &[&str] = &[
        "axmenuitem",
        "axbutton",
        "menuitem",
        "button",
        "splitbutton",
        "menu item",
        "push button",
    ];
    PRESSED.contains(&role.to_ascii_lowercase().as_str())
        && crate::computer::keys::names_paste(label)
}

/// The driver's `frame` of one element (`{x, y, w, h}`, in desktop units),
/// when it gave a whole one with an area.
fn element_frame(element: &Value) -> Option<Rect> {
    let frame = element.get("frame")?;
    let number = |key: &str| {
        frame
            .get(key)
            .and_then(Value::as_f64)
            .filter(|n| n.is_finite())
    };
    let rect = Rect {
        x: number("x")?,
        y: number("y")?,
        width: number("w")?,
        height: number("h")?,
    };
    (!rect.is_empty()).then_some(rect)
}

/// Rebuild the caller's predicates in the driver's vocabulary. Every field is
/// copied by name from the closed types in `computer::types`, so nothing the
/// agent wrote reaches the driver unexamined.
pub fn driver_predicates(expect: &[VerifyPredicate]) -> Result<Vec<Value>, HelperError> {
    if expect.is_empty() || expect.len() > MAX_VERIFY_PREDICATES {
        return Err(HelperError::new(
            HelperErrorCode::BadRequest,
            format!("verify takes 1 to {MAX_VERIFY_PREDICATES} predicates"),
        ));
    }
    expect
        .iter()
        .map(|p| {
            let mut out = Map::new();
            if let Some(window) = &p.window {
                let mut w = Map::new();
                if let Some(exists) = window.exists {
                    w.insert("exists".into(), json!(exists));
                }
                if let Some(b) = &window.bounds {
                    let mut bounds = json!({ "x": b.x, "y": b.y, "width": b.width, "height": b.height });
                    if let Some(t) = b.tolerance_px {
                        bounds["tolerance_px"] = json!(t.clamp(0.0, 100.0));
                    }
                    w.insert("bounds".into(), bounds);
                }
                out.insert("window".into(), Value::Object(w));
            }
            if let Some(element) = &p.element {
                if element.exists == Some(false) {
                    return Err(HelperError::new(
                        HelperErrorCode::BadRequest,
                        "an element's absence cannot be proven; check `exists: true` or leave it out",
                    ));
                }
                let mut selector = Map::new();
                if let Some(role) = element.selector.role.as_deref().filter(|s| !s.is_empty()) {
                    selector.insert("role".into(), json!(role));
                }
                if let Some(label) = element.selector.label_contains.as_deref().filter(|s| !s.is_empty()) {
                    selector.insert("label_contains".into(), json!(label));
                }
                let mut e = Map::new();
                e.insert("selector".into(), Value::Object(selector));
                for (key, value) in [
                    ("exists", element.exists),
                    ("enabled", element.enabled),
                    ("selected", element.selected),
                ] {
                    if let Some(v) = value {
                        e.insert(key.into(), json!(v));
                    }
                }
                out.insert("element".into(), Value::Object(e));
            }
            if out.is_empty() {
                return Err(HelperError::new(
                    HelperErrorCode::BadRequest,
                    "every predicate needs a `window` or an `element` part",
                ));
            }
            Ok(Value::Object(out))
        })
        .collect()
}

fn verify_status(value: Option<&Value>) -> VerifyStatus {
    match value.and_then(Value::as_str) {
        Some("satisfied") => VerifyStatus::Satisfied,
        Some("unsatisfied") => VerifyStatus::Unsatisfied,
        _ => VerifyStatus::Unknown,
    }
}

pub async fn verify(
    driver: &DriverProc,
    pid: u32,
    window_id: u64,
    request: &VerifyRequest,
) -> Result<RawVerify, HelperError> {
    let expect = driver_predicates(&request.expect)?;
    let timeout_ms = request
        .timeout_ms
        .unwrap_or(5_000)
        .min(MAX_VERIFY_TIMEOUT_MS);
    let mut args = json!({
        "pid": pid,
        "window_id": window_id,
        "expect": expect,
        "timeout_ms": timeout_ms,
        "include_screenshot": false,
    });
    if let Some(samples) = request.stable_samples {
        args["stable_samples"] = json!(samples.clamp(1, MAX_STABLE_SAMPLES));
    }
    let budget = Duration::from_millis(u64::from(timeout_ms)) + VERIFY_OVERHEAD;
    let result = call(driver, "verify_state", args, budget).await?;
    let meta = structured("verify_state", &result)?;
    Ok(parse_verify(meta))
}

/// The verdicts, without the observed values the driver reports beside them.
fn parse_verify(meta: &Value) -> RawVerify {
    RawVerify {
        status: verify_status(meta.get("status")),
        stable: meta.get("stable").and_then(Value::as_bool).unwrap_or(false),
        samples: meta.get("samples").and_then(Value::as_u64).unwrap_or(0),
        elapsed_ms: meta.get("elapsed_ms").and_then(Value::as_u64).unwrap_or(0),
        predicates: meta
            .get("predicates")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .map(|p| PredicateResult {
                        index: p
                            .get("index")
                            .and_then(Value::as_u64)
                            .and_then(|v| u32::try_from(v).ok())
                            .unwrap_or(0),
                        status: verify_status(p.get("status")),
                        unknown_reason: string(p, "unknown_reason"),
                    })
                    .collect()
            })
            .unwrap_or_default(),
    }
}
