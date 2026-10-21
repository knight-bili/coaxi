import Foundation

/// A configured OpenAI-compatible model provider endpoint for a given agent.
/// Mirrors the server's `ModelProviderInfo`. We intentionally do NOT decode the
/// cleartext `api_key` — the UI only ever shows `apiKeyMasked`, and a new key is
/// sent only when the user re-enters one (blank = keep). Extra wire fields
/// (`api_key`, `created_at`, `updated_at`) are ignored by the decoder.
struct ModelProviderInfo: Decodable, Identifiable, Hashable, Sendable {
    let id: Int
    let name: String
    let apiUrl: String
    let apiKeyMasked: String
    let agentType: AgentType
    /// Interpretation depends on `agentType`: for `.claudeCode` it's a JSON
    /// string of `{main,reasoning,haiku,sonnet,opus}`; for others a plain model
    /// name. May be nil/empty.
    let model: String?
}

/// Result of `update_model_provider`: the updated provider plus how many running
/// sessions still hold the old credentials until restarted (surfaced as a toast).
/// `affectedRunningSessions` is already camelCase on the wire, so it matches the
/// `.convertFromSnakeCase` decoder as-is.
struct UpdateModelProviderResult: Decodable, Sendable {
    let provider: ModelProviderInfo
    let affectedRunningSessions: Int
}

/// Per-model overrides for a `claude_code` provider, (de)serialized to/from the
/// provider `model` JSON string. Mirrors the web's `ClaudeProviderModel` plus its
/// `parseClaudeProviderModel` / `serializeClaudeProviderModel` helpers.
struct ClaudeProviderModel: Equatable, Sendable {
    var main: String = ""
    var reasoning: String = ""
    var haiku: String = ""
    var sonnet: String = ""
    var opus: String = ""

    /// Parse the provider `model` JSON string into per-model fields (lenient:
    /// non-string / blank values are dropped). Empty / invalid input → all-blank.
    static func parse(_ raw: String?) -> ClaudeProviderModel {
        guard let raw, let data = raw.data(using: .utf8),
              let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            return ClaudeProviderModel()
        }
        func field(_ key: String) -> String {
            guard let v = obj[key] as? String else { return "" }
            return v.trimmingCharacters(in: .whitespacesAndNewlines)
        }
        return ClaudeProviderModel(
            main: field("main"), reasoning: field("reasoning"),
            haiku: field("haiku"), sonnet: field("sonnet"), opus: field("opus")
        )
    }

    /// Serialize the trimmed non-empty fields to a compact JSON string, or nil
    /// when nothing is set (mirrors `serializeClaudeProviderModel`).
    func serialized() -> String? {
        var out: [String: String] = [:]
        func put(_ key: String, _ value: String) {
            let t = value.trimmingCharacters(in: .whitespacesAndNewlines)
            if !t.isEmpty { out[key] = t }
        }
        put("main", main); put("reasoning", reasoning)
        put("haiku", haiku); put("sonnet", sonnet); put("opus", opus)
        guard !out.isEmpty,
              let data = try? JSONSerialization.data(withJSONObject: out, options: [.sortedKeys]),
              let json = String(data: data, encoding: .utf8) else { return nil }
        return json
    }
}

extension AgentType {
    /// Agent types that support a custom model provider (matches the web's
    /// `MODEL_PROVIDER_AGENT_TYPES`).
    static let modelProviderSupported: [AgentType] = [.claudeCode, .codex, .gemini]
}
