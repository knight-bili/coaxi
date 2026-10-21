import Foundation

// Codex's config lives in `~/.codex/config.toml` (structured toggles) +
// `~/.codex/auth.json` (the API key). This is a line-based TOML editor ported 1:1
// from the web `acp-agent-settings.tsx` (parseToml*, updateTomlRoot*, section
// helpers, patchCodexConfigTomlText, patchCodexAuthJsonText). Faithful enough to
// preserve unknown keys/sections; anything exotic is preserved verbatim and
// editable via the native-config (raw TOML) editor.

enum AgentTOML {

    // MARK: - Line split / join (JS `split(/\r?\n/)` / `join("\n")`)

    private static func lines(_ text: String) -> [String] {
        text.replacingOccurrences(of: "\r\n", with: "\n").components(separatedBy: "\n")
    }
    private static func join(_ lines: [String]) -> String {
        lines.joined(separator: "\n").trimmingCharacters(in: .whitespacesAndNewlines)
    }

    // MARK: - Tiny regex helpers

    private static func capture(_ pattern: String, _ s: String) -> String? {
        guard let re = try? NSRegularExpression(pattern: pattern) else { return nil }
        let range = NSRange(s.startIndex..., in: s)
        guard let m = re.firstMatch(in: s, range: range), m.numberOfRanges > 1,
              let r = Range(m.range(at: 1), in: s) else { return nil }
        return String(s[r])
    }
    private static func matches(_ pattern: String, _ s: String) -> Bool {
        guard let re = try? NSRegularExpression(pattern: pattern) else { return false }
        return re.firstMatch(in: s, range: NSRange(s.startIndex..., in: s)) != nil
    }

    /// Minimal JSON string quoting for TOML values (matches `JSON.stringify(str)`).
    static func jsonQuote(_ s: String) -> String {
        var out = "\""
        for scalar in s.unicodeScalars {
            switch scalar {
            case "\"": out += "\\\""
            case "\\": out += "\\\\"
            case "\n": out += "\\n"
            case "\r": out += "\\r"
            case "\t": out += "\\t"
            default:
                if scalar.value < 0x20 { out += String(format: "\\u%04x", scalar.value) }
                else { out.unicodeScalars.append(scalar) }
            }
        }
        return out + "\""
    }

    // MARK: - TOML scalar parsing

    private static func parseAssignmentKey(_ rawLine: String) -> String? {
        let line = rawLine.trimmingCharacters(in: .whitespaces)
        if line.isEmpty || line.hasPrefix("#") { return nil }
        guard let eq = line.firstIndex(of: "="), eq != line.startIndex else { return nil }
        let key = line[line.startIndex..<eq].trimmingCharacters(in: .whitespaces)
        guard !key.isEmpty, matches("^[A-Za-z0-9_.-]+$", key) else { return nil }
        return key
    }

    private static func parseStringLiteral(_ raw: String) -> String? {
        let text = raw.trimmingCharacters(in: .whitespaces)
        if text.isEmpty { return nil }
        let chars = Array(text)
        if chars[0] == "\"" {
            var escaped = false
            var i = 1
            while i < chars.count {
                let ch = chars[i]
                if escaped { escaped = false; i += 1; continue }
                if ch == "\\" { escaped = true; i += 1; continue }
                if ch == "\"" {
                    let literal = String(chars[0...i])
                    if let data = literal.data(using: .utf8),
                       let s = try? JSONSerialization.jsonObject(with: data, options: [.fragmentsAllowed]) as? String {
                        return s
                    }
                    return String(chars[1..<i])
                }
                i += 1
            }
            return nil
        }
        if chars[0] == "'" {
            if let end = chars[1...].firstIndex(of: "'") {
                return String(chars[1..<end])
            }
            return nil
        }
        return nil
    }

    private static func parseStringAssignment(_ rawLine: String) -> (key: String, value: String)? {
        guard let key = parseAssignmentKey(rawLine) else { return nil }
        let line = rawLine.trimmingCharacters(in: .whitespaces)
        guard let eq = line.firstIndex(of: "=") else { return nil }
        let valueText = String(line[line.index(after: eq)...])
        guard let value = parseStringLiteral(valueText) else { return nil }
        return (key, value.trimmingCharacters(in: .whitespaces))
    }

    private static func parseBooleanAssignment(_ rawLine: String) -> (key: String, value: Bool)? {
        guard let key = parseAssignmentKey(rawLine) else { return nil }
        let line = rawLine.trimmingCharacters(in: .whitespaces)
        guard let eq = line.firstIndex(of: "=") else { return nil }
        let valueText = line[line.index(after: eq)...].trimmingCharacters(in: .whitespaces)
        // /^(true|false)(?:\s+#.*)?$/
        if matches("^(true|false)(?:\\s+#.*)?$", valueText) {
            return (key, valueText.hasPrefix("true"))
        }
        return nil
    }

    // MARK: - Extract (read)

    struct CodexTomlValues {
        var model = ""
        var modelProvider = ""
        var modelReasoningEffort: CodexReasoningEffort = .fallback
        var providerNames: [String] = []
        var providerBaseUrls: [String: String] = [:]
        var providerSupportsWebsockets: [String: Bool] = [:]
        var featureResponsesWebsocketsV2 = false
        var featureSkills = false
        var serviceTierFast = false
    }

    static func extractCodexToml(_ configTomlText: String) -> CodexTomlValues {
        var v = CodexTomlValues()
        var providerNames = Set<String>()
        var currentProviderSection: String? = nil
        var inFeaturesSection = false

        for rawLine in lines(configTomlText) {
            let line = rawLine.trimmingCharacters(in: .whitespaces)
            if line.isEmpty || line.hasPrefix("#") { continue }

            if let section = capture("^\\[\\s*model_providers\\.([A-Za-z0-9_-]+)\\s*\\]$", line) {
                currentProviderSection = section
                inFeaturesSection = false
                if !section.trimmingCharacters(in: .whitespaces).isEmpty { providerNames.insert(section.trimmingCharacters(in: .whitespaces)) }
                continue
            }
            if matches("^\\[\\s*features\\s*\\]$", line) {
                inFeaturesSection = true; currentProviderSection = nil; continue
            }
            if line.hasPrefix("[") && line.hasSuffix("]") {
                currentProviderSection = nil; inFeaturesSection = false; continue
            }

            let assignment = parseStringAssignment(rawLine)
            if let a = assignment {
                if a.key == "model" { v.model = a.value; continue }
                if a.key == "model_provider" { v.modelProvider = a.value; continue }
                if a.key == "model_reasoning_effort" {
                    v.modelReasoningEffort = CodexReasoningEffort(rawValue: a.value.lowercased()) ?? .fallback
                    continue
                }
                if currentProviderSection == nil && !inFeaturesSection && a.key == "service_tier" {
                    v.serviceTierFast = a.value.lowercased() == "fast"; continue
                }
            }

            if let b = parseBooleanAssignment(rawLine) {
                if let section = currentProviderSection, b.key == "supports_websockets" {
                    v.providerSupportsWebsockets[section] = b.value
                    providerNames.insert(section.trimmingCharacters(in: .whitespaces)); continue
                }
                if inFeaturesSection && b.key == "responses_websockets_v2" { v.featureResponsesWebsocketsV2 = b.value; continue }
                if inFeaturesSection && b.key == "skills" { v.featureSkills = b.value; continue }
                if let p = capture("^model_providers\\.([A-Za-z0-9_-]+)\\.supports_websockets$", b.key) {
                    providerNames.insert(p.trimmingCharacters(in: .whitespaces))
                    v.providerSupportsWebsockets[p] = b.value; continue
                }
                if b.key == "features.responses_websockets_v2" { v.featureResponsesWebsocketsV2 = b.value; continue }
                if b.key == "features.skills" { v.featureSkills = b.value; continue }
            }

            guard let a = assignment else { continue }

            if let rawKey = parseAssignmentKey(rawLine),
               let p = capture("^model_providers\\.([A-Za-z0-9_-]+)\\.", rawKey) {
                providerNames.insert(p.trimmingCharacters(in: .whitespaces))
            }
            if let section = currentProviderSection, a.key == "base_url", !a.value.isEmpty {
                v.providerBaseUrls[section] = a.value
                providerNames.insert(section.trimmingCharacters(in: .whitespaces)); continue
            }
            if let p = capture("^model_providers\\.([A-Za-z0-9_-]+)\\.base_url$", a.key), !a.value.isEmpty {
                v.providerBaseUrls[p] = a.value
                providerNames.insert(p.trimmingCharacters(in: .whitespaces))
            }
        }

        if !v.modelProvider.trimmingCharacters(in: .whitespaces).isEmpty {
            providerNames.insert(v.modelProvider.trimmingCharacters(in: .whitespaces))
        }
        providerNames.insert(codexDefaultModelProvider)
        for name in v.providerBaseUrls.keys where !name.trimmingCharacters(in: .whitespaces).isEmpty {
            providerNames.insert(name.trimmingCharacters(in: .whitespaces))
        }
        v.providerNames = Array(providerNames)
        return v
    }

    struct CodexValues {
        var apiBaseUrl = ""
        var apiKey: String? = ""
        var model = ""
        var modelProvider = ""
        var reasoningEffort: CodexReasoningEffort = .fallback
        var supportsWebsockets = false
        var skills = false
        var serviceTierFast = false
    }

    static func extractCodex(authJsonText: String, configTomlText: String) -> CodexValues {
        let auth = parseAuthObject(authJsonText)
        let toml = extractCodexToml(configTomlText)
        let hasExplicit = !toml.modelProvider.trimmingCharacters(in: .whitespaces).isEmpty
        let active = hasExplicit ? toml.modelProvider.trimmingCharacters(in: .whitespaces) : codexDefaultModelProvider
        let baseUrl = hasExplicit
            ? (toml.providerBaseUrls[active] ?? "")
            : (toml.providerBaseUrls[codexDefaultModelProvider] ?? toml.providerBaseUrls["openai"] ?? "")
        let websockets = toml.providerSupportsWebsockets[active]
            ?? (active == codexDefaultModelProvider ? toml.featureResponsesWebsocketsV2 : false)
        var v = CodexValues()
        v.apiBaseUrl = baseUrl
        if auth.error == nil, let obj = auth.object {
            v.apiKey = JSONConfig.pickFirstString(obj, ["OPENAI_API_KEY", "OPENAI_API_TOKEN", "API_KEY"]) ?? ""
        } else {
            v.apiKey = nil
        }
        v.model = toml.model
        v.modelProvider = active
        v.reasoningEffort = toml.modelReasoningEffort
        v.supportsWebsockets = websockets
        v.skills = toml.featureSkills
        v.serviceTierFast = toml.serviceTierFast
        return v
    }

    // MARK: - auth.json

    private static func parseAuthObject(_ text: String) -> (object: [String: Any]?, error: String?) {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.isEmpty { return ([:], nil) }
        guard let data = trimmed.data(using: .utf8) else { return (nil, "auth.json format error") }
        do {
            let obj = try JSONSerialization.jsonObject(with: data)
            guard let dict = obj as? [String: Any] else { return (nil, "auth.json must be a JSON object") }
            return (dict, nil)
        } catch {
            return (nil, "auth.json format error: \(error.localizedDescription)")
        }
    }

    static func inferCodexAuthMode(_ authJsonText: String) -> CodexAuthMode {
        let parsed = parseAuthObject(authJsonText)
        if let obj = parsed.object {
            let authMode = obj["auth_mode"] as? String
            if authMode == "chatgpt" || !obj.keys.contains("OPENAI_API_KEY") || obj["OPENAI_API_KEY"] is NSNull {
                return .chatgptSubscription
            }
        }
        return .apiKey
    }

    /// Set/clear the OPENAI_API_KEY in auth.json (api_key mode only; never touches
    /// `auth_mode`/`tokens`, so a chatgpt-subscription agent is left intact).
    static func patchCodexAuth(_ authJsonText: String, apiKey: String) -> String {
        let parsed = parseAuthObject(authJsonText)
        var obj = (parsed.error == nil ? parsed.object : [:]) ?? [:]
        let key = apiKey.trimmingCharacters(in: .whitespaces)
        if !key.isEmpty {
            obj["OPENAI_API_KEY"] = key
            obj.removeValue(forKey: "API_KEY")
        } else {
            obj.removeValue(forKey: "OPENAI_API_KEY")
            obj.removeValue(forKey: "OPENAI_API_TOKEN")
            obj.removeValue(forKey: "API_KEY")
        }
        return JSONConfig.serialize(obj)
    }

    // MARK: - TOML root writers

    private static func rootEndIndex(_ lines: [String]) -> Int {
        for (i, l) in lines.enumerated() {
            let t = l.trimmingCharacters(in: .whitespaces)
            if matches("^\\[.*\\]$", t) { return i }
        }
        return lines.count
    }

    private static func rootAssignmentIndex(_ lines: [String], _ key: String) -> Int {
        let end = rootEndIndex(lines)
        for i in 0..<end where parseAssignmentKey(lines[i]) == key { return i }
        return -1
    }

    private static func preferredRootInsertion(_ lines: [String], _ key: String) -> Int {
        if key == "model" {
            let p = rootAssignmentIndex(lines, "model_provider")
            return p >= 0 ? p : 0
        }
        if key == "model_reasoning_effort" {
            let m = rootAssignmentIndex(lines, "model")
            return m >= 0 ? m + 1 : 0
        }
        var insertAt = rootEndIndex(lines)
        while insertAt > 0 && lines[insertAt - 1].trimmingCharacters(in: .whitespaces).isEmpty { insertAt -= 1 }
        return insertAt
    }

    static func setRootString(_ tomlText: String, _ key: String, _ value: String) -> String {
        var ls = lines(tomlText)
        let idx = rootAssignmentIndex(ls, key)
        let next = value.trimmingCharacters(in: .whitespaces)
        if next.isEmpty {
            if idx >= 0 { ls.remove(at: idx) }
            return join(ls)
        }
        let lineText = "\(key) = \(jsonQuote(value))"
        if idx >= 0 { ls[idx] = lineText }
        else { ls.insert(lineText, at: max(0, preferredRootInsertion(ls, key))) }
        return join(ls)
    }

    static func setRootBool(_ tomlText: String, _ key: String, _ value: Bool) -> String {
        var ls = lines(tomlText)
        let idx = rootAssignmentIndex(ls, key)
        let lineText = "\(key) = \(value ? "true" : "false")"
        if idx >= 0 { ls[idx] = lineText } else { ls.insert(lineText, at: 0) }
        return join(ls)
    }

    // MARK: - TOML section writers

    private static func sectionRange(_ lines: [String], _ name: String) -> (start: Int, end: Int)? {
        let header = "[\(name)]"
        var start = -1, end = lines.count
        for (i, l) in lines.enumerated() {
            let t = l.trimmingCharacters(in: .whitespaces)
            if start < 0 { if t == header { start = i }; continue }
            if matches("^\\[.*\\]$", t) { end = i; break }
        }
        return start < 0 ? nil : (start, end)
    }

    /// `upsertTomlSectionBooleanKey` (value==nil deletes the key, pruning the
    /// section if it becomes empty).
    static func upsertSectionBool(_ tomlText: String, section name: String, key: String, value: Bool?) -> String {
        var ls = lines(tomlText)
        if let sec = sectionRange(ls, name) {
            var assignIdx = -1
            for i in (sec.start + 1)..<sec.end where parseAssignmentKey(ls[i]) == key { assignIdx = i; break }
            if value == nil {
                if assignIdx >= 0 { ls.remove(at: assignIdx) }
                if let refreshed = sectionRange(ls, name) {
                    let hasEntries = ls[(refreshed.start + 1)..<refreshed.end].contains { raw in
                        let l = raw.trimmingCharacters(in: .whitespaces)
                        return !l.isEmpty && !l.hasPrefix("#")
                    }
                    if !hasEntries {
                        var before = Array(ls[0..<refreshed.start])
                        var after = Array(ls[refreshed.end...])
                        while let last = before.last, last.trimmingCharacters(in: .whitespaces).isEmpty { before.removeLast() }
                        while let first = after.first, first.trimmingCharacters(in: .whitespaces).isEmpty { after.removeFirst() }
                        let merged = (!before.isEmpty && !after.isEmpty) ? before + [""] + after : before + after
                        return join(merged)
                    }
                }
                return join(ls)
            }
            let lineText = "\(key) = \(value! ? "true" : "false")"
            if assignIdx >= 0 { ls[assignIdx] = lineText }
            else {
                var insertAt = sec.end
                var i = sec.end - 1
                while i > sec.start { if !ls[i].trimmingCharacters(in: .whitespaces).isEmpty { insertAt = i + 1; break }; i -= 1 }
                ls.insert(lineText, at: insertAt)
            }
            return join(ls)
        }
        guard let value else { return tomlText.trimmingCharacters(in: .whitespacesAndNewlines) }
        let lineText = "\(key) = \(value ? "true" : "false")"
        let insertAt = rootEndIndex(ls)
        let prefixBlank = (insertAt > 0 && !ls[insertAt - 1].trimmingCharacters(in: .whitespaces).isEmpty) ? [""] : []
        let suffixBlank = (insertAt < ls.count && !ls[insertAt].trimmingCharacters(in: .whitespaces).isEmpty) ? [""] : []
        ls.insert(contentsOf: prefixBlank + ["[\(name)]", lineText] + suffixBlank, at: insertAt)
        return join(ls)
    }

    private static func providerSectionRange(_ lines: [String], _ provider: String) -> (start: Int, end: Int)? {
        let pattern = "^\\[\\s*model_providers\\.\(NSRegularExpression.escapedPattern(for: provider))\\s*\\]$"
        var start = -1, end = lines.count
        for (i, l) in lines.enumerated() {
            let t = l.trimmingCharacters(in: .whitespaces)
            if start < 0 { if matches(pattern, t) { start = i }; continue }
            if matches("^\\[.*\\]$", t) { end = i; break }
        }
        return start < 0 ? nil : (start, end)
    }

    private static func patchProviderBaseUrl(_ tomlText: String, provider: String, apiBaseUrl: String) -> String {
        let provider = provider.trimmingCharacters(in: .whitespaces)
        guard !provider.isEmpty else { return tomlText.trimmingCharacters(in: .whitespacesAndNewlines) }
        let next = apiBaseUrl.trimmingCharacters(in: .whitespaces)
        var ls = lines(tomlText)
        if let sec = providerSectionRange(ls, provider) {
            var baseIdx = -1
            for i in (sec.start + 1)..<sec.end {
                if let a = parseStringAssignment(ls[i]), a.key == "base_url" { baseIdx = i; break }
            }
            if next.isEmpty {
                if baseIdx >= 0 { ls.remove(at: baseIdx) }
                return join(ls)
            }
            let lineText = "base_url = \(jsonQuote(next))"
            if baseIdx >= 0 { ls[baseIdx] = lineText } else { ls.insert(lineText, at: sec.end) }
            return join(ls)
        }
        if next.isEmpty { return tomlText.trimmingCharacters(in: .whitespacesAndNewlines) }
        let appended = tomlText.trimmingCharacters(in: CharacterSet.whitespacesAndNewlines.subtracting(CharacterSet(charactersIn: " ")))
            .trimmingTrailingNewlines()
        let sectionText = "[model_providers.\(provider)]\nbase_url = \(jsonQuote(next))"
        if appended.isEmpty { return sectionText }
        return "\(appended)\n\n\(sectionText)".trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private static func patchProviderField(_ tomlText: String, provider: String, key: String, lineText: String) -> String {
        let provider = provider.trimmingCharacters(in: .whitespaces)
        guard !provider.isEmpty else { return tomlText.trimmingCharacters(in: .whitespacesAndNewlines) }
        var ls = lines(tomlText)
        if let sec = providerSectionRange(ls, provider) {
            var fieldIdx = -1
            for i in (sec.start + 1)..<sec.end where parseAssignmentKey(ls[i]) == key { fieldIdx = i; break }
            if fieldIdx >= 0 { ls[fieldIdx] = lineText }
            else {
                var insertAt = sec.end
                while insertAt > sec.start + 1 && ls[insertAt - 1].trimmingCharacters(in: .whitespaces).isEmpty { insertAt -= 1 }
                ls.insert(lineText, at: insertAt)
            }
            return join(ls)
        }
        let appended = tomlText.trimmingTrailingNewlines()
        let sectionText = "[model_providers.\(provider)]\n\(lineText)"
        if appended.isEmpty { return sectionText }
        return "\(appended)\n\n\(sectionText)".trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private static func ensureProviderDefaults(_ tomlText: String, provider: String) -> String {
        guard provider.trimmingCharacters(in: .whitespaces) == codexDefaultModelProvider else { return tomlText }
        var next = tomlText
        let current = extractCodexToml(next)
        let codegBaseUrl = current.providerBaseUrls[codexDefaultModelProvider] ?? ""
        // Only (re)write base_url when non-empty — an empty value means the user
        // cleared it, so leave it deleted rather than re-adding `base_url = ""`.
        if !codegBaseUrl.isEmpty {
            next = patchProviderField(next, provider: codexDefaultModelProvider, key: "base_url", lineText: "base_url = \(jsonQuote(codegBaseUrl))")
        }
        next = patchProviderField(next, provider: codexDefaultModelProvider, key: "name", lineText: "name = \"codeg\"")
        next = patchProviderField(next, provider: codexDefaultModelProvider, key: "wire_api", lineText: "wire_api = \"responses\"")
        next = patchProviderField(next, provider: codexDefaultModelProvider, key: "requires_openai_auth", lineText: "requires_openai_auth = true")
        return next
    }

    // MARK: - patchCodexConfigTomlText (api_key mode; no model_provider link)

    static func patchCodex(_ tomlText: String, _ d: AgentDraft) -> String {
        var next = tomlText

        next = setRootString(next, "model", d.model)
        next = setRootString(next, "model_reasoning_effort", d.codexReasoningEffort.rawValue)

        // apiBaseUrl → active provider's base_url (default "codeg" when none set).
        do {
            let toml = extractCodexToml(next)
            let provider = !toml.modelProvider.trimmingCharacters(in: .whitespaces).isEmpty
                ? toml.modelProvider.trimmingCharacters(in: .whitespaces) : codexDefaultModelProvider
            if toml.modelProvider.trimmingCharacters(in: .whitespaces).isEmpty && !d.apiBaseUrl.trimmingCharacters(in: .whitespaces).isEmpty {
                next = setRootString(next, "model_provider", provider)
            }
            next = patchProviderBaseUrl(next, provider: provider, apiBaseUrl: d.apiBaseUrl)
            next = ensureProviderDefaults(next, provider: provider)
        }

        // supports_websockets → active provider (default "codeg" when none set).
        do {
            let toml = extractCodexToml(next)
            let provider = !toml.modelProvider.trimmingCharacters(in: .whitespaces).isEmpty
                ? toml.modelProvider.trimmingCharacters(in: .whitespaces) : codexDefaultModelProvider
            if toml.modelProvider.trimmingCharacters(in: .whitespaces).isEmpty {
                next = setRootString(next, "model_provider", provider)
            }
            next = patchProviderField(next, provider: provider, key: "supports_websockets",
                                      lineText: "supports_websockets = \(d.codexSupportsWebsockets ? "true" : "false")")
            next = ensureProviderDefaults(next, provider: provider)
        }

        // Re-normalize root model / reasoning effort, derive the feature flag.
        let normalized = extractCodexToml(next)
        if !normalized.model.trimmingCharacters(in: .whitespaces).isEmpty {
            next = setRootString(next, "model", normalized.model)
        }
        next = setRootString(next, "model_reasoning_effort", normalized.modelReasoningEffort.rawValue)
        let active = !normalized.modelProvider.trimmingCharacters(in: .whitespaces).isEmpty
            ? normalized.modelProvider.trimmingCharacters(in: .whitespaces) : codexDefaultModelProvider
        let featureOn = normalized.providerSupportsWebsockets[active] ?? false
        next = upsertSectionBool(next, section: "features", key: "responses_websockets_v2", value: featureOn ? true : nil)
        next = upsertSectionBool(next, section: "features", key: "skills", value: d.codexSkills ? true : nil)
        next = setRootString(next, "service_tier", d.codexServiceTierFast ? "fast" : "")
        next = setRootBool(next, "disable_response_storage", true)

        let trimmed = next.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? "" : trimmed + "\n"
    }
}

private extension String {
    func trimmingTrailingNewlines() -> String {
        var s = self
        while let last = s.last, last == "\n" || last == "\r" { s.removeLast() }
        return s
    }
}
