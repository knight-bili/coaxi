// Apache-2.0; Codeg aff1e6c 原工具参数解析，不重写动作默认或边界。
use serde_json::Value;

// Fixed donor limits from acp/delegation/companion.rs; preserve the approved tool bounds.
const MAX_CLIPBOARD_TEXT: usize = 100_000;
const MAX_LAUNCH_NAME: usize = 512;
const MAX_MENU_PATH: usize = 16;

fn computer_target_id(arguments: &Value, tool: &str) -> Result<String, String> {
    arguments
        .get("targetId")
        .or_else(|| arguments.get("target_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            format!("{tool} requires a non-empty `targetId` string (from computer_list_windows)")
        })
}

fn computer_optional_u32(arguments: &Value, tool: &str, key: &str) -> Result<Option<u32>, String> {
    match arguments.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_u64()
            .or_else(|| v.as_f64().filter(|f| f.fract() == 0.0 && *f >= 0.0).map(|f| f as u64))
            .and_then(|n| u32::try_from(n).ok())
            .map(Some)
            .ok_or_else(|| format!("{tool}: `{key}` must be a whole non-negative number, not {v}")),
    }
}

pub fn computer_capture_request(arguments: &Value) -> Result<(String, Option<u32>), String> {
    let target_id = computer_target_id(arguments, "computer_screenshot")?;
    let max = computer_optional_u32(arguments, "computer_screenshot", "maxDimension")?;
    if max == Some(0) {
        return Err(
            "computer_screenshot: `maxDimension` must be at least 1; leave it out for the default"
                .to_string(),
        );
    }
    Ok((target_id, max))
}

pub fn computer_snapshot_request(
    arguments: &Value,
) -> Result<(String, crate::model_tools::SnapshotRequest), String> {
    let tool = "computer_snapshot";
    let target_id = computer_target_id(arguments, tool)?;
    let query = match arguments.get("query") {
        None | Some(Value::Null) => None,
        Some(Value::String(q)) => Some(q.clone()).filter(|q| !q.trim().is_empty()),
        Some(other) => return Err(format!("{tool}: `query` must be a string, not {other}")),
    };
    Ok((
        target_id,
        crate::model_tools::SnapshotRequest {
            max_chars: computer_optional_u32(arguments, tool, "maxChars")?.map(|n| n as usize),
            max_depth: computer_optional_u32(arguments, tool, "maxDepth")?.filter(|n| *n > 0),
            max_elements: computer_optional_u32(arguments, tool, "maxElements")?.filter(|n| *n > 0),
            query,
        },
    ))
}

pub fn computer_verify_request(
    arguments: &Value,
) -> Result<(String, crate::computer::types::VerifyRequest), String> {
    use crate::computer::types::{VerifyPredicate, VerifyRequest, MAX_VERIFY_PREDICATES};
    let tool = "computer_verify";
    let target_id = computer_target_id(arguments, tool)?;
    let expect: Vec<VerifyPredicate> = match arguments.get("expect") {
        Some(Value::Array(items)) => items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                serde_json::from_value::<VerifyPredicate>(item.clone())
                    .map_err(|e| format!("{tool}: `expect[{i}]` is not a predicate this tool knows: {e}"))
            })
            .collect::<Result<_, _>>()?,
        _ => {
            return Err(format!(
                "{tool} requires `expect`: an array of 1 to {MAX_VERIFY_PREDICATES} predicates"
            ))
        }
    };
    if expect.is_empty() || expect.len() > MAX_VERIFY_PREDICATES {
        return Err(format!(
            "{tool}: `expect` must hold 1 to {MAX_VERIFY_PREDICATES} predicates, not {}",
            expect.len()
        ));
    }
    Ok((
        target_id,
        VerifyRequest {
            expect,
            timeout_ms: computer_optional_u32(arguments, tool, "timeoutMs")?,
            stable_samples: computer_optional_u32(arguments, tool, "stableSamples")?,
        },
    ))
}

fn computer_ref(value: &Value, tool: &str) -> Result<u32, String> {
    let parsed = match value {
        Value::Number(n) => n.as_u64().and_then(|n| u32::try_from(n).ok()),
        Value::String(s) => s
            .trim()
            .trim_start_matches('[')
            .trim_end_matches(']')
            .parse::<u32>()
            .ok(),
        _ => None,
    };
    parsed.ok_or_else(|| {
        format!(
            "{tool}: `ref` must be an element's number from computer_snapshot (the N in `[N]`), \
             not {value}"
        )
    })
}

fn computer_coordinate(value: &Value, tool: &str) -> Result<(f64, f64), String> {
    let pair = value.as_array().filter(|a| a.len() == 2).and_then(|a| {
        let x = a[0].as_f64().filter(|v| v.is_finite() && *v >= 0.0)?;
        let y = a[1].as_f64().filter(|v| v.is_finite() && *v >= 0.0)?;
        Some((x, y))
    });
    pair.ok_or_else(|| {
        format!(
            "{tool}: `coordinate` must be [x, y], two non-negative numbers in the pixels of the \
             window's latest computer_screenshot, not {value}"
        )
    })
}


fn computer_generation(arguments: &Value, tool: &str, what: &str) -> Result<String, String> {
    arguments
        .get("generation")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            format!(
                "{tool}: a `{what}` needs the `generation` of the {} it came from",
                if what == "ref" {
                    "computer_snapshot"
                } else {
                    "computer_screenshot"
                }
            )
        })
}

fn computer_target(
    arguments: &Value,
    tool: &str,
) -> Result<Option<crate::computer::types::AgentTarget>, String> {
    use crate::computer::types::{AgentTarget, ElementTarget, PointTarget};
    let present = |k: &str| arguments.get(k).is_some_and(|v| !v.is_null());
    match (present("ref"), present("coordinate")) {
        (true, true) => Err(format!("{tool}: give `ref` or `coordinate`, not both")),
        (true, false) => Ok(Some(AgentTarget::Element(ElementTarget {
            index: computer_ref(&arguments["ref"], tool)?,
            generation: computer_generation(arguments, tool, "ref")?,
        }))),
        (false, true) => {
            let (x, y) = computer_coordinate(&arguments["coordinate"], tool)?;
            Ok(Some(AgentTarget::Point(PointTarget {
                x,
                y,
                generation: computer_generation(arguments, tool, "coordinate")?,
            })))
        }
        (false, false) => Ok(None),
    }
}

fn computer_modifiers(
    arguments: &Value,
    tool: &str,
) -> Result<crate::computer::keys::Modifiers, String> {
    use crate::computer::keys::Modifiers;
    match arguments.get("modifiers") {
        None | Some(Value::Null) => Ok(Modifiers::default()),
        Some(Value::Array(items)) => {
            let names = items
                .iter()
                .map(|v| {
                    v.as_str()
                        .map(str::to_string)
                        .ok_or_else(|| format!("{tool}: `modifiers` must be an array of names"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            Modifiers::parse(&names).map_err(|e| format!("{tool}: {e}"))
        }
        Some(other) => Err(format!(
            "{tool}: `modifiers` must be an array of names, not {other}"
        )),
    }
}

fn computer_clipboard_op(
    tool: &str,
    arguments: &Value,
) -> Result<crate::model_tools::ClipboardOp, String> {
    use crate::model_tools::ClipboardOp;
    let takes: &[&str] = if tool == "computer_clipboard_write" {
        &["text"]
    } else {
        &[]
    };
    if let Some(unknown) = arguments
        .as_object()
        .into_iter()
        .flat_map(|o| o.keys())
        .find(|k| !takes.contains(&k.as_str()))
    {
        return Err(format!("{tool} takes no argument `{unknown}`"));
    }
    if tool != "computer_clipboard_write" {
        return Ok(ClipboardOp::Read);
    }
    match arguments.get("text") {
        Some(Value::String(text)) if text.chars().count() <= MAX_CLIPBOARD_TEXT => {
            Ok(ClipboardOp::Write { text: text.clone() })
        }
        Some(Value::String(_)) => Err(format!(
            "{tool}: `text` may be at most {MAX_CLIPBOARD_TEXT} characters"
        )),
        _ => Err(format!("{tool} requires `text`, a string")),
    }
}

fn computer_launch_arguments(
    arguments: &Value,
) -> Result<(Option<String>, Option<String>), String> {
    const TOOL: &str = "computer_launch_app";
    if let Some(unknown) = arguments
        .as_object()
        .into_iter()
        .flat_map(|o| o.keys())
        .find(|k| !matches!(k.as_str(), "name" | "key"))
    {
        return Err(format!(
            "{TOOL} takes no argument `{unknown}`; it takes `name` or `key`"
        ));
    }
    let text = |key: &str| -> Result<Option<String>, String> {
        match arguments.get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::String(v)) => {
                let v = v.trim();
                if v.is_empty() || v.chars().count() > MAX_LAUNCH_NAME {
                    Err(format!(
                        "{TOOL}: `{key}` must be a name of 1 to {MAX_LAUNCH_NAME} characters"
                    ))
                } else {
                    Ok(Some(v.to_string()))
                }
            }
            Some(_) => Err(format!("{TOOL}: `{key}` must be a string")),
        }
    };
    let (name, key) = (text("name")?, text("key")?);
    if name.is_none() && key.is_none() {
        return Err(format!(
            "{TOOL} requires `name` (as the system lists the application) or `key` (its bundle \
             identifier or path)"
        ));
    }
    Ok((name, key))
}

fn computer_menu_path(arguments: &Value, tool: &str) -> Result<Vec<String>, String> {
    let invalid = || {
        format!(
            "{tool} requires `path`: the titles from the menu bar down to the command, e.g. \
             [\"File\", \"Export\", \"PDF…\"] — one to {MAX_MENU_PATH} of them"
        )
    };
    let items = arguments
        .get("path")
        .and_then(Value::as_array)
        .ok_or_else(invalid)?;
    if items.is_empty() || items.len() > MAX_MENU_PATH {
        return Err(invalid());
    }
    items
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::trim)
                .filter(|title| !title.is_empty())
                .map(str::to_string)
                .ok_or_else(invalid)
        })
        .collect()
}

fn computer_element(
    arguments: &Value,
    tool: &str,
) -> Result<crate::computer::types::ElementTarget, String> {
    use crate::computer::types::AgentTarget;
    match computer_target(arguments, tool)? {
        Some(AgentTarget::Element(element)) => Ok(element),
        _ => Err(format!(
            "{tool} requires `ref` (an element's number from computer_snapshot) and `generation`"
        )),
    }
}

fn computer_choice<'a>(
    arguments: &Value,
    tool: &str,
    key: &str,
    allowed: &[&'a str],
) -> Result<Option<&'a str>, String> {
    match arguments.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => allowed
            .iter()
            .find(|a| **a == s.as_str())
            .copied()
            .map(Some)
            .ok_or_else(|| format!("{tool}: `{key}` must be one of {}", allowed.join(", "))),
        Some(other) => Err(format!(
            "{tool}: `{key}` must be one of {}, not {other}",
            allowed.join(", ")
        )),
    }
}

fn computer_bool(arguments: &Value, tool: &str, key: &str) -> Result<bool, String> {
    match arguments.get(key) {
        None | Some(Value::Null) => Ok(false),
        Some(Value::Bool(b)) => Ok(*b),
        Some(other) => Err(format!("{tool}: `{key}` must be true or false, not {other}")),
    }
}

fn computer_text(arguments: &Value, tool: &str, key: &str) -> Result<String, String> {
    use crate::computer::types::MAX_ACTION_TEXT_CHARS;
    let text = arguments
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{tool} requires `{key}`, a string"))?;
    if text.chars().count() > MAX_ACTION_TEXT_CHARS {
        return Err(format!(
            "{tool}: `{key}` may be at most {MAX_ACTION_TEXT_CHARS} characters; send it in pieces"
        ));
    }
    Ok(text.to_string())
}

fn computer_count(
    arguments: &Value,
    tool: &str,
    key: &str,
    range: std::ops::RangeInclusive<u32>,
    default: u32,
) -> Result<u32, String> {
    match computer_optional_u32(arguments, tool, key)? {
        None => Ok(default),
        Some(n) if range.contains(&n) => Ok(n),
        Some(n) => Err(format!(
            "{tool}: `{key}` must be from {} to {}, not {n}",
            range.start(),
            range.end()
        )),
    }
}

fn computer_act_arguments(tool: &str) -> &'static [&'static str] {
    match tool {
        "computer_click" => &[
            "targetId",
            "target_id",
            "ref",
            "coordinate",
            "generation",
            "button",
            "count",
            "modifiers",
            "delivery",
        ],
        "computer_drag" => &[
            "targetId",
            "target_id",
            "from",
            "to",
            "generation",
            "button",
            "modifiers",
            "durationMs",
            "delivery",
        ],
        "computer_scroll" => &[
            "targetId",
            "target_id",
            "direction",
            "amount",
            "unit",
            "ref",
            "coordinate",
            "generation",
            "delivery",
        ],
        "computer_type" => &[
            "targetId",
            "target_id",
            "ref",
            "generation",
            "text",
            "submit",
            "delivery",
        ],
        "computer_press_key" => &[
            "targetId",
            "target_id",
            "key",
            "modifiers",
            "repeat",
            "ref",
            "generation",
            "delivery",
        ],
        "computer_hold_key" => &[
            "targetId",
            "target_id",
            "key",
            "modifiers",
            "durationMs",
            "ref",
            "generation",
            "delivery",
        ],
        "computer_set_value" => &["targetId", "target_id", "ref", "generation", "value"],
        "computer_restore" => &["targetId", "target_id"],
        "computer_invoke_menu" => &["targetId", "target_id", "path"],
        "computer_set_window_frame" => &["targetId", "target_id", "x", "y", "width", "height"],
        _ => &[],
    }
}

pub fn computer_act_request(
    tool: &str,
    arguments: &Value,
) -> Result<
    (
        String,
        crate::computer::types::ComputerActRequest,
        Option<crate::computer::types::ActDelivery>,
    ),
    String,
> {
    use crate::computer::keys::{Chord, Key};
    use crate::computer::types::{
        ActDelivery, ComputerActRequest, PointerButton, ScrollDirection, ScrollUnit, MAX_HOLD_MS,
        MAX_KEY_REPEAT, MAX_SCROLL_AMOUNT,
    };
    let allowed = computer_act_arguments(tool);
    if let Some(unknown) = arguments
        .as_object()
        .and_then(|args| args.keys().find(|k| !allowed.contains(&k.as_str())))
    {
        return Err(format!(
            "{tool} takes no argument `{unknown}`; it takes {}",
            allowed
                .iter()
                .filter(|a| **a != "target_id")
                .map(|a| format!("`{a}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    let target_id = computer_target_id(arguments, tool)?;
    let request = match tool {
        "computer_click" => {
            let target = computer_target(arguments, tool)?.ok_or_else(|| {
                format!(
                    "{tool} requires `ref` (from computer_snapshot) or `coordinate` (from \
                     computer_screenshot), with its `generation`"
                )
            })?;
            let button = match computer_choice(arguments, tool, "button", &["left", "right", "middle"])? {
                Some("right") => PointerButton::Right,
                Some("middle") => PointerButton::Middle,
                _ => PointerButton::Left,
            };
            let count = computer_count(arguments, tool, "count", 1..=2, 1)?;
            if count == 2 && button != PointerButton::Left {
                return Err(format!(
                    "{tool}: a double click (`count: 2`) is with the left button only"
                ));
            }
            ComputerActRequest::Click {
                target,
                button,
                count: count as u8,
                modifiers: computer_modifiers(arguments, tool)?,
            }
        }
        "computer_drag" => {
            let generation = computer_generation(arguments, tool, "from")?;
            let point = |name: &str| -> Result<crate::computer::types::PointTarget, String> {
                let value = arguments
                    .get(name)
                    .filter(|v| !v.is_null())
                    .ok_or_else(|| {
                        format!(
                            "{tool} requires `from` and `to`, each an [x, y] point in the \
                             image computer_screenshot returned"
                        )
                    })?;
                let (x, y) = computer_coordinate(value, tool)?;
                Ok(crate::computer::types::PointTarget {
                    generation: generation.clone(),
                    x,
                    y,
                })
            };
            ComputerActRequest::Drag {
                from: point("from")?,
                to: point("to")?,
                button: match computer_choice(
                    arguments,
                    tool,
                    "button",
                    &["left", "right", "middle"],
                )? {
                    Some("right") => PointerButton::Right,
                    Some("middle") => PointerButton::Middle,
                    _ => PointerButton::Left,
                },
                modifiers: computer_modifiers(arguments, tool)?,
                duration_ms: match arguments.get("durationMs").filter(|v| !v.is_null()) {
                    None => None,
                    Some(_) => Some(computer_count(
                        arguments,
                        tool,
                        "durationMs",
                        0..=crate::computer::types::MAX_DRAG_MS,
                        0,
                    )?),
                },
            }
        }
        "computer_scroll" => {
            let direction = match computer_choice(
                arguments,
                tool,
                "direction",
                &["up", "down", "left", "right"],
            )? {
                Some("up") => ScrollDirection::Up,
                Some("down") => ScrollDirection::Down,
                Some("left") => ScrollDirection::Left,
                Some("right") => ScrollDirection::Right,
                _ => {
                    return Err(format!(
                        "{tool} requires `direction`: up, down, left or right"
                    ))
                }
            };
            let unit = match computer_choice(arguments, tool, "unit", &["line", "page"])? {
                Some("page") => ScrollUnit::Page,
                _ => ScrollUnit::Line,
            };
            ComputerActRequest::Scroll {
                target: computer_target(arguments, tool)?,
                direction,
                amount: computer_count(arguments, tool, "amount", 1..=MAX_SCROLL_AMOUNT, 3)?,
                unit,
            }
        }
        "computer_type" => ComputerActRequest::Type {
            target: computer_element(arguments, tool)?,
            text: {
                let text = computer_text(arguments, tool, "text")?;
                if text.is_empty() {
                    return Err(format!("{tool}: `text` is empty"));
                }
                text
            },
            submit: computer_bool(arguments, tool, "submit")?,
        },
        "computer_press_key" | "computer_hold_key" => {
            let key = arguments
                .get("key")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("{tool} requires `key`, a string"))?;
            let key = Key::parse(key).map_err(|e| format!("{tool}: {e}"))?;
            let chord = Chord {
                key,
                modifiers: computer_modifiers(arguments, tool)?,
            };
            let target = match computer_target(arguments, tool)? {
                Some(crate::computer::types::AgentTarget::Element(e)) => Some(e),
                Some(crate::computer::types::AgentTarget::Point(_)) => {
                    return Err(format!(
                        "{tool} presses a key on an element (`ref`) or on whatever has focus — \
                         not at a coordinate"
                    ))
                }
                None => None,
            };
            if chord.types_text() && target.is_none() {
                return Err(format!(
                    "{tool}: a key that types a character goes only into an element you name — \
                     pass its `ref` and `generation`, or type the text with computer_type"
                ));
            }
            if tool == "computer_hold_key" {
                if arguments.get("durationMs").is_none_or(Value::is_null) {
                    return Err(format!(
                        "{tool} requires `durationMs`, how long to hold the key (at most {MAX_HOLD_MS})"
                    ));
                }
                ComputerActRequest::HoldKey {
                    target,
                    chord,
                    duration_ms: computer_count(arguments, tool, "durationMs", 1..=MAX_HOLD_MS, 1)?,
                }
            } else {
                ComputerActRequest::Key {
                    target,
                    chord,
                    repeat: computer_count(arguments, tool, "repeat", 1..=MAX_KEY_REPEAT, 1)?,
                }
            }
        }
        "computer_set_value" => ComputerActRequest::SetValue {
            target: computer_element(arguments, tool)?,
            value: computer_text(arguments, tool, "value")?,
        },
        "computer_restore" => ComputerActRequest::Restore,
        "computer_invoke_menu" => ComputerActRequest::InvokeMenu {
            path: computer_menu_path(arguments, tool)?,
        },
        "computer_set_window_frame" => {
            let number = |key: &str| -> Result<Option<f64>, String> {
                match arguments.get(key) {
                    None | Some(Value::Null) => Ok(None),
                    Some(v) => v
                        .as_f64()
                        .filter(|n| n.is_finite())
                        .map(Some)
                        .ok_or_else(|| format!("{tool}: `{key}` must be a number")),
                }
            };
            let (x, y, width, height) = (
                number("x")?,
                number("y")?,
                number("width")?,
                number("height")?,
            );
            if x.is_none() && y.is_none() && width.is_none() && height.is_none() {
                return Err(format!(
                    "{tool} requires at least one of `x`, `y`, `width`, `height`"
                ));
            }
            ComputerActRequest::SetFrame {
                x,
                y,
                width,
                height,
            }
        }
        other => return Err(format!("unknown tool: {other}")),
    };
    // Only the tools that take it get this far with one (see
    // `computer_act_arguments`).
    let delivery = computer_choice(arguments, tool, "delivery", &["background", "foreground"])?
        .map(|word| match word {
            "foreground" => ActDelivery::Foreground,
            _ => ActDelivery::Background,
        });
    Ok((target_id, request, delivery))
}

pub async fn dispatch(access:&impl crate::model_tools::ComputerToolAccess,name:&str,arguments:&Value)->anyhow::Result<Value>{
    use crate::model_tools::ComputerToolAccess;
    let parsed=|error:String|anyhow::anyhow!("invalid_arguments: {error}");
    let value=match name {
        "computer_list_apps"=>serde_json::to_value(access.list_apps().await)?,
        "computer_list_windows"=>serde_json::to_value(access.list_windows(computer_optional_u32(arguments,name,"pid").map_err(parsed)?).await)?,
        "computer_screenshot"=>{let (target,max)=computer_capture_request(arguments).map_err(parsed)?;serde_json::to_value(access.capture(&target,max).await)?},
        "computer_snapshot"=>{let (target,request)=computer_snapshot_request(arguments).map_err(parsed)?;serde_json::to_value(access.snapshot(&target,request).await)?},
        "computer_verify"=>{let (target,request)=computer_verify_request(arguments).map_err(parsed)?;serde_json::to_value(access.verify(&target,request).await)?},
        "computer_launch_app"=>{let (app,key)=computer_launch_arguments(arguments).map_err(parsed)?;serde_json::to_value(access.launch_app(app,key).await)?},
        "computer_clipboard_read"|"computer_clipboard_write"=>serde_json::to_value(access.clipboard(computer_clipboard_op(name,arguments).map_err(parsed)?).await)?,
        _=>{let(target,request,delivery)=computer_act_request(name,arguments).map_err(parsed)?;serde_json::to_value(access.act(&target,request,delivery).await)?},
    };
    Ok(value)
}
