package app.codeg.android.feature.sessiondetail.interactive

import app.codeg.android.core.designsystem.diff.DiffFile
import app.codeg.android.core.designsystem.diff.UnifiedDiff
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive

data class PermissionPlanEntry(val text: String, val status: String?)
data class PermissionAllowedPrompt(val prompt: String, val tool: String)

/**
 * Everything the permission card needs, extracted from the freeform `tool_call`
 * JSON of a `permission_request`. Faithful port of the iOS `ParsedPermission` /
 * `PermissionParse` (which mirrors the web `parsePermissionToolCall`).
 */
data class ParsedPermission(
    val title: String,
    val kind: String,
    val command: String?,
    val cwd: String?,
    val diffFiles: List<DiffFile>,
    val planMarkdown: String?,
    val planEntries: List<PermissionPlanEntry>,
    val allowedPrompts: List<PermissionAllowedPrompt>,
    val modeTarget: String?,
    val url: String?,
    val query: String?,
    val prompt: String?,
    val jsonPreview: String,
) {
    /** True for an ExitPlanMode / plan-bearing approval (drives plan-styled copy). */
    val isPlan: Boolean
        get() {
            val k = kind.lowercase().replace("_", "")
            return k.contains("plan") || planMarkdown != null || planEntries.isNotEmpty()
        }

    /** Whether the body has anything richer than the raw-JSON fallback. */
    val hasStructuredBody: Boolean
        get() = command != null || diffFiles.isNotEmpty() || planMarkdown != null ||
            planEntries.isNotEmpty() || allowedPrompts.isNotEmpty() ||
            modeTarget != null || url != null || query != null || prompt != null

    companion object {
        fun parse(toolCall: JsonElement): ParsedPermission {
            val obj = toolCall as? JsonObject
            val rawKind = pickString(obj, listOf("kind", "toolName", "tool_name", "name", "type")) ?: "tool"

            val rawInputValue = pickValue(obj, listOf("rawInput", "raw_input", "input", "arguments", "params", "payload"))
            val rawInputObj = asObject(rawInputValue)

            val command = extractCommand(rawInputValue) ?: extractCommand(toolCall)
            val cwd = pickString(rawInputObj, listOf("cwd", "workdir", "workingDirectory", "working_directory"))
                ?: pickString(obj, listOf("cwd", "workdir", "workingDirectory", "working_directory"))

            val diffText = extractDiffText(rawInputValue, rawInputObj)
                ?: buildDiffFromChanges(rawInputObj, obj)
            val diffFiles = diffText?.let { UnifiedDiff.parse(it) } ?: emptyList()

            val planEntries = parsePlanEntries(rawInputObj)
            val planMarkdown = (pickValue(rawInputObj, listOf("plan")) as? JsonPrimitive)
                ?.takeIf { it.isString }?.content?.trim()?.takeIf { it.isNotEmpty() }

            val allowedPrompts = parseAllowedPrompts(rawInputObj)
            val modeTarget = pickString(rawInputObj, listOf("modeId", "mode_id", "targetMode", "target_mode"))
            val url = pickString(rawInputObj, listOf("url")) ?: pickString(obj, listOf("url"))
            val query = pickString(rawInputObj, listOf("query")) ?: pickString(obj, listOf("query"))
            val prompt = pickString(rawInputObj, listOf("prompt")) ?: pickString(obj, listOf("prompt"))
            val title = pickString(obj, listOf("title", "toolName", "tool_name", "name")) ?: fallbackTitle(rawKind)

            return ParsedPermission(
                title = title, kind = rawKind, command = command, cwd = cwd, diffFiles = diffFiles,
                planMarkdown = planMarkdown, planEntries = planEntries, allowedPrompts = allowedPrompts,
                modeTarget = modeTarget, url = url, query = query, prompt = prompt,
                jsonPreview = prettyPrint(toolCall),
            )
        }
    }
}

private val PJson = Json { ignoreUnknownKeys = true; isLenient = true }
private val PrettyJson = Json { prettyPrint = true; ignoreUnknownKeys = true; isLenient = true }

private fun JsonElement?.str(): String? = (this as? JsonPrimitive)?.let { if (it.isString) it.content else null }
private fun JsonElement?.nonEmpty(): String? = str()?.takeIf { it.isNotEmpty() }
private fun JsonElement?.isNullElem(): Boolean = this == null || this is JsonNull
private fun parseJsonString(s: String): JsonElement? = runCatching { PJson.parseToJsonElement(s) }.getOrNull()

private fun asObject(v: JsonElement?): JsonObject? {
    if (v is JsonObject) return v
    if (v is JsonPrimitive && v.isString) {
        val t = v.content.trim()
        if (t.startsWith("{")) return parseJsonString(t) as? JsonObject
    }
    return null
}

private fun pickValue(obj: JsonObject?, keys: List<String>): JsonElement? {
    if (obj == null) return null
    for (k in keys) obj[k]?.takeIf { !it.isNullElem() }?.let { return it }
    return null
}

private fun pickString(obj: JsonObject?, keys: List<String>): String? {
    if (obj == null) return null
    for (k in keys) obj[k].nonEmpty()?.let { return it }
    return null
}

private fun fallbackTitle(kind: String): String {
    val normalized = kind.replace("_", " ").trim()
    if (normalized.isEmpty()) return ""
    return normalized.split(" ").filter { it.isNotEmpty() }
        .joinToString(" ") { it.replaceFirstChar { c -> c.uppercase() } }
}

private fun extractCommand(value: JsonElement?, depth: Int = 0): String? {
    if (depth > 4 || value == null || value.isNullElem()) return null
    when (value) {
        is JsonPrimitive -> {
            if (!value.isString) return null
            val t = value.content.trim()
            if (t.isEmpty() || UnifiedDiff.looksLikeDiff(t)) return null
            if (!t.startsWith("{") && !t.startsWith("[")) return t
            return extractCommand(parseJsonString(t), depth + 1)
        }
        is JsonArray -> {
            val parts = value.mapNotNull { it.str() }.filter { it.trim().isNotEmpty() }
            val joined = parts.joinToString(" ").trim()
            return joined.ifEmpty { null }
        }
        is JsonObject -> {
            for (key in listOf("command", "cmd", "script", "args", "argv", "command_args", "commandArgs")) {
                extractCommand(value[key], depth + 1)?.let { return it }
            }
            for (key in listOf("rawInput", "raw_input", "input", "arguments", "params", "payload")) {
                extractCommand(value[key], depth + 1)?.let { return it }
            }
            return null
        }
        else -> return null
    }
}

private fun extractDiffText(rawInput: JsonElement?, rawInputObj: JsonObject?): String? {
    val candidates = mutableListOf<JsonElement?>(rawInput)
    if (rawInputObj != null) {
        candidates.addAll(listOf(rawInputObj["patch"], rawInputObj["diff"], rawInputObj["unified_diff"], rawInputObj["unifiedDiff"]))
    }
    for (c in candidates) {
        val s = c.str() ?: continue
        val normalized = unescape(s).trim()
        if (normalized.isNotEmpty() && UnifiedDiff.looksLikeDiff(normalized)) return normalized
    }
    return null
}

private fun buildDiffFromChanges(rawInputObj: JsonObject?, toolCallObj: JsonObject?): String? {
    val blocks = mutableListOf<String>()

    if (rawInputObj != null) {
        val path = pickString(rawInputObj, listOf("file_path", "filePath", "path", "notebook_path", "notebookPath", "target_file", "targetFile"))
        if (path != null) {
            val oldText = pickString(rawInputObj, listOf("old_string", "oldString", "old_text", "oldText")) ?: ""
            val newText = pickString(rawInputObj, listOf("new_string", "newString", "new_text", "newText", "content", "text", "new_source", "newSource")) ?: ""
            compactDiff(path, oldText, newText)?.let { blocks.add(it) }
        }
    }

    (toolCallObj?.get("content") as? JsonArray)?.forEach { item ->
        val rec = item as? JsonObject ?: return@forEach
        if (rec["type"].str()?.lowercase() != "diff") return@forEach
        val path = rec["path"].nonEmpty() ?: return@forEach
        val oldText = (rec["old_text"] ?: rec["oldText"]).str() ?: ""
        val newText = (rec["new_text"] ?: rec["newText"]).str() ?: ""
        compactDiff(path, oldText, newText)?.let { blocks.add(it) }
    }

    val joined = blocks.joinToString("\n\n").trim()
    return joined.ifEmpty { null }
}

private fun compactDiff(path: String, oldText: String, newText: String, context: Int = 2): String? {
    val oldLines = oldText.split("\n").toMutableList()
    if (oldLines.lastOrNull() == "") oldLines.removeAt(oldLines.size - 1)
    val newLines = newText.split("\n").toMutableList()
    if (newLines.lastOrNull() == "") newLines.removeAt(newLines.size - 1)

    var prefix = 0
    while (prefix < oldLines.size && prefix < newLines.size && oldLines[prefix] == newLines[prefix]) prefix++
    var suffix = 0
    while (suffix < oldLines.size - prefix && suffix < newLines.size - prefix &&
        oldLines[oldLines.size - 1 - suffix] == newLines[newLines.size - 1 - suffix]
    ) suffix++

    val removed = oldLines.subList(prefix, oldLines.size - suffix)
    val added = newLines.subList(prefix, newLines.size - suffix)
    if (removed.isEmpty() && added.isEmpty()) return null

    val before = oldLines.subList(maxOf(0, prefix - context), prefix)
    val after = oldLines.subList(oldLines.size - suffix, minOf(oldLines.size, oldLines.size - suffix + context))

    val oldStart = maxOf(1, prefix + 1 - before.size)
    val oldCount = before.size + removed.size + after.size
    val newCount = before.size + added.size + after.size

    val parts = mutableListOf("--- $path", "+++ $path", "@@ -$oldStart,$oldCount +$oldStart,$newCount @@")
    before.forEach { parts.add(" $it") }
    removed.forEach { parts.add("-$it") }
    added.forEach { parts.add("+$it") }
    after.forEach { parts.add(" $it") }
    return parts.joinToString("\n")
}

private fun parsePlanEntries(rawInputObj: JsonObject?): List<PermissionPlanEntry> {
    if (rawInputObj == null) return emptyList()
    for (key in listOf("plan", "entries", "steps", "todos")) {
        val list = rawInputObj[key] as? JsonArray ?: continue
        if (list.isEmpty()) continue
        val entries = list.mapNotNull { item ->
            val rec = item as? JsonObject ?: return@mapNotNull null
            val text = pickString(rec, listOf("step", "content", "title", "task", "description")) ?: return@mapNotNull null
            PermissionPlanEntry(text, pickString(rec, listOf("status", "state")))
        }
        if (entries.isNotEmpty()) return entries
    }
    return emptyList()
}

private fun parseAllowedPrompts(rawInputObj: JsonObject?): List<PermissionAllowedPrompt> {
    val list = pickValue(rawInputObj, listOf("allowedPrompts", "allowed_prompts")) as? JsonArray ?: return emptyList()
    return list.mapNotNull { item ->
        val rec = item as? JsonObject ?: return@mapNotNull null
        val prompt = pickString(rec, listOf("prompt", "description", "text")) ?: return@mapNotNull null
        PermissionAllowedPrompt(prompt, pickString(rec, listOf("tool", "toolName", "tool_name")) ?: "")
    }
}

private fun unescape(text: String): String =
    text.replace("\\r\\n", "\n").replace("\\n", "\n").replace("\\t", "\t")

private fun prettyPrint(element: JsonElement): String =
    runCatching { PrettyJson.encodeToString(JsonElement.serializer(), element) }.getOrDefault(element.toString())
