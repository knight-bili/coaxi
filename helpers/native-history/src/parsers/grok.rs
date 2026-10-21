use serde_json::Value;
use super::truncate_str;
pub(crate) fn cap_json_to_budget(value: &Value, budget: usize) -> Option<String> {
    let mut per_string = budget;
    loop {
        let serialized = serde_json::to_string(&cap_json_string_values(value, per_string)).ok()?;
        if serialized.len() <= budget || per_string == 0 {
            return Some(serialized);
        }
        per_string /= 2;
    }
}

fn cap_json_string_values(value: &Value, cap: usize) -> Value {
    match value {
        Value::String(s) => Value::String(truncate_str(s, cap)),
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|v| cap_json_string_values(v, cap))
                .collect(),
        ),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(k, v)| (k.clone(), cap_json_string_values(v, cap)))
                .collect(),
        ),
        other => other.clone(),
    }
}
