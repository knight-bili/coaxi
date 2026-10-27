//! 本次静态合同使用的闭集校验；不加载远端引用，不替客户端补默认值。
use super::*;
pub(super) fn validate(root: &Value, shape: &Value, value: &Value, depth: usize) -> Result<()> {
    ensure!(depth <= 64, "conversation_tools.shape_depth_limit");
    if let Some(reference) = shape["$ref"].as_str() {
        let pointer = reference
            .strip_prefix('#')
            .filter(|pointer| pointer.starts_with('/'))
            .context("conversation_tools.external_reference_forbidden")?;
        return validate(
            root,
            root.pointer(pointer)
                .context("conversation_tools.schema_reference_missing")?,
            value,
            depth + 1,
        );
    }
    if let Some(choices) = shape["oneOf"].as_array() {
        ensure!(
            choices
                .iter()
                .filter(|shape| validate(root, shape, value, depth + 1).is_ok())
                .count()
                == 1,
            "conversation_tools.shape_mismatch"
        );
    }
    if let Some(choices) = shape["anyOf"].as_array() {
        ensure!(
            choices
                .iter()
                .any(|shape| validate(root, shape, value, depth + 1).is_ok()),
            "conversation_tools.shape_mismatch"
        );
    }
    if let Some(expected) = shape.get("const") {
        ensure!(value == expected, "conversation_tools.constant_mismatch")
    }
    if let Some(options) = shape["enum"].as_array() {
        ensure!(options.contains(value), "conversation_tools.enum_invalid")
    }
    if let Some(kind) = shape["type"].as_str() {
        ensure!(
            match kind {
                "object" => value.is_object(),
                "array" => value.is_array(),
                "string" => value.is_string(),
                "boolean" => value.is_boolean(),
                "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
                "number" => value.as_f64().is_some_and(f64::is_finite),
                "null" => value.is_null(),
                _ => false,
            },
            "conversation_tools.type_invalid"
        )
    }
    if let Some(object) = value.as_object() {
        ensure!(
            object.len() <= 4096
                && shape["maxProperties"]
                    .as_u64()
                    .is_none_or(|max| object.len() as u64 <= max),
            "conversation_tools.properties_limit"
        );
        if let Some(required) = shape["required"].as_array() {
            for key in required {
                ensure!(
                    key.as_str().is_some_and(|key| object.contains_key(key)),
                    "conversation_tools.field_required"
                )
            }
        }
        for (key, item) in object {
            ensure!(
                key.len() <= 4096 && !key.contains('\0'),
                "conversation_tools.key_invalid"
            );
            if let Some(child) = shape["properties"].get(key) {
                validate(root, child, item, depth + 1)?
            } else if shape["additionalProperties"] == false {
                anyhow::bail!("conversation_tools.unknown_field")
            } else if shape["additionalProperties"].is_object() {
                validate(root, &shape["additionalProperties"], item, depth + 1)?
            }
        }
    }
    if let Some(items) = value.as_array() {
        ensure!(
            items.len() <= 8192
                && shape["minItems"]
                    .as_u64()
                    .is_none_or(|min| items.len() as u64 >= min)
                && shape["maxItems"]
                    .as_u64()
                    .is_none_or(|max| items.len() as u64 <= max),
            "conversation_tools.items_limit"
        );
        if let Some(child) = shape.get("items") {
            for item in items {
                validate(root, child, item, depth + 1)?
            }
        }
    }
    if let Some(text) = value.as_str() {
        let length = text.chars().count() as u64;
        ensure!(
            !text.contains('\0')
                && text.len() <= MAX_RESULT
                && shape["minLength"].as_u64().is_none_or(|min| length >= min)
                && shape["maxLength"].as_u64().is_none_or(|max| length <= max),
            "conversation_tools.text_limit"
        );
        if let Some(pattern) = shape["pattern"].as_str() {
            ensure!(
                regex::Regex::new(pattern)?.is_match(text),
                "conversation_tools.pattern_invalid"
            )
        }
        match shape["format"].as_str() {
            Some("coaxi-camp-id") => {
                crate::camp_id::ThreadId::parse(text)
                    .context("conversation_tools.camp_id_invalid")?;
            }
            Some("coaxi-agent-id") => {
                ensure!(
                    crate::agent_identity::parse_agent_id(text).is_some(),
                    "conversation_tools.agent_id_invalid"
                );
            }
            Some("uuid") => {
                uuid::Uuid::parse_str(text).context("conversation_tools.uuid_invalid")?;
            }
            Some("date-time") => {
                chrono::DateTime::parse_from_rfc3339(text)
                    .context("conversation_tools.time_invalid")?;
            }
            _ => {}
        }
    }
    if value.is_number() {
        let number = value
            .as_f64()
            .context("conversation_tools.number_invalid")?;
        ensure!(
            number.is_finite()
                && shape["minimum"].as_f64().is_none_or(|min| number >= min)
                && shape["maximum"].as_f64().is_none_or(|max| number <= max),
            "conversation_tools.number_out_of_range"
        )
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::{FlagsV1, MemberPolicyV1, PolicyV1, validate_operation};
    use serde_json::json;

    #[test]
    fn public_operations_preserve_canonical_camp_and_member_ids() {
        let camp = crate::camp_id::ThreadId::new().to_string();
        let scope = json!({"threadId":camp,"conversationId":null,"scopeKind":"public"});
        let request = json!({"scope":scope,"after":null,"limit":50});
        assert!(validate_operation("conversationTools.questions.list", &request).is_ok());
        for invalid in [
            uuid::Uuid::new_v4().to_string(),
            "rvcamp_00000000000000000000000000".into(),
            camp.to_uppercase(),
            "rvcamp_bad".into(),
        ] {
            let mut changed = request.clone();
            changed["scope"]["threadId"] = json!(invalid);
            assert!(validate_operation("conversationTools.questions.list", &changed).is_err());
        }
        let mut unknown = request.clone();
        unknown["scope"]["untrustedField"] = json!(true);
        assert!(validate_operation("conversationTools.questions.list", &unknown).is_err());
        let mut private = request.clone();
        private["scope"]["scopeKind"] = json!("single_chat");
        private["scope"]["conversationId"] = json!(uuid::Uuid::new_v4().to_string());
        assert!(validate_operation("conversationTools.questions.list", &private).is_ok());
        private["scope"]["conversationId"] = json!("not-a-conversation");
        assert!(validate_operation("conversationTools.questions.list", &private).is_err());
        for (agent, valid) in [
            ("agent_1", true),
            ("agent_4", true),
            ("agent_25", true),
            ("agent_0", false),
            ("agent_01", false),
            ("agent_-1", false),
            ("", false),
            ("agent_9223372036854775808", false),
        ] {
            let policy = PolicyV1 {
                members: vec![MemberPolicyV1 {
                    agent_id: agent.into(),
                    flags: FlagsV1::default(),
                }],
                ..PolicyV1::default()
            };
            assert_eq!(policy.validate().is_ok(), valid, "{agent}");
        }
    }
}
