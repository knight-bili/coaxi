//! 只读取已确认的静态合同，不加载远端 Schema 或执行定义中的代码。
use super::*;
pub(super) fn validate(schema: &Value, value: &Value, depth: usize) -> Result<()> {
    ensure!(depth <= 48, "custom_runtime.shape_too_deep");
    if let Some(choices) = schema["oneOf"].as_array() {
        ensure!(
            choices
                .iter()
                .filter(|shape| validate(shape, value, depth + 1).is_ok())
                .count()
                == 1,
            "custom_runtime.shape_mismatch"
        );
        return Ok(());
    }
    if let Some(expected) = schema.get("const") {
        ensure!(value == expected, "custom_runtime.constant_mismatch")
    }
    if let Some(choices) = schema["enum"].as_array() {
        ensure!(choices.contains(value), "custom_runtime.enum_invalid")
    }
    if let Some(kind) = schema["type"].as_str() {
        ensure!(
            match kind {
                "object" => value.is_object(),
                "array" => value.is_array(),
                "string" => value.is_string(),
                "boolean" => value.is_boolean(),
                "null" => value.is_null(),
                "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
                _ => false,
            },
            "custom_runtime.type_invalid"
        )
    }
    if let Some(map) = value.as_object() {
        ensure!(
            schema["maxProperties"]
                .as_u64()
                .is_none_or(|max| map.len() as u64 <= max),
            "custom_runtime.too_many_properties"
        );
        if let Some(required) = schema["required"].as_array() {
            for key in required {
                ensure!(
                    key.as_str().is_some_and(|key| map.contains_key(key)),
                    "custom_runtime.field_missing"
                )
            }
        }
        for (key, item) in map {
            if let Some(shape) = schema.get("propertyNames") {
                validate(shape, &Value::String(key.clone()), depth + 1)?
            }
            if let Some(shape) = schema["properties"].get(key) {
                validate(shape, item, depth + 1)?
            } else if schema["additionalProperties"] == false {
                anyhow::bail!("custom_runtime.unknown_field")
            } else if schema["additionalProperties"].is_object() {
                validate(&schema["additionalProperties"], item, depth + 1)?
            }
        }
    }
    if let Some(items) = value.as_array() {
        ensure!(
            schema["maxItems"]
                .as_u64()
                .is_none_or(|max| items.len() as u64 <= max)
                && schema["minItems"]
                    .as_u64()
                    .is_none_or(|min| items.len() as u64 >= min),
            "custom_runtime.array_size_invalid"
        );
        if let Some(shape) = schema.get("items") {
            for item in items {
                validate(shape, item, depth + 1)?
            }
        }
    }
    if let Some(text) = value.as_str() {
        let length = text.chars().count() as u64;
        ensure!(
            !text.contains('\0')
                && schema["maxLength"].as_u64().is_none_or(|max| length <= max)
                && schema["minLength"].as_u64().is_none_or(|min| length >= min),
            "custom_runtime.text_size_invalid"
        );
        if let Some(pattern) = schema["pattern"].as_str() {
            ensure!(
                regex::Regex::new(pattern)?.is_match(text),
                "custom_runtime.text_pattern_invalid"
            )
        }
        match schema["format"].as_str() {
            Some("uuid") => {
                uuid::Uuid::parse_str(text).context("custom_runtime.uuid_invalid")?;
            }
            Some("uri") => {
                reqwest::Url::parse(text).context("custom_runtime.uri_invalid")?;
            }
            _ => {}
        }
    }
    if value.is_number() {
        let number = value.as_f64().context("custom_runtime.number_invalid")?;
        ensure!(
            schema["minimum"].as_f64().is_none_or(|min| number >= min)
                && schema["maximum"].as_f64().is_none_or(|max| number <= max),
            "custom_runtime.number_out_of_range"
        )
    }
    Ok(())
}
