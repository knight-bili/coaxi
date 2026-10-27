//! Exact approved r1 inputs/copy, with closed scoped page/result schemas.
use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::sync::OnceLock;

pub(crate) const OPERATIONS: [&str; 6] = [
    "delegation.start",
    "delegation.status",
    "delegation.cancel",
    "delegation.resume",
    "delegation.read",
    "delegation.history",
];
fn approved() -> &'static Value {
    static CONTRACT: OnceLock<Value> = OnceLock::new();
    CONTRACT.get_or_init(|| {
        serde_json::from_str(include_str!("../resources/delegation-tools-r1.json"))
            .expect("embedded approved delegation contract")
    })
}
pub(crate) fn definitions() -> Vec<Value> {
    OPERATIONS
        .into_iter()
        .map(|op| {
            json!({
                "name":op,"title":op,"description":approved()["operation_descriptions"][op],
                "inputSchema":approved()["operations"][op],"outputSchema":output_schema(op),
            })
        })
        .collect()
}
fn object(properties: Value, required: Value) -> Value {
    json!({"type":"object","additionalProperties":false,"properties":properties,"required":required})
}
fn identifier() -> Value {
    json!({"type":"string","minLength":1})
}
fn unknown() -> Value {
    object(
        json!({"task_id":identifier(),"status":{"const":"unknown"}}),
        json!(["task_id", "status"]),
    )
}
fn output_schema(op: &str) -> Value {
    let report = &approved()["taskReport"];
    match op {
        "delegation.start" | "delegation.cancel" | "delegation.resume" => report.clone(),
        "delegation.status" => object(
            json!({"tasks":{"type":"array","items":report},"cursor":identifier()}),
            json!(["tasks", "cursor"]),
        ),
        "delegation.read" => json!({"oneOf":[unknown(), object(json!({"task_id":identifier(),
            "result_ref":object(json!({"conversation_id":identifier(),"message_id":identifier()}),json!(["conversation_id","message_id"])),
            "text":{"type":"string"},"next_cursor":identifier()}),json!(["task_id","result_ref","text"]))]}),
        "delegation.history" => {
            let attachment = object(
                json!({"id":identifier(),"name":{"type":"string"},"kind":identifier(),"size_bytes":{"type":"integer","minimum":0}}),
                json!(["id", "name", "kind"]),
            );
            let message = object(
                json!({"id":identifier(),"sequence":{"type":"integer","minimum":1},"author_type":{"enum":["user","delegation_parent","assistant"]},"body":{"type":"string"},"created_at":{"type":"string"},"attachments":{"type":"array","items":attachment}}),
                json!(["id", "sequence", "author_type", "body", "created_at"]),
            );
            json!({"oneOf":[unknown(),object(json!({"task_id":identifier(),"conversation_id":identifier(),"messages":{"type":"array","items":message},"next_cursor":identifier()}),json!(["task_id","conversation_id","messages"]))]})
        }
        _ => unreachable!(),
    }
}
pub(crate) fn validate_output(op: &str, output: &Value) -> Result<()> {
    crate::builtin_tool_cli_output::validate_schema(output, &output_schema(op))?;
    // The existing bounded schema engine has no generic regexp evaluator.
    // Reuse the exact wire type for the approved builtin/custom identifier;
    // neither a lookalike runtime nor a custom path can pass output projection.
    fn runtime(report: &Value) -> Result<()> {
        if let Some(target) = report.get("agent_type") {
            crate::delegation_contract::RuntimeTarget::try_from(
                target
                    .as_str()
                    .context("delegation Runtime must be a string")?
                    .to_owned(),
            )
            .map_err(anyhow::Error::msg)?;
        }
        Ok(())
    }
    if op == "delegation.status" {
        for report in output["tasks"]
            .as_array()
            .context("delegation status tasks required")?
        {
            runtime(report)?;
        }
    } else if matches!(
        op,
        "delegation.start" | "delegation.cancel" | "delegation.resume"
    ) {
        runtime(output)?;
    }
    Ok(())
}
