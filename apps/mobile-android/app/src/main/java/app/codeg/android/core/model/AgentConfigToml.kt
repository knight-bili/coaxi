package app.codeg.android.core.model

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonPrimitive

/**
 * Codex's config lives in `~/.codex/config.toml` (structured toggles) +
 * `~/.codex/auth.json` (the API key). This is a line-based TOML editor ported 1:1
 * from the iOS `AgentConfigTOML.swift` (which mirrors the web `acp-agent-settings.tsx`).
 * Faithful enough to preserve unknown keys/sections; anything exotic is preserved
 * verbatim and editable via the native-config (raw TOML) editor.
 */
object AgentToml {

    private val lenientJson = Json { isLenient = true; ignoreUnknownKeys = true }

    // region Line split / join

    private fun lines(text: String): List<String> = text.replace("\r\n", "\n").split("\n")

    private fun join(lines: List<String>): String = lines.joinToString("\n").trim()

    // endregion

    // region Tiny regex helpers

    private fun capture(pattern: String, s: String): String? =
        Regex(pattern).find(s)?.groupValues?.getOrNull(1)

    private fun matchesRe(pattern: String, s: String): Boolean = Regex(pattern).containsMatchIn(s)

    /** Minimal JSON string quoting for TOML values (matches `JSON.stringify(str)`). */
    fun jsonQuote(s: String): String {
        val sb = StringBuilder("\"")
        for (ch in s) {
            when (ch) {
                '"' -> sb.append("\\\"")
                '\\' -> sb.append("\\\\")
                '\n' -> sb.append("\\n")
                '\r' -> sb.append("\\r")
                '\t' -> sb.append("\\t")
                else -> if (ch.code < 0x20) sb.append("\\u%04x".format(ch.code)) else sb.append(ch)
            }
        }
        return sb.append("\"").toString()
    }

    // endregion

    // region TOML scalar parsing

    private fun parseAssignmentKey(rawLine: String): String? {
        val line = rawLine.trim()
        if (line.isEmpty() || line.startsWith("#")) return null
        val eq = line.indexOf('=')
        if (eq <= 0) return null
        val key = line.substring(0, eq).trim()
        if (key.isEmpty() || !Regex("^[A-Za-z0-9_.-]+$").matches(key)) return null
        return key
    }

    private fun jsonDecodeStringLiteral(literal: String): String? = try {
        lenientJson.parseToJsonElement(literal).asStringOrNull()
    } catch (_: Exception) {
        null
    }

    private fun parseStringLiteral(raw: String): String? {
        val text = raw.trim()
        if (text.isEmpty()) return null
        if (text[0] == '"') {
            var escaped = false
            var i = 1
            while (i < text.length) {
                val ch = text[i]
                if (escaped) { escaped = false; i++; continue }
                if (ch == '\\') { escaped = true; i++; continue }
                if (ch == '"') {
                    val literal = text.substring(0, i + 1)
                    return jsonDecodeStringLiteral(literal) ?: text.substring(1, i)
                }
                i++
            }
            return null
        }
        if (text[0] == '\'') {
            val end = text.indexOf('\'', startIndex = 1)
            if (end >= 0) return text.substring(1, end)
            return null
        }
        return null
    }

    private fun parseStringAssignment(rawLine: String): Pair<String, String>? {
        val key = parseAssignmentKey(rawLine) ?: return null
        val line = rawLine.trim()
        val eq = line.indexOf('=')
        if (eq < 0) return null
        val value = parseStringLiteral(line.substring(eq + 1)) ?: return null
        return key to value.trim()
    }

    private fun parseBooleanAssignment(rawLine: String): Pair<String, Boolean>? {
        val key = parseAssignmentKey(rawLine) ?: return null
        val line = rawLine.trim()
        val eq = line.indexOf('=')
        if (eq < 0) return null
        val valueText = line.substring(eq + 1).trim()
        if (matchesRe("^(true|false)(?:\\s+#.*)?$", valueText)) {
            return key to valueText.startsWith("true")
        }
        return null
    }

    // endregion

    // region Extract (read)

    data class CodexTomlValues(
        val model: String = "",
        val modelProvider: String = "",
        val modelReasoningEffort: CodexReasoningEffort = CodexReasoningEffort.FALLBACK,
        val providerNames: List<String> = emptyList(),
        val providerBaseUrls: Map<String, String> = emptyMap(),
        val providerSupportsWebsockets: Map<String, Boolean> = emptyMap(),
        val featureResponsesWebsocketsV2: Boolean = false,
        val featureSkills: Boolean = false,
        val serviceTierFast: Boolean = false,
    )

    fun extractCodexToml(configTomlText: String): CodexTomlValues {
        var model = ""
        var modelProvider = ""
        var reasoning = CodexReasoningEffort.FALLBACK
        val providerBaseUrls = LinkedHashMap<String, String>()
        val providerSupportsWs = LinkedHashMap<String, Boolean>()
        var featureWsV2 = false
        var featureSkills = false
        var serviceTierFast = false
        val providerNames = LinkedHashSet<String>()
        var currentProviderSection: String? = null
        var inFeaturesSection = false

        for (rawLine in lines(configTomlText)) {
            val line = rawLine.trim()
            if (line.isEmpty() || line.startsWith("#")) continue

            val section = capture("^\\[\\s*model_providers\\.([A-Za-z0-9_-]+)\\s*\\]$", line)
            if (section != null) {
                currentProviderSection = section
                inFeaturesSection = false
                if (section.trim().isNotEmpty()) providerNames.add(section.trim())
                continue
            }
            if (matchesRe("^\\[\\s*features\\s*\\]$", line)) {
                inFeaturesSection = true; currentProviderSection = null; continue
            }
            if (line.startsWith("[") && line.endsWith("]")) {
                currentProviderSection = null; inFeaturesSection = false; continue
            }

            val assignment = parseStringAssignment(rawLine)
            if (assignment != null) {
                val (key, value) = assignment
                if (key == "model") { model = value; continue }
                if (key == "model_provider") { modelProvider = value; continue }
                if (key == "model_reasoning_effort") { reasoning = CodexReasoningEffort.fromWire(value); continue }
                if (currentProviderSection == null && !inFeaturesSection && key == "service_tier") {
                    serviceTierFast = value.lowercase() == "fast"; continue
                }
            }

            val boolean = parseBooleanAssignment(rawLine)
            if (boolean != null) {
                val (key, value) = boolean
                val sec = currentProviderSection
                if (sec != null && key == "supports_websockets") {
                    providerSupportsWs[sec] = value
                    providerNames.add(sec.trim()); continue
                }
                if (inFeaturesSection && key == "responses_websockets_v2") { featureWsV2 = value; continue }
                if (inFeaturesSection && key == "skills") { featureSkills = value; continue }
                val p = capture("^model_providers\\.([A-Za-z0-9_-]+)\\.supports_websockets$", key)
                if (p != null) {
                    providerNames.add(p.trim()); providerSupportsWs[p] = value; continue
                }
                if (key == "features.responses_websockets_v2") { featureWsV2 = value; continue }
                if (key == "features.skills") { featureSkills = value; continue }
            }

            if (assignment == null) continue
            val (key, value) = assignment

            parseAssignmentKey(rawLine)?.let { rawKey ->
                capture("^model_providers\\.([A-Za-z0-9_-]+)\\.", rawKey)?.let { providerNames.add(it.trim()) }
            }
            val sec = currentProviderSection
            if (sec != null && key == "base_url" && value.isNotEmpty()) {
                providerBaseUrls[sec] = value
                providerNames.add(sec.trim()); continue
            }
            val pBase = capture("^model_providers\\.([A-Za-z0-9_-]+)\\.base_url$", key)
            if (pBase != null && value.isNotEmpty()) {
                providerBaseUrls[pBase] = value
                providerNames.add(pBase.trim())
            }
        }

        if (modelProvider.trim().isNotEmpty()) providerNames.add(modelProvider.trim())
        providerNames.add(CODEX_DEFAULT_MODEL_PROVIDER)
        for (name in providerBaseUrls.keys) if (name.trim().isNotEmpty()) providerNames.add(name.trim())

        return CodexTomlValues(
            model = model,
            modelProvider = modelProvider,
            modelReasoningEffort = reasoning,
            providerNames = providerNames.toList(),
            providerBaseUrls = providerBaseUrls,
            providerSupportsWebsockets = providerSupportsWs,
            featureResponsesWebsocketsV2 = featureWsV2,
            featureSkills = featureSkills,
            serviceTierFast = serviceTierFast,
        )
    }

    data class CodexValues(
        val apiBaseUrl: String = "",
        val apiKey: String? = "",
        val model: String = "",
        val modelProvider: String = "",
        val reasoningEffort: CodexReasoningEffort = CodexReasoningEffort.FALLBACK,
        val supportsWebsockets: Boolean = false,
        val skills: Boolean = false,
        val serviceTierFast: Boolean = false,
    )

    fun extractCodex(authJsonText: String, configTomlText: String): CodexValues {
        val auth = parseAuthObject(authJsonText)
        val toml = extractCodexToml(configTomlText)
        val hasExplicit = toml.modelProvider.trim().isNotEmpty()
        val active = if (hasExplicit) toml.modelProvider.trim() else CODEX_DEFAULT_MODEL_PROVIDER
        val baseUrl = if (hasExplicit) {
            toml.providerBaseUrls[active] ?: ""
        } else {
            toml.providerBaseUrls[CODEX_DEFAULT_MODEL_PROVIDER] ?: toml.providerBaseUrls["openai"] ?: ""
        }
        val websockets = toml.providerSupportsWebsockets[active]
            ?: if (active == CODEX_DEFAULT_MODEL_PROVIDER) toml.featureResponsesWebsocketsV2 else false
        val apiKey: String? = if (auth.error == null && auth.obj != null) {
            JsonConfig.pickFirstString(auth.obj, listOf("OPENAI_API_KEY", "OPENAI_API_TOKEN", "API_KEY")) ?: ""
        } else {
            null
        }
        return CodexValues(
            apiBaseUrl = baseUrl,
            apiKey = apiKey,
            model = toml.model,
            modelProvider = active,
            reasoningEffort = toml.modelReasoningEffort,
            supportsWebsockets = websockets,
            skills = toml.featureSkills,
            serviceTierFast = toml.serviceTierFast,
        )
    }

    // endregion

    // region auth.json

    private data class AuthParsed(val obj: Map<String, JsonElement>?, val error: String?)

    private fun parseAuthObject(text: String): AuthParsed {
        val trimmed = text.trim()
        if (trimmed.isEmpty()) return AuthParsed(emptyMap(), null)
        val p = JsonConfig.parse(trimmed)
        return if (p.error != null) AuthParsed(null, p.error) else AuthParsed(p.config, null)
    }

    fun inferCodexAuthMode(authJsonText: String): CodexAuthMode {
        val obj = parseAuthObject(authJsonText).obj
        if (obj != null) {
            val authMode = obj["auth_mode"]?.asStringOrNull()
            val hasKey = obj.containsKey("OPENAI_API_KEY") && obj["OPENAI_API_KEY"] != JsonNull
            if (authMode == "chatgpt" || !hasKey) return CodexAuthMode.CHATGPT_SUBSCRIPTION
        }
        return CodexAuthMode.API_KEY
    }

    /**
     * Set/clear the OPENAI_API_KEY in auth.json (api_key mode only; never touches
     * `auth_mode`/`tokens`, so a chatgpt-subscription agent is left intact).
     */
    fun patchCodexAuth(authJsonText: String, apiKey: String): String {
        val parsed = parseAuthObject(authJsonText)
        val obj = LinkedHashMap<String, JsonElement>(if (parsed.error == null) parsed.obj ?: emptyMap() else emptyMap())
        val key = apiKey.trim()
        if (key.isNotEmpty()) {
            obj["OPENAI_API_KEY"] = JsonPrimitive(key)
            obj.remove("API_KEY")
        } else {
            obj.remove("OPENAI_API_KEY")
            obj.remove("OPENAI_API_TOKEN")
            obj.remove("API_KEY")
        }
        return JsonConfig.serialize(obj)
    }

    // endregion

    // region TOML root writers

    private fun rootEndIndex(lines: List<String>): Int {
        for ((i, l) in lines.withIndex()) {
            if (matchesRe("^\\[.*\\]$", l.trim())) return i
        }
        return lines.size
    }

    private fun rootAssignmentIndex(lines: List<String>, key: String): Int {
        val end = rootEndIndex(lines)
        for (i in 0 until end) if (parseAssignmentKey(lines[i]) == key) return i
        return -1
    }

    private fun preferredRootInsertion(lines: List<String>, key: String): Int {
        if (key == "model") {
            val p = rootAssignmentIndex(lines, "model_provider")
            return if (p >= 0) p else 0
        }
        if (key == "model_reasoning_effort") {
            val m = rootAssignmentIndex(lines, "model")
            return if (m >= 0) m + 1 else 0
        }
        var insertAt = rootEndIndex(lines)
        while (insertAt > 0 && lines[insertAt - 1].trim().isEmpty()) insertAt--
        return insertAt
    }

    fun setRootString(tomlText: String, key: String, value: String): String {
        val ls = lines(tomlText).toMutableList()
        val idx = rootAssignmentIndex(ls, key)
        val next = value.trim()
        if (next.isEmpty()) {
            if (idx >= 0) ls.removeAt(idx)
            return join(ls)
        }
        val lineText = "$key = ${jsonQuote(value)}"
        if (idx >= 0) ls[idx] = lineText
        else ls.add(maxOf(0, preferredRootInsertion(ls, key)), lineText)
        return join(ls)
    }

    fun setRootBool(tomlText: String, key: String, value: Boolean): String {
        val ls = lines(tomlText).toMutableList()
        val idx = rootAssignmentIndex(ls, key)
        val lineText = "$key = ${if (value) "true" else "false"}"
        if (idx >= 0) ls[idx] = lineText else ls.add(0, lineText)
        return join(ls)
    }

    // endregion

    // region TOML section writers

    private fun sectionRange(lines: List<String>, name: String): Pair<Int, Int>? {
        val header = "[$name]"
        var start = -1
        var end = lines.size
        for ((i, l) in lines.withIndex()) {
            val t = l.trim()
            if (start < 0) {
                if (t == header) start = i
                continue
            }
            if (matchesRe("^\\[.*\\]$", t)) { end = i; break }
        }
        return if (start < 0) null else start to end
    }

    /**
     * `upsertTomlSectionBooleanKey` ([value]==null deletes the key, pruning the
     * section if it becomes empty).
     */
    fun upsertSectionBool(tomlText: String, name: String, key: String, value: Boolean?): String {
        val ls = lines(tomlText).toMutableList()
        val sec = sectionRange(ls, name)
        if (sec != null) {
            val (start, end) = sec
            var assignIdx = -1
            for (i in (start + 1) until end) if (parseAssignmentKey(ls[i]) == key) { assignIdx = i; break }
            if (value == null) {
                if (assignIdx >= 0) ls.removeAt(assignIdx)
                val refreshed = sectionRange(ls, name)
                if (refreshed != null) {
                    val (rStart, rEnd) = refreshed
                    val hasEntries = (rStart + 1 until rEnd).any { idx ->
                        val l = ls[idx].trim()
                        l.isNotEmpty() && !l.startsWith("#")
                    }
                    if (!hasEntries) {
                        val before = ls.subList(0, rStart).toMutableList()
                        val after = ls.subList(rEnd, ls.size).toMutableList()
                        while (before.isNotEmpty() && before.last().trim().isEmpty()) before.removeAt(before.size - 1)
                        while (after.isNotEmpty() && after.first().trim().isEmpty()) after.removeAt(0)
                        val merged = if (before.isNotEmpty() && after.isNotEmpty()) before + listOf("") + after else before + after
                        return join(merged)
                    }
                }
                return join(ls)
            }
            val lineText = "$key = ${if (value) "true" else "false"}"
            if (assignIdx >= 0) {
                ls[assignIdx] = lineText
            } else {
                var insertAt = end
                var i = end - 1
                while (i > start) { if (ls[i].trim().isNotEmpty()) { insertAt = i + 1; break }; i-- }
                ls.add(insertAt, lineText)
            }
            return join(ls)
        }
        if (value == null) return tomlText.trim()
        val lineText = "$key = ${if (value) "true" else "false"}"
        val insertAt = rootEndIndex(ls)
        val prefixBlank = if (insertAt > 0 && ls[insertAt - 1].trim().isNotEmpty()) listOf("") else emptyList()
        val suffixBlank = if (insertAt < ls.size && ls[insertAt].trim().isNotEmpty()) listOf("") else emptyList()
        ls.addAll(insertAt, prefixBlank + listOf("[$name]", lineText) + suffixBlank)
        return join(ls)
    }

    /**
     * Upsert a quoted STRING key into a `[name]` section ([value]==null/blank deletes
     * the key, pruning the section if it becomes empty). The string analogue of
     * [upsertSectionBool]; every other line in the file is preserved byte-for-byte,
     * so unrelated blocks (e.g. Grok's `[model.<id>]` / `[session]`) survive untouched.
     */
    fun upsertSectionString(tomlText: String, name: String, key: String, value: String?): String {
        val ls = lines(tomlText).toMutableList()
        val sec = sectionRange(ls, name)
        if (sec != null) {
            val (start, end) = sec
            var assignIdx = -1
            for (i in (start + 1) until end) if (parseAssignmentKey(ls[i]) == key) { assignIdx = i; break }
            if (value == null) {
                if (assignIdx >= 0) ls.removeAt(assignIdx)
                val refreshed = sectionRange(ls, name)
                if (refreshed != null) {
                    val (rStart, rEnd) = refreshed
                    val hasEntries = (rStart + 1 until rEnd).any { idx ->
                        val l = ls[idx].trim()
                        l.isNotEmpty() && !l.startsWith("#")
                    }
                    if (!hasEntries) {
                        val before = ls.subList(0, rStart).toMutableList()
                        val after = ls.subList(rEnd, ls.size).toMutableList()
                        while (before.isNotEmpty() && before.last().trim().isEmpty()) before.removeAt(before.size - 1)
                        while (after.isNotEmpty() && after.first().trim().isEmpty()) after.removeAt(0)
                        val merged = if (before.isNotEmpty() && after.isNotEmpty()) before + listOf("") + after else before + after
                        return join(merged)
                    }
                }
                return join(ls)
            }
            val lineText = "$key = ${jsonQuote(value)}"
            if (assignIdx >= 0) {
                ls[assignIdx] = lineText
            } else {
                var insertAt = end
                var i = end - 1
                while (i > start) { if (ls[i].trim().isNotEmpty()) { insertAt = i + 1; break }; i-- }
                ls.add(insertAt, lineText)
            }
            return join(ls)
        }
        if (value == null) return tomlText.trim()
        val lineText = "$key = ${jsonQuote(value)}"
        val insertAt = rootEndIndex(ls)
        val prefixBlank = if (insertAt > 0 && ls[insertAt - 1].trim().isNotEmpty()) listOf("") else emptyList()
        val suffixBlank = if (insertAt < ls.size && ls[insertAt].trim().isNotEmpty()) listOf("") else emptyList()
        ls.addAll(insertAt, prefixBlank + listOf("[$name]", lineText) + suffixBlank)
        return join(ls)
    }

    /**
     * Merge Grok's two structured controls into `~/.grok/config.toml` client-side:
     * `permission_mode` under `[ui]`, `default_reasoning_effort` under `[models]`
     * (empty string ⇒ "use default" ⇒ remove the key). This mirrors the server's
     * `apply_grok_structured_config` but preserves every other key/block verbatim —
     * unlike sending `grok_structured` (which omits the web-only custom-model /
     * `[session]` fields and so DELETES them). See AgentsViewModel.makeConfigBody.
     */
    fun patchGrok(tomlText: String, permissionMode: String, reasoningEffort: String): String {
        var out = upsertSectionString(tomlText, "ui", "permission_mode", permissionMode.ifEmpty { null })
        out = upsertSectionString(out, "models", "default_reasoning_effort", reasoningEffort.ifEmpty { null })
        return out
    }

    private fun providerSectionRange(lines: List<String>, provider: String): Pair<Int, Int>? {
        val pattern = "^\\[\\s*model_providers\\.${Regex.escape(provider)}\\s*\\]$"
        var start = -1
        var end = lines.size
        for ((i, l) in lines.withIndex()) {
            val t = l.trim()
            if (start < 0) {
                if (matchesRe(pattern, t)) start = i
                continue
            }
            if (matchesRe("^\\[.*\\]$", t)) { end = i; break }
        }
        return if (start < 0) null else start to end
    }

    private fun patchProviderBaseUrl(tomlText: String, provider: String, apiBaseUrl: String): String {
        val prov = provider.trim()
        if (prov.isEmpty()) return tomlText.trim()
        val next = apiBaseUrl.trim()
        val ls = lines(tomlText).toMutableList()
        val sec = providerSectionRange(ls, prov)
        if (sec != null) {
            val (start, end) = sec
            var baseIdx = -1
            for (i in (start + 1) until end) {
                val a = parseStringAssignment(ls[i])
                if (a != null && a.first == "base_url") { baseIdx = i; break }
            }
            if (next.isEmpty()) {
                if (baseIdx >= 0) ls.removeAt(baseIdx)
                return join(ls)
            }
            val lineText = "base_url = ${jsonQuote(next)}"
            if (baseIdx >= 0) ls[baseIdx] = lineText else ls.add(end, lineText)
            return join(ls)
        }
        if (next.isEmpty()) return tomlText.trim()
        val appended = tomlText.trim { it == '\n' || it == '\r' || it == '\t' }.trimEnd('\n', '\r')
        val sectionText = "[model_providers.$prov]\nbase_url = ${jsonQuote(next)}"
        return if (appended.isEmpty()) sectionText else "$appended\n\n$sectionText".trim()
    }

    private fun patchProviderField(tomlText: String, provider: String, key: String, lineText: String): String {
        val prov = provider.trim()
        if (prov.isEmpty()) return tomlText.trim()
        val ls = lines(tomlText).toMutableList()
        val sec = providerSectionRange(ls, prov)
        if (sec != null) {
            val (start, end) = sec
            var fieldIdx = -1
            for (i in (start + 1) until end) if (parseAssignmentKey(ls[i]) == key) { fieldIdx = i; break }
            if (fieldIdx >= 0) {
                ls[fieldIdx] = lineText
            } else {
                var insertAt = end
                while (insertAt > start + 1 && ls[insertAt - 1].trim().isEmpty()) insertAt--
                ls.add(insertAt, lineText)
            }
            return join(ls)
        }
        val appended = tomlText.trimEnd('\n', '\r')
        val sectionText = "[model_providers.$prov]\n$lineText"
        return if (appended.isEmpty()) sectionText else "$appended\n\n$sectionText".trim()
    }

    private fun ensureProviderDefaults(tomlText: String, provider: String): String {
        if (provider.trim() != CODEX_DEFAULT_MODEL_PROVIDER) return tomlText
        var next = tomlText
        val current = extractCodexToml(next)
        val codegBaseUrl = current.providerBaseUrls[CODEX_DEFAULT_MODEL_PROVIDER] ?: ""
        // Only (re)write base_url when non-empty — an empty value means the user
        // cleared it, so leave it deleted rather than re-adding `base_url = ""`.
        if (codegBaseUrl.isNotEmpty()) {
            next = patchProviderField(next, CODEX_DEFAULT_MODEL_PROVIDER, "base_url", "base_url = ${jsonQuote(codegBaseUrl)}")
        }
        next = patchProviderField(next, CODEX_DEFAULT_MODEL_PROVIDER, "name", "name = \"codeg\"")
        next = patchProviderField(next, CODEX_DEFAULT_MODEL_PROVIDER, "wire_api", "wire_api = \"responses\"")
        next = patchProviderField(next, CODEX_DEFAULT_MODEL_PROVIDER, "requires_openai_auth", "requires_openai_auth = true")
        return next
    }

    // endregion

    // region patchCodexConfigTomlText (api_key mode; no model_provider link)

    fun patchCodex(tomlText: String, d: AgentDraft): String {
        var next = tomlText

        next = setRootString(next, "model", d.model)
        next = setRootString(next, "model_reasoning_effort", d.codexReasoningEffort.wire)

        // apiBaseUrl → active provider's base_url (default "codeg" when none set).
        run {
            val toml = extractCodexToml(next)
            val provider = if (toml.modelProvider.trim().isNotEmpty()) toml.modelProvider.trim() else CODEX_DEFAULT_MODEL_PROVIDER
            if (toml.modelProvider.trim().isEmpty() && d.apiBaseUrl.trim().isNotEmpty()) {
                next = setRootString(next, "model_provider", provider)
            }
            next = patchProviderBaseUrl(next, provider, d.apiBaseUrl)
            next = ensureProviderDefaults(next, provider)
        }

        // supports_websockets → active provider (default "codeg" when none set).
        run {
            val toml = extractCodexToml(next)
            val provider = if (toml.modelProvider.trim().isNotEmpty()) toml.modelProvider.trim() else CODEX_DEFAULT_MODEL_PROVIDER
            if (toml.modelProvider.trim().isEmpty()) {
                next = setRootString(next, "model_provider", provider)
            }
            next = patchProviderField(
                next, provider, "supports_websockets",
                "supports_websockets = ${if (d.codexSupportsWebsockets) "true" else "false"}",
            )
            next = ensureProviderDefaults(next, provider)
        }

        // Re-normalize root model / reasoning effort, derive the feature flag.
        val normalized = extractCodexToml(next)
        if (normalized.model.trim().isNotEmpty()) next = setRootString(next, "model", normalized.model)
        next = setRootString(next, "model_reasoning_effort", normalized.modelReasoningEffort.wire)
        val active = if (normalized.modelProvider.trim().isNotEmpty()) normalized.modelProvider.trim() else CODEX_DEFAULT_MODEL_PROVIDER
        val featureOn = normalized.providerSupportsWebsockets[active] ?: false
        next = upsertSectionBool(next, "features", "responses_websockets_v2", if (featureOn) true else null)
        next = upsertSectionBool(next, "features", "skills", if (d.codexSkills) true else null)
        next = setRootString(next, "service_tier", if (d.codexServiceTierFast) "fast" else "")
        next = setRootBool(next, "disable_response_storage", true)

        val trimmed = next.trim()
        return if (trimmed.isEmpty()) "" else "$trimmed\n"
    }

    // endregion
}
