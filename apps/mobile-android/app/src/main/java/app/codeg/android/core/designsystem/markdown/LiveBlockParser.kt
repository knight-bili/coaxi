package app.codeg.android.core.designsystem.markdown

/**
 * Incremental Markdown block parser for the **in-flight** assistant text. The live
 * text only grows by appended deltas (~50ms flush), so re-parsing the whole segment
 * every flush is O(n²) over a long streamed reply — the confirmed root cause. This
 * keeps the already-finalized prefix blocks (everything before the last blank-line
 * boundary that sits outside a code fence) and re-parses only the growing tail, so the
 * stable body of a message is parsed exactly once.
 *
 * `parse(prefix) + parse(tail) == parse(whole)` holds because, outside a fence, a blank
 * line is a pure block separator in [parseMarkdownBlocks]; the split point is only taken
 * where no fence is open. Fence tracking mirrors [parseMarkdownBlocks] exactly: a fence
 * opens on a line whose trimmed text starts with ``` or ~~~, and closes only on a later
 * line made entirely of the SAME marker char — so a ``` body containing a ~~~ line (or
 * vice-versa) is never mistaken for a fence toggle.
 *
 * **Scope / known limit:** reuse is at blank-line granularity. A *single* block with no
 * internal blank line (one huge fenced code block, bullet list, table, or paragraph)
 * still re-parses its own growing tail each flush until it is terminated — the tail *is*
 * that block. This is inherent to the "stable prefix + growing tail" model; the residual
 * cost is a cheap line scan (not the expensive inline-span build / recomposition, which
 * viewport virtualization already makes tail-only), and the most common large block —
 * an unterminated code fence — renders collapsed (20 lines). Sub-block incremental
 * parsing would be a separate, larger change.
 *
 * Keyed by the segment's node id — a live turn usually has one growing text segment,
 * occasionally a few when tools interleave. Entries are bounded (LRU) and a
 * shrink/divergence (a new turn, or reflowed text) resets that key. Single-threaded in
 * practice (the composition thread builds live nodes), guarded anyway.
 */
object LiveBlockParser {
    private const val MAX_SEGMENTS = 16

    /** Test-only: total characters handed to [parseMarkdownBlocks], to assert the stable
     *  prefix is not re-parsed each flush. Not used in production paths. */
    @Volatile
    internal var parsedChars: Long = 0L
        private set

    private class State {
        var text: String = ""
        var stableLen: Int = 0            // text[0, stableLen) is finalized into stableBlocks (always outside a fence)
        var stableBlocks: List<MarkdownBlock> = emptyList()
    }

    private val states = object : LinkedHashMap<String, State>(16, 0.75f, /* accessOrder = */ true) {
        override fun removeEldestEntry(eldest: Map.Entry<String, State>): Boolean = size > MAX_SEGMENTS
    }

    @Synchronized
    fun parse(key: String, newText: String): List<MarkdownBlock> {
        val s = states.getOrPut(key) { State() }
        // Reuse the finalized prefix only if newText extends what we already stabilized.
        if (newText.length < s.stableLen || !newText.regionMatches(0, s.text, 0, s.stableLen)) {
            s.stableLen = 0; s.stableBlocks = emptyList()
        }
        s.text = newText
        advance(s)
        val tail = if (s.stableLen >= newText.length) "" else newText.substring(s.stableLen)
        val tailBlocks = if (tail.isBlank()) emptyList() else parseBlocks(tail)
        return when {
            s.stableBlocks.isEmpty() -> tailBlocks
            tailBlocks.isEmpty() -> s.stableBlocks
            else -> s.stableBlocks + tailBlocks
        }
    }

    /** Clear a segment's incremental state (e.g. when a turn finalizes). Optional — the LRU self-evicts. */
    @Synchronized
    fun forget(key: String) { states.remove(key) }

    /**
     * Advance the finalized prefix to the last blank-line boundary outside a code fence,
     * parsing only the newly-stabilized chunk. Scans just the unstable region
     * `[stableLen, len)` (which always starts outside a fence), so the stable prefix is
     * never re-scanned.
     */
    private fun advance(s: State) {
        val t = s.text
        val n = t.length
        var open: String? = null   // fence marker currently open ("```"/"~~~"), or null
        var boundary = s.stableLen
        var prevBlank = false       // the unstable region always starts at a non-blank line
        var i = s.stableLen
        while (i < n) {
            val nl = t.indexOf('\n', i)
            val lineEnd = if (nl == -1) n else nl
            val trimmed = t.substring(i, lineEnd).trim()
            // A safe split point: a line start preceded by a blank line, outside any fence.
            if (prevBlank && open == null && i > s.stableLen) boundary = i
            val cur = open
            if (cur == null) {
                open = when {
                    trimmed.startsWith("```") -> "```"
                    trimmed.startsWith("~~~") -> "~~~"
                    else -> null
                }
            } else if (trimmed.startsWith(cur) && trimmed.all { it == cur[0] }) {
                open = null // closes only on a same-marker, all-fence-char line
            }
            prevBlank = trimmed.isEmpty()
            i = if (nl == -1) n else nl + 1
        }
        if (boundary > s.stableLen) {
            val chunk = parseBlocks(t.substring(s.stableLen, boundary))
            s.stableBlocks = if (s.stableBlocks.isEmpty()) chunk else s.stableBlocks + chunk
            s.stableLen = boundary
        }
    }

    private fun parseBlocks(text: String): List<MarkdownBlock> {
        parsedChars += text.length
        return parseMarkdownBlocks(text)
    }
}
