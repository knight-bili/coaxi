package app.codeg.android.feature.sessiondetail.rendering

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.tween
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Adjust
import androidx.compose.material.icons.rounded.Build
import androidx.compose.material.icons.rounded.Checklist
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.CheckCircle
import androidx.compose.material.icons.rounded.Description
import androidx.compose.material.icons.rounded.Edit
import androidx.compose.material.icons.rounded.ErrorOutline
import androidx.compose.material.icons.rounded.ExpandLess
import androidx.compose.material.icons.rounded.ExpandMore
import androidx.compose.material.icons.rounded.Groups
import androidx.compose.material.icons.rounded.Language
import androidx.compose.material.icons.rounded.Layers
import androidx.compose.material.icons.rounded.RadioButtonUnchecked
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material.icons.rounded.Terminal
import androidx.compose.material3.CircularProgressIndicator
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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import android.content.Context
import androidx.compose.ui.platform.LocalContext
import app.codeg.android.R
import app.codeg.android.core.designsystem.component.CodeBlock
import app.codeg.android.core.designsystem.component.LivePulse
import app.codeg.android.core.designsystem.diff.DiffView
import app.codeg.android.core.designsystem.diff.UnifiedDiff
import app.codeg.android.core.designsystem.markdown.MarkdownContent
import app.codeg.android.core.designsystem.theme.CodegTheme
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive

private val SuccessGreen = Color(0xFF85D18F)

/** A collapsible card for a single tool call. Port of iOS `ToolCallCard`. */
@Composable
fun ToolCallCard(vm: ToolCallVM, modifier: Modifier = Modifier, nested: Boolean = false) {
    val colors = CodegTheme.colors
    val autoExpand = vm.state == ToolCallState.RUNNING ||
        vm.state == ToolCallState.ERROR ||
        (vm.diffFiles != null && (vm.diffFiles.sumOf { f -> f.hunks.sumOf { it.rows.size } } <= 40))
    var expanded by remember(vm.id) { mutableStateOf(autoExpand) }
    val hasBody = vm.diffFiles != null || !vm.input.isNullOrBlank() || vm.hasOutput

    Column(
        modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(10.dp))
            .background(if (nested) Color.Transparent else colors.bgElevated.copy(alpha = 0.5f))
            .border(0.5.dp, colors.hairline, RoundedCornerShape(10.dp)),
    ) {
        Row(
            Modifier
                .fillMaxWidth()
                .clickable(enabled = hasBody) { expanded = !expanded }
                .padding(horizontal = 10.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            ToolStateIcon(vm)
            Text(
                text = vm.displayTitle,
                fontSize = 13.sp,
                fontWeight = FontWeight.Medium,
                color = colors.textPrimary,
                maxLines = 1,
                modifier = Modifier.weight(1f),
            )
            vm.diffFiles?.let { files ->
                val adds = files.sumOf { it.additions }
                val dels = files.sumOf { it.deletions }
                if (adds > 0) Text("+$adds", color = SuccessGreen, fontSize = 11.sp, fontWeight = FontWeight.SemiBold)
                if (dels > 0) Text("−$dels", color = colors.danger, fontSize = 11.sp, fontWeight = FontWeight.SemiBold)
            }
            if (hasBody) {
                Icon(
                    if (expanded) Icons.Rounded.ExpandLess else Icons.Rounded.ExpandMore,
                    contentDescription = null,
                    tint = colors.textTertiary,
                    modifier = Modifier.size(18.dp),
                )
            }
        }

        AnimatedVisibility(
            visible = expanded && hasBody,
            enter = expandVertically(tween(220)) + fadeIn(tween(220)),
            exit = shrinkVertically(tween(180)) + fadeOut(tween(120)),
        ) {
            Column(
                Modifier.fillMaxWidth().padding(start = 10.dp, end = 10.dp, bottom = 10.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                ToolBody(vm)
            }
        }
    }
}

@Composable
private fun ToolBody(vm: ToolCallVM) {
    val diff = vm.diffFiles
    if (diff != null) {
        DiffView(diff)
        return
    }
    // Input — a per-tool body for shell / todos, else a structured label→value field
    // list. The field list is the fix for delegate_to_agent and other MCP / generic
    // tools, which previously dumped their whole argument object as a raw JSON block.
    val input = vm.input
    if (!input.isNullOrBlank()) {
        when (vm.bucket) {
            ToolKindBucket.EXECUTE -> BashInputBody(input)
            ToolKindBucket.TODO -> TodoInputBody(input)
            else -> StructuredJsonView(input)
        }
    }
    // Output.
    if (vm.hasOutput) ToolOutputSection(vm)
}

/** Shell input: the command as a bash block. A JSON payload without a `command`
 *  field falls back to the structured field list; a bare command string (Codex
 *  `exec_command` persists the raw command, not JSON) shows as bash. */
@Composable
private fun BashInputBody(input: String) {
    val parsed = ToolDerive.parseJson(input)
    val args = ToolDerive.effectiveArgs(parsed)
    val cmd = args?.str("command") ?: args?.str("cmd") ?: args?.str("script")
    when {
        cmd != null -> CodeBlock(code = cmd, language = "bash")
        parsed == null -> CodeBlock(code = input, language = "bash")
        else -> StructuredJsonView(input)
    }
}

/** Todo-write input rendered as an evolving checklist (status icon + strikethrough
 *  on completed items). Falls back to the field list when there are no todos. */
@Composable
private fun TodoInputBody(input: String) {
    val colors = CodegTheme.colors
    val args = ToolDerive.effectiveArgs(ToolDerive.parseJson(input))
    val todos = (args?.get("todos") as? JsonArray) ?: (args?.get("todoList") as? JsonArray)
    if (todos.isNullOrEmpty()) {
        StructuredJsonView(input)
        return
    }
    Column(verticalArrangement = Arrangement.spacedBy(5.dp)) {
        todos.forEach { el ->
            val todo = el as? JsonObject ?: return@forEach
            val status = todo.str("status") ?: "pending"
            val text = todo.str("content") ?: todo.str("title") ?: return@forEach
            Row(horizontalArrangement = Arrangement.spacedBy(7.dp), verticalAlignment = Alignment.Top) {
                Icon(
                    when (status) {
                        "completed" -> Icons.Rounded.CheckCircle
                        "in_progress" -> Icons.Rounded.Adjust
                        else -> Icons.Rounded.RadioButtonUnchecked
                    },
                    contentDescription = null,
                    tint = when (status) {
                        "completed" -> SuccessGreen
                        "in_progress" -> colors.accent
                        else -> colors.textTertiary
                    },
                    modifier = Modifier.size(14.dp),
                )
                Text(
                    text,
                    fontSize = 12.sp,
                    color = if (status == "completed") colors.textTertiary else colors.textSecondary,
                    textDecoration = if (status == "completed") TextDecoration.LineThrough else null,
                )
            }
        }
    }
}

/** Tool output: a line-numbered Read file, a labeled error, a cleaned command
 *  console (JSON envelope unwrapped + CLI metadata stripped), or classified
 *  json / diff / markdown / log. */
@Composable
private fun ToolOutputSection(vm: ToolCallVM) {
    // Codex wraps every MCP result in `{result: <CallToolResult>, error: null}` on its
    // live wire; unwrap to the text the tool actually returned before classifying, or
    // the card renders the envelope's raw JSON.
    val out = ToolOutputFormat.mcpResultText(vm.trimmedOutput) ?: vm.trimmedOutput
    if (vm.bucket == ToolKindBucket.READ && !vm.isError) {
        val file = ReadOutputFormat.parse(out)
        if (file != null && file.content.isNotBlank()) {
            FileBodyView(file)
            return
        }
    }
    when {
        vm.isError -> ToolErrorOutput(out)
        vm.isCommand -> {
            val clean = ToolOutputFormat.cleanCommandOutput(out)
            if (clean.isNotEmpty()) CodeBlock(code = clean, language = "console")
        }
        else -> when (ToolOutputFormat.classify(out)) {
            ToolOutputFormat.Kind.JSON -> CodeBlock(code = prettyPrintJsonString(out) ?: out, language = "json")
            ToolOutputFormat.Kind.DIFF ->
                if (UnifiedDiff.looksLikeDiff(out)) {
                    UnifiedDiff.parse(out)?.let { DiffView(it) } ?: CodeBlock(code = out, language = "diff")
                } else {
                    CodeBlock(code = out, language = "diff")
                }
            ToolOutputFormat.Kind.MARKDOWN -> MarkdownContent(out)
            ToolOutputFormat.Kind.LOG -> CodeBlock(code = out, language = "output")
        }
    }
}

@Composable
private fun ToolStateIcon(vm: ToolCallVM) {
    val colors = CodegTheme.colors
    when (vm.state) {
        ToolCallState.RUNNING, ToolCallState.INPUT_STREAMING ->
            CircularProgressIndicator(modifier = Modifier.size(14.dp), strokeWidth = 2.dp, color = colors.accent)
        ToolCallState.DONE -> Icon(bucketIcon(vm.bucket), null, tint = colors.textSecondary, modifier = Modifier.size(15.dp))
        ToolCallState.ERROR -> Icon(Icons.Rounded.ErrorOutline, null, tint = colors.danger, modifier = Modifier.size(15.dp))
    }
}

/** A summary card for 2+ consecutive tool calls. Port of iOS `ToolGroupCard`. */
@Composable
fun ToolGroupCard(items: List<ToolCallVM>, modifier: Modifier = Modifier, streaming: Boolean = false) {
    val colors = CodegTheme.colors
    // Key on the stable first-tool id, not the items' value: live grouped tools rebuild
    // their VMs every flush, and remember(items) would reset the expand state each time.
    var expanded by remember(items.firstOrNull()?.id) { mutableStateOf(false) }
    val errorCount = items.count { it.state == ToolCallState.ERROR }

    Column(
        modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(10.dp))
            .background(colors.bgElevated.copy(alpha = 0.5f))
            .border(0.5.dp, colors.hairline, RoundedCornerShape(10.dp)),
    ) {
        Row(
            Modifier
                .fillMaxWidth()
                .clickable { expanded = !expanded }
                .padding(horizontal = 10.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Icon(Icons.Rounded.Layers, null, tint = colors.accent, modifier = Modifier.size(15.dp))
            Text(
                text = groupSummary(LocalContext.current, items),
                fontSize = 13.sp,
                fontWeight = FontWeight.SemiBold,
                color = colors.textPrimary,
                maxLines = 1,
                modifier = Modifier.weight(1f),
            )
            if (streaming) {
                LivePulse(dotSize = 7.dp)
            } else if (errorCount > 0) {
                Text(stringResource(R.string.toolcall_failed_count, errorCount), color = colors.danger, fontSize = 11.sp, fontWeight = FontWeight.SemiBold)
            }
            Icon(
                if (expanded) Icons.Rounded.ExpandLess else Icons.Rounded.ExpandMore,
                contentDescription = null,
                tint = colors.textTertiary,
                modifier = Modifier.size(18.dp),
            )
        }
        AnimatedVisibility(
            visible = expanded,
            enter = expandVertically(tween(220)) + fadeIn(tween(220)),
            exit = shrinkVertically(tween(180)) + fadeOut(tween(120)),
        ) {
            Column(
                Modifier.fillMaxWidth().padding(start = 8.dp, end = 8.dp, bottom = 8.dp),
                verticalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                for (item in items) ToolCallCard(item, nested = true)
            }
        }
    }
}

/** Bucket → glyph (shared by the tool card and the timeline marker). */
fun bucketIcon(bucket: ToolKindBucket): ImageVector = when (bucket) {
    ToolKindBucket.READ -> Icons.Rounded.Description
    ToolKindBucket.EDIT -> Icons.Rounded.Edit
    ToolKindBucket.SEARCH -> Icons.Rounded.Search
    ToolKindBucket.EXECUTE -> Icons.Rounded.Terminal
    ToolKindBucket.WEB -> Icons.Rounded.Language
    ToolKindBucket.TODO -> Icons.Rounded.Checklist
    ToolKindBucket.TASK -> Icons.Rounded.Groups
    ToolKindBucket.OTHER -> Icons.Rounded.Build
}

/** "Read 3 files · Edited 2 files" — counts per bucket, fixed iOS order. Localized via plurals. */
private fun groupSummary(context: Context, items: List<ToolCallVM>): String {
    val counts = items.groupingBy { it.bucket }.eachCount()
    val order = listOf(
        ToolKindBucket.READ, ToolKindBucket.EDIT, ToolKindBucket.SEARCH, ToolKindBucket.EXECUTE,
        ToolKindBucket.WEB, ToolKindBucket.TODO, ToolKindBucket.TASK, ToolKindBucket.OTHER,
    )
    val res = context.resources
    val phrases = order.mapNotNull { b ->
        val c = counts[b] ?: return@mapNotNull null
        if (c == 0) return@mapNotNull null
        when (b) {
            ToolKindBucket.READ -> res.getQuantityString(R.plurals.tool_summary_read, c, c)
            ToolKindBucket.EDIT -> res.getQuantityString(R.plurals.tool_summary_edit, c, c)
            ToolKindBucket.SEARCH -> res.getQuantityString(R.plurals.tool_summary_search, c, c)
            ToolKindBucket.EXECUTE -> res.getQuantityString(R.plurals.tool_summary_execute, c, c)
            ToolKindBucket.WEB -> res.getQuantityString(R.plurals.tool_summary_web, c, c)
            ToolKindBucket.TODO -> context.getString(R.string.tool_summary_todos)
            ToolKindBucket.TASK -> res.getQuantityString(R.plurals.tool_summary_task, c, c)
            ToolKindBucket.OTHER -> res.getQuantityString(R.plurals.tool_summary_tools, c, c)
        }
    }
    return if (phrases.isEmpty()) res.getQuantityString(R.plurals.tool_summary_tools, items.size, items.size)
    else phrases.joinToString(" · ")
}

private fun JsonObject.str(key: String): String? {
    val prim = this[key] as? JsonPrimitive ?: return null
    return if (prim.isString) prim.content else null
}
