use anyhow::Result;
use serde_json::{Value, json};

/// 保留目录读取失败，避免客户端把运输故障缓存成“没有工具”。
pub(crate) fn response(id: Value, catalog: Result<Value>) -> Value {
    match catalog {
        Ok(tools) if tools.is_array() => {
            json!({"jsonrpc":"2.0","id":id,"result":{"tools":tools}})
        }
        _ => json!({"jsonrpc":"2.0","id":id,"error":{
            "code":-32603,
            "message":"Tool discovery is unavailable. Check the Coaxi tool connection and retry tools/list."
        }}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovery_failure_stays_retryable_without_hiding_or_leaking_the_error() {
        let tools = json!([{"name":"browser_list_tabs","inputSchema":{"type":"object"}}]);
        assert_eq!(
            response(json!(7), Ok(tools.clone())),
            json!({"jsonrpc":"2.0","id":7,"result":{"tools":tools}})
        );
        assert_eq!(
            response(json!("empty"), Ok(json!([])))["result"]["tools"],
            json!([])
        );
        for unavailable in [
            Err(anyhow::anyhow!("secret-token private/path")),
            Ok(Value::Null),
            Ok(json!({"tools":[]})),
        ] {
            let reply = response(json!("discovery"), unavailable);
            assert_eq!(reply["id"], "discovery");
            assert_eq!(reply["error"]["code"], -32603);
            assert!(reply.get("result").is_none());
            assert!(!reply.to_string().contains("secret-token"));
            assert!(!reply.to_string().contains("private/path"));
        }
        assert_eq!(
            response(json!("retry"), Ok(tools.clone()))["result"]["tools"],
            tools
        );
    }
}
