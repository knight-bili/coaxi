package app.codeg.android.feature.sessiondetail.rendering

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.ErrorOutline
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.codeg.android.R
import app.codeg.android.core.designsystem.component.CodeBlock
import app.codeg.android.core.designsystem.component.CopyButton
import app.codeg.android.core.designsystem.theme.CodegTheme
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive

/**
 * Visual rendering for tool-call *input arguments* and *output*, so an MCP /
 * generic tool call (e.g. `delegate_to_agent`) reads as structured `label: value`
 * fields and clean terminal / markdown output instead of a dumped raw-JSON string.
 * Faithful port of the iOS `ToolValueFormat.swift` (which mirrors the codeg web
 * client's `GenericToolInput` / `ToolOutput` / `commandOutputFromJsonString`
 * helpers) so the two clients render the same payloads alike.
 */

// region Pure JSON helpers

private val lenient = Json { ignoreUnknownKeys = true; isLenient = true }
private val prettyJson = Json { prettyPrint = true; ignoreUnknownKeys = true; isLenient = true }

/** Parse a JSON string allowing a top-level fragment (a bare `"string"` / number) —
 *  command results are sometimes a JSON-encoded scalar. */
private fun parseAny(s: String?): JsonElement? {
    if (s.isNullOrBlank()) return null
    return runCatching { lenient.parseToJsonElement(s) }.getOrNull()
}

/** A string field of this object, or null when absent / not a string. */
private fun JsonObject.strField(key: String): String? {
    val prim = this[key] as? JsonPrimitive ?: return null
    return if (prim.isString) prim.content else null
}

/** A short scalar rendering for inline display ("true", "42", "1.5", "text"). */
private fun scalarString(prim: JsonPrimitive): String = prim.content

/** Pretty-print a JSON value for a code block; a bare string yields its content. */
private fun prettyPrint(el: JsonElement): String = when (el) {
    is JsonPrimitive -> el.content
    else -> runCatching { prettyJson.encodeToString(JsonElement.serializer(), el) }.getOrElse { el.toString() }
}

/** Re-serialize a JSON-object/array string pretty-printed; null if it isn't one
 *  (a bare scalar isn't worth a "json" block). */
fun prettyPrintJsonString(s: String): String? {
    val any = parseAny(s) ?: return null
    if (any is JsonPrimitive) return null
    return prettyPrint(any)
}

/** Fields rendered as a code block when they carry a string value. */
private val codeFields = setOf(
    "command", "cmd", "script", "old_string", "new_string",
    "content", "new_source", "prompt", "code", "patch", "diff",
)

/** Fields never shown (noise / internal flags). Mirrors web `HIDDEN_FIELDS`. */
private val hiddenFields = setOf("dangerouslyDisableSandbox")

// endregion

// region Output normalization

/** Turns a tool's raw output (often a JSON envelope) into clean displayable text
 *  and classifies its kind. Mirrors the web `ToolOutput` pipeline. */
object ToolOutputFormat {

    enum class Kind { JSON, DIFF, MARKDOWN, LOG }

    /** Pull human-readable text out of a JSON command-result envelope
     *  (`{stdout, stderr, exit_code, formatted_output, …}`), or null when the
     *  output isn't such an envelope. */
    fun commandOutput(output: String): String? {
        val parsed = parseAny(output) ?: return null
        if (parsed is JsonPrimitive && parsed.isString) return parsed.content
        val obj = parsed as? JsonObject ?: return null

        val envelopeKeys = listOf(
            "command", "parsed_cmd", "cwd", "exit_code",
            "stdout", "stderr", "formatted_output", "aggregated_output",
        )
        val isEnvelope = envelopeKeys.any { obj[it] != null }

        val stdout = obj.strField("stdout") ?: ""
        val stderr = obj.strField("stderr") ?: ""
        if (stdout.isNotEmpty() || stderr.isNotEmpty()) {
            if (stdout.isNotEmpty() && stderr.isNotEmpty()) return "$stdout\n[stderr]\n$stderr"
            return stdout.ifEmpty { stderr }
        }
        for (key in listOf("formatted_output", "aggregated_output", "output", "text", "result")) {
            obj.strField(key)?.takeIf { it.isNotEmpty() }?.let { return it }
        }
        // Metadata-only envelope (command/cwd/exit_code, no body) → empty, so we don't
        // fall back to dumping the raw JSON as terminal output.
        return if (isEnvelope) "" else null
    }

    private val metaLine = Regex(
        "^(exit code\\s*[:=]|wall time\\s*[:=]|chunk id\\s*[:=]|original token count\\s*[:=]|total output lines\\s*[:=]|process exited with code\\s)",
        RegexOption.IGNORE_CASE,
    )
    private val outputSepLine = Regex("^output:\\s*$", RegexOption.IGNORE_CASE)

    private fun isMetaLine(s: String) = metaLine.containsMatchIn(s)

    /** Strip CLI execution-envelope metadata ("Chunk ID:", "Wall time:", "Output:"
     *  separator, …) leaving the real command output. Mirrors web `parseCliExecutionEnvelope`. */
    fun parseCliEnvelope(text: String): String {
        val lines = text.split("\n")
        var outputSep = -1
        for ((idx, line) in lines.withIndex()) {
            val trimmed = line.trim()
            if (outputSepLine.matches(trimmed)) { outputSep = idx; break }
            if (!isMetaLine(trimmed) && trimmed.isNotEmpty()) break
        }
        if (outputSep >= 0) {
            var start = outputSep + 1
            while (start < lines.size) {
                val trimmed = lines[start].trim()
                if (isMetaLine(trimmed) || trimmed.isEmpty()) { start++; continue }
                break
            }
            return lines.subList(start, lines.size).joinToString("\n")
        }
        var index = 0
        var sawMeta = false
        while (index < lines.size) {
            val trimmed = lines[index].trim()
            if (isMetaLine(trimmed)) { sawMeta = true; index++; continue }
            if (sawMeta && trimmed.isEmpty()) { index++; continue }
            break
        }
        if (!sawMeta) return text
        while (index < lines.size && lines[index].trim().isEmpty()) index++
        return lines.subList(index, lines.size).joinToString("\n")
    }

    /** Remove a single leading / trailing Markdown code fence (```sh … ```). */
    fun stripMarkdownFence(text: String): String {
        var result = text
        Regex("^\\s*```[\\w-]*\\s*\\n?").find(result)?.let { result = result.removeRange(it.range) }
        Regex("\\n?\\s*```\\s*$").find(result)?.let { result = result.removeRange(it.range) }
        return result
    }

    /** Full command-output pipeline: unwrap a JSON envelope, strip CLI metadata,
     *  drop a wrapping code fence. */
    fun cleanCommandOutput(source: String): String {
        val unwrapped = commandOutput(source) ?: source
        return stripMarkdownFence(parseCliEnvelope(unwrapped))
    }

    /** Classify finalized output so the renderer can pick a JSON block, a diff view,
     *  Markdown, or a plain log block. */
    fun classify(output: String): Kind {
        val trimmed = output.trim()
        if ((trimmed.startsWith("{") || trimmed.startsWith("[")) && parseAny(trimmed) != null) return Kind.JSON
        if (trimmed.contains("diff --git") || Regex("(?m)^@@ ").containsMatchIn(trimmed)) return Kind.DIFF
        return if (looksLikeMarkdown(output)) Kind.MARKDOWN else Kind.LOG
    }

    /** ≥2 markdown signals → render the output as markdown (iOS heuristic). */
    fun looksLikeMarkdown(s: String): Boolean {
        var count = 0
        val patterns = listOf(
            "(?m)^#{1,6}\\s", "(?m)^\\s*[-*+]\\s", "(?m)^\\s*\\d+\\.\\s",
            "\\*\\*[^*]+\\*\\*", "\\[[^\\]]+\\]\\([^)]+\\)", "```", "(?m)^>\\s", "(?m)^\\|.+\\|$",
        )
        for (p in patterns) {
            if (Regex(p).containsMatchIn(s)) count++
            if (count >= 2) return true
        }
        return false
    }

    /**
     * The displayable text inside an MCP `CallToolResult`, after stripping the host
     * envelope Codex wraps around EVERY MCP result on its live wire
     * (`{result: <CallToolResult>, error: null}`). Null when [output] isn't such a
     * payload at all, so the caller falls through to its normal handling.
     *
     * Without this the whole envelope reaches the renderer as opaque JSON and every
     * MCP tool on Codex (`delegate_to_agent`, context7, codeg-mcp, …) shows raw braces
     * instead of the text the tool returned.
     */
    fun mcpResultText(output: String): String? {
        val obj = ToolDerive.parseJson(output) ?: return null
        val peeled = McpResultEnvelope.peel(obj, McpResultEnvelope::isCallToolResult)
        // A host envelope that failed outright carries no result to render — its own
        // error string is the whole story, and beats dumping the envelope JSON.
        peeled.hostError?.let { return it }
        if (!McpResultEnvelope.isCallToolResult(peeled.obj)) return null
        val text = (peeled.obj["content"] as? JsonArray)
            ?.mapNotNull { (it as? JsonObject)?.strField("text")?.takeIf(String::isNotEmpty) }
            ?.joinToString("\n")
            .orEmpty()
        if (text.isNotEmpty()) return text
        // No text parts: fall back to the structured payload, pretty-printed.
        return (peeled.obj["structuredContent"] as? JsonObject)?.let { prettyPrint(it) }
    }

    /** Parse a JSON-object error body into labeled `(key, value)` fields, or null for
     *  a plain-text error. Mirrors web `renderErrorText`. */
    fun errorFields(errorText: String): List<Pair<String, String>>? {
        val obj = ToolDerive.parseJson(errorText.trim()) ?: return null
        val entries = obj.entries.filter { it.value !is JsonNull }
        if (entries.isEmpty()) return null
        return entries.map { (key, value) ->
            key to (if (value is JsonPrimitive && value.isString) value.content else prettyPrint(value))
        }
    }
}

/** A Read tool's output: the codeg server returns a file body as a JSON envelope
 *  `{"start_line": N, "content": "..."}`. Null for anything else. */
object ReadOutputFormat {
    data class File(val content: String, val startLine: Int)

    fun parse(output: String): File? {
        val obj = ToolDerive.parseJson(output) ?: return null
        val startPrim = obj["start_line"] as? JsonPrimitive ?: return null
        if (startPrim.isString) return null
        val start = startPrim.content.toDoubleOrNull()?.toInt() ?: return null
        val content = obj.strField("content") ?: return null
        return File(content, start)
    }
}

// endregion

// region Friendly field labels

/** Human labels for common tool-argument keys (mirrors web `FIELD_LABEL_KEYS`).
 *  Unknown keys fall back to a humanized form of the key. */
object ToolFieldLabel {
    private val map = mapOf(
        "file_path" to "File", "notebook_path" to "Notebook", "path" to "Path",
        "command" to "Command", "cmd" to "Command", "script" to "Script",
        "old_string" to "Replace", "new_string" to "With",
        "pattern" to "Pattern", "query" to "Query", "url" to "URL",
        "description" to "Description", "content" to "Content", "new_source" to "Source",
        "prompt" to "Prompt", "subject" to "Subject", "task" to "Task",
        "task_id" to "Task", "task_ids" to "Tasks", "taskId" to "Task",
        "status" to "Status", "skill" to "Skill", "args" to "Args",
        "offset" to "Offset", "limit" to "Limit", "glob" to "Glob",
        "type" to "Type", "output_mode" to "Output", "replace_all" to "Replace all",
        "language" to "Language", "timeout" to "Timeout", "wait_ms" to "Wait",
        "run_in_background" to "Background", "background" to "Background",
        "subagent_type" to "Agent", "agent_type" to "Agent",
        "libraryName" to "Library", "libraryId" to "Library ID",
        "working_dir" to "Directory", "cwd" to "Directory",
    )

    fun label(key: String): String = map[key] ?: humanize(key)

    /** `read_only` → "Read only", `timeoutMs` → "Timeout ms". */
    private fun humanize(key: String): String {
        var spaced = key.replace("_", " ")
        spaced = spaced.replace(Regex("([a-z0-9])([A-Z])"), "$1 $2")
        spaced = spaced.lowercase().trim()
        val first = spaced.firstOrNull() ?: return key
        return first.uppercase() + spaced.drop(1)
    }
}

// endregion

// region Field composables

/** `label: value` on one line — short scalars. */
@Composable
fun ToolFieldInline(label: String, value: String) {
    val colors = CodegTheme.colors
    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(label, fontSize = 11.sp, fontWeight = FontWeight.SemiBold, color = colors.textTertiary)
        Text(
            value,
            fontFamily = FontFamily.Monospace,
            fontSize = 11.5.sp,
            color = colors.textSecondary,
            modifier = Modifier.weight(1f),
        )
    }
}

/** A field label above a block child (code / long text / nested JSON). */
@Composable
fun ToolFieldBlock(label: String, content: @Composable () -> Unit) {
    val colors = CodegTheme.colors
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Text(label, fontSize = 11.sp, fontWeight = FontWeight.SemiBold, color = colors.textTertiary)
        content()
    }
}

/** A header-less monospaced block for a long plain-string value. */
@Composable
fun PlainTextBlock(text: String) {
    val colors = CodegTheme.colors
    Row(
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(8.dp))
            .background(colors.codeSurface)
            .border(0.5.dp, colors.hairline, RoundedCornerShape(8.dp))
            .horizontalScroll(rememberScrollState())
            .padding(horizontal = 12.dp, vertical = 10.dp),
    ) {
        Text(text, fontFamily = FontFamily.Monospace, fontSize = 11.5.sp, color = colors.textSecondary)
    }
}

/** Renders a JSON object's arguments as a structured field list (label + value) —
 *  the universal fallback for MCP and otherwise-unrecognized tools (incl.
 *  `delegate_to_agent`), replacing a dumped raw-JSON code block. Falls back to a
 *  pretty JSON block for an array, or a plain block for a non-object string. */
@Composable
fun StructuredJsonView(json: String, inlineMaxLength: Int = 200) {
    val obj = ToolDerive.parseJson(json)
    if (obj != null) {
        val entries = obj.entries.filter { it.key !in hiddenFields && it.value !is JsonNull }
        if (entries.isNotEmpty()) {
            Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                entries.forEach { (key, value) -> ToolField(key, value, inlineMaxLength) }
            }
        }
        return
    }
    if (json.isNotBlank()) {
        val any = parseAny(json)
        if (any is JsonArray) CodeBlock(prettyPrint(any), language = "json")
        else PlainTextBlock(json)
    }
}

@Composable
private fun ToolField(key: String, value: JsonElement, inlineMaxLength: Int) {
    val label = ToolFieldLabel.label(key)
    when {
        value is JsonPrimitive && value.isString -> {
            val s = value.content
            when {
                key in codeFields -> ToolFieldBlock(label) { CodeBlock(s, language = codeLanguage(key)) }
                s.length > inlineMaxLength || s.contains("\n") -> ToolFieldBlock(label) { PlainTextBlock(s) }
                else -> ToolFieldInline(label, s)
            }
        }
        value is JsonPrimitive -> ToolFieldInline(label, scalarString(value))
        else -> ToolFieldBlock(label) { CodeBlock(prettyPrint(value), language = "json") }
    }
}

private fun codeLanguage(key: String): String? = when (key) {
    "command", "cmd", "script" -> "bash"
    "patch", "diff" -> "diff"
    else -> null
}

/** A file body with a right-aligned line-number gutter — a Read tool's output.
 *  Numbers start at the envelope's `start_line`. Long files collapse behind a
 *  toggle. */
@Composable
fun FileBodyView(file: ReadOutputFormat.File, collapsedLineLimit: Int = 60) {
    val colors = CodegTheme.colors
    val content = remember(file) { file.content.trimEnd('\n', '\r') }
    val lines = remember(content) { content.split("\n") }
    val isLong = lines.size > collapsedLineLimit
    var expanded by remember(file) { mutableStateOf(false) }
    val shown = if (isLong && !expanded) lines.take(collapsedLineLimit) else lines
    val maxNo = file.startLine + lines.size - 1
    val digits = maxOf(2, maxNo.toString().length)
    val gutterWidth = (digits * 8 + 6).dp

    Column(
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(8.dp))
            .background(colors.codeSurface)
            .border(0.5.dp, colors.hairline, RoundedCornerShape(8.dp)),
    ) {
        Row(
            Modifier.fillMaxWidth().padding(start = 10.dp, end = 4.dp, top = 2.dp, bottom = 2.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text("FILE", fontFamily = FontFamily.Monospace, fontSize = 10.sp, fontWeight = FontWeight.SemiBold, color = colors.textTertiary)
            Row(Modifier.weight(1f)) {}
            CopyButton(content)
        }
        HorizontalDivider(thickness = 0.5.dp, color = colors.hairline)
        Column(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(vertical = 8.dp)) {
            shown.forEachIndexed { idx, line ->
                Row(Modifier.padding(vertical = 1.dp)) {
                    Text(
                        (file.startLine + idx).toString(),
                        fontFamily = FontFamily.Monospace,
                        fontSize = 12.sp,
                        color = colors.textTertiary.copy(alpha = 0.7f),
                        textAlign = TextAlign.End,
                        modifier = Modifier.width(gutterWidth).padding(end = 8.dp),
                    )
                    Text(
                        line.ifEmpty { " " },
                        fontFamily = FontFamily.Monospace,
                        fontSize = 12.5.sp,
                        color = colors.textSecondary,
                        modifier = Modifier.padding(end = 12.dp),
                    )
                }
            }
        }
        if (isLong) {
            Text(
                if (expanded) stringResource(R.string.code_show_less) else stringResource(R.string.code_show_more_lines, lines.size - collapsedLineLimit),
                fontSize = 11.sp,
                fontWeight = FontWeight.SemiBold,
                color = colors.accent,
                modifier = Modifier.fillMaxWidth().clickable { expanded = !expanded }.padding(horizontal = 12.dp, vertical = 7.dp),
            )
        }
    }
}

/** Error output. A JSON-object error body is split into labeled fields (so an
 *  `{error, code, detail}` envelope is readable); plain text falls back to a mono
 *  block. */
@Composable
fun ToolErrorOutput(output: String) {
    val colors = CodegTheme.colors
    val fields = remember(output) { ToolOutputFormat.errorFields(output) }
    Column(
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(8.dp))
            .background(colors.danger.copy(alpha = 0.08f))
            .border(0.5.dp, colors.danger.copy(alpha = 0.4f), RoundedCornerShape(8.dp))
            .padding(horizontal = 12.dp, vertical = 10.dp),
        verticalArrangement = Arrangement.spacedBy(7.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(5.dp)) {
            Icon(Icons.Rounded.ErrorOutline, null, tint = colors.danger, modifier = Modifier.size(13.dp))
            Text(stringResource(R.string.tool_error_output), fontSize = 11.sp, fontWeight = FontWeight.SemiBold, color = colors.danger)
        }
        if (fields != null) {
            fields.forEach { (key, value) ->
                Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    Text(key.uppercase(), fontSize = 10.sp, fontWeight = FontWeight.SemiBold, color = colors.danger.copy(alpha = 0.7f))
                    Text(value, fontFamily = FontFamily.Monospace, fontSize = 11.5.sp, color = colors.danger.copy(alpha = 0.92f))
                }
            }
        } else {
            Text(output, fontFamily = FontFamily.Monospace, fontSize = 12.sp, color = colors.danger.copy(alpha = 0.92f))
        }
    }
}

// endregion
