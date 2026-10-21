package app.codeg.android.feature.sessiondetail.rendering

import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive

/**
 * Context-compaction tool-call detection (port of the web `lib/context-compaction.ts`).
 *
 * A compaction lifecycle arrives as an ACP `tool_call` tagged with
 * `_meta.contextCompaction == true` — codex-acp emits it natively (1.1.3+), and
 * codeg's Grok bridge synthesizes the same shape for `auto_compact_completed`, both
 * on the live stream and when re-reading history. It is addressed by that meta flag,
 * NOT by tool name, so it works for every host that adopts the tag.
 *
 * It renders as a chrome-less centred divider ("context was compacted here"), not as
 * a tool card, and must never fold into a tool group.
 */
object ContextCompaction {

    /** Whether this tool call's `meta` marks it as a compaction. A JSON string
     *  `"true"` does NOT count — the web tests `=== true`. */
    fun matches(meta: JsonObject?): Boolean {
        val flag = meta?.get("contextCompaction") as? JsonPrimitive ?: return false
        return !flag.isString && flag.content == "true"
    }

    /**
     * The token counts Grok stamps on its compaction card (`tokensBefore` /
     * `tokensAfter`). codex sends none, so both are frequently null — the divider
     * falls back to a plain label then.
     */
    fun tokens(meta: JsonObject?): Pair<Int?, Int?> = count(meta, "tokensBefore") to count(meta, "tokensAfter")

    /** A non-negative, finite integer field off the opaque meta pass-through. */
    private fun count(meta: JsonObject?, key: String): Int? {
        val value = (meta?.get(key) as? JsonPrimitive)?.takeIf { !it.isString }?.content?.toDoubleOrNull()
        return value?.takeIf { it.isFinite() && it >= 0 }?.toInt()
    }
}
