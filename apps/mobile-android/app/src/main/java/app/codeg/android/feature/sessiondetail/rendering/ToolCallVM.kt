package app.codeg.android.feature.sessiondetail.rendering

import app.codeg.android.core.designsystem.diff.DiffFile
import app.codeg.android.core.model.ContentBlock
import app.codeg.android.core.model.ImageData
import app.codeg.android.core.model.MessageTurn
import kotlinx.serialization.json.JsonObject

/** The visual lifecycle of a tool call. */
enum class ToolCallState { INPUT_STREAMING, RUNNING, DONE, ERROR }

/**
 * A unified, view-ready model for one tool call. Both the persisted path
 * (`tool_use` + `tool_result` paired) and the live path adapt into this, so the
 * card UI is shared. Derived fields are computed once via [of]. Port of iOS
 * `ToolCallVM`.
 */
data class ToolCallVM(
    val id: String,
    val rawName: String,
    val kind: String,
    val state: ToolCallState,
    val input: String?,
    val output: String?,
    val content: String?,
    val isError: Boolean,
    val displayTitle: String,
    val bucket: ToolKindBucket,
    val diffFiles: List<DiffFile>?,
    /** The agent's opaque ACP metadata for this call (Grok's `x.ai/tool` identity,
     *  the context-compaction tag, …). Null for most tools and hosts. */
    val meta: JsonObject? = null,
) {
    val trimmedOutput: String get() = (output ?: "").trim()
    val hasOutput: Boolean get() = trimmedOutput.isNotEmpty()
    val isCommand: Boolean get() = bucket == ToolKindBucket.EXECUTE

    /**
     * A plan-*mode* transition (Claude's `EnterPlanMode`/`ExitPlanMode`, Grok's
     * `enter_plan_mode`/`exit_plan_mode`, Cline's `switch_mode`). A mode signal, not a
     * work tool, so it renders standalone instead of folding into a group.
     */
    val isPlanMode: Boolean get() = ToolDerive.isPlanModeName(rawName)

    companion object {
        fun of(
            id: String,
            rawName: String,
            kind: String,
            state: ToolCallState,
            input: String?,
            output: String?,
            content: String?,
            isError: Boolean,
            meta: JsonObject? = null,
        ): ToolCallVM {
            // Grok's plan-mode tools carry their authoritative identity in
            // `meta["x.ai/tool"].kind`, while the `title` we're handed as [rawName]
            // MUTATES across the lifecycle (`enter_plan_mode` -> "Plan: Enter" -> "Plan
            // mode entered"). Resolve them to the canonical name up front so the live
            // stream lands on the same identity the persisted path reads — and so the
            // card's title stops changing mid-stream.
            val resolvedName = ToolDerive.grokPlanModeName(meta) ?: rawName
            val parsed = ToolDerive.parseJson(input)
            val bucket = ToolDerive.bucket(resolvedName, kind)
            return ToolCallVM(
                id = id,
                rawName = resolvedName,
                kind = kind,
                state = state,
                input = input,
                output = output,
                content = content,
                isError = isError,
                displayTitle = ToolDerive.title(resolvedName, parsed),
                bucket = bucket,
                diffFiles = ToolDerive.diff(bucket, input, output, content, parsed),
                meta = meta,
            )
        }
    }
}

/** One ordered piece of a rendered assistant turn (static subset; live variants added in M4). */
sealed interface RenderPart {
    data class Text(val text: String) : RenderPart
    data class Reasoning(val text: String) : RenderPart
    data class Tool(val vm: ToolCallVM) : RenderPart
    data class ToolGroup(val items: List<ToolCallVM>, val streaming: Boolean) : RenderPart
    data class Image(val image: ImageData, val caption: String?) : RenderPart
    /**
     * A context compaction (`meta.contextCompaction`) — a conversation boundary
     * marker, not a tool call. Rendered as a chrome-less centred divider; the token
     * counts are Grok-only (codex sends none).
     */
    data class Compaction(val before: Int?, val after: Int?, val running: Boolean) : RenderPart
    data class Unknown(val type: String) : RenderPart
}

/** Adapts persisted [MessageTurn]s into `[RenderPart]`. Port of iOS `MessageRender`. */
object MessageRender {

    fun adaptTurn(turn: MessageTurn): List<RenderPart> {
        val blocks = turn.blocks
        val consumed = mutableSetOf<Int>()
        val parts = mutableListOf<RenderPart>()

        blocks.forEachIndexed { idx, block ->
            if (idx in consumed) return@forEachIndexed
            when (block) {
                is ContentBlock.Text ->
                    if (block.text.isNotBlank()) parts.add(RenderPart.Text(block.text))
                is ContentBlock.Thinking ->
                    if (block.text.isNotBlank()) parts.add(RenderPart.Reasoning(block.text))
                is ContentBlock.Image ->
                    parts.add(RenderPart.Image(block.image, null))
                is ContentBlock.ImageGeneration -> {
                    if (block.image != null) parts.add(RenderPart.Image(block.image, block.revisedPrompt))
                    else if (!block.revisedPrompt.isNullOrEmpty()) parts.add(RenderPart.Text(block.revisedPrompt))
                }
                is ContentBlock.ToolUse -> {
                    val resultIdx = findResult(blocks, idx, block.id, consumed)
                    var output: String? = null
                    var isErr = false
                    if (resultIdx != null) {
                        (blocks[resultIdx] as? ContentBlock.ToolResult)?.let {
                            output = it.outputPreview; isErr = it.isError; consumed.add(resultIdx)
                        }
                    }
                    val state = when {
                        resultIdx == null -> ToolCallState.RUNNING
                        isErr -> ToolCallState.ERROR
                        else -> ToolCallState.DONE
                    }
                    // A compaction is a boundary marker, not a call: it renders as a
                    // divider between turns, so it never becomes a ToolCallVM (and so
                    // can't be swept into a tool group).
                    if (ContextCompaction.matches(block.meta)) {
                        val (before, after) = ContextCompaction.tokens(block.meta)
                        parts.add(RenderPart.Compaction(before, after, state == ToolCallState.RUNNING))
                        return@forEachIndexed
                    }
                    parts.add(
                        RenderPart.Tool(
                            // Qualify the synthetic fallback with the turn id: a bare positional
                            // "tool-$idx" would collide across replies once it becomes a node key.
                            ToolCallVM.of(
                                block.id ?: "${turn.id}-tool-$idx", block.name, "", state,
                                block.inputPreview, output, null, isErr, block.meta,
                            ),
                        ),
                    )
                }
                is ContentBlock.ToolResult -> {
                    parts.add(
                        RenderPart.Tool(
                            ToolCallVM.of(
                                block.id ?: "${turn.id}-result-$idx", "result", "",
                                if (block.isError) ToolCallState.ERROR else ToolCallState.DONE,
                                null, block.outputPreview, null, block.isError,
                            ),
                        ),
                    )
                }
                is ContentBlock.Unknown -> parts.add(RenderPart.Unknown(block.type))
            }
        }
        return groupConsecutiveTools(parts)
    }

    /** Collapse runs of 2+ consecutive tool calls into a [RenderPart.ToolGroup]. */
    fun groupConsecutiveTools(parts: List<RenderPart>): List<RenderPart> {
        val out = mutableListOf<RenderPart>()
        var run = mutableListOf<ToolCallVM>()

        fun flush() {
            if (run.size >= 2) {
                val streaming = run.any { it.state == ToolCallState.RUNNING || it.state == ToolCallState.INPUT_STREAMING }
                out.add(RenderPart.ToolGroup(run.toList(), streaming))
            } else {
                run.firstOrNull()?.let { out.add(RenderPart.Tool(it)) }
            }
            run = mutableListOf()
        }

        for (part in parts) {
            // Agent-dispatch (`TASK`) and plan-mode transitions each render on their
            // own, so they never fold into a generic tool run.
            if (part is RenderPart.Tool && part.vm.bucket != ToolKindBucket.TASK && !part.vm.isPlanMode) {
                run.add(part.vm)
            } else {
                flush()
                out.add(part)
            }
        }
        flush()
        return out
    }

    private fun findResult(blocks: List<ContentBlock>, after: Int, toolID: String?, consumed: Set<Int>): Int? {
        if (toolID != null) {
            for (j in (after + 1) until blocks.size) {
                if (j in consumed) continue
                val b = blocks[j]
                if (b is ContentBlock.ToolResult && b.id == toolID) return j
            }
        }
        for (j in (after + 1) until blocks.size) {
            if (j in consumed) continue
            val b = blocks[j]
            if (b is ContentBlock.ToolUse) break
            if (b is ContentBlock.ToolResult) return j
        }
        return null
    }
}
