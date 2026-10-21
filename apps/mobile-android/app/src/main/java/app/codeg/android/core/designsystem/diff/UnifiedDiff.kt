package app.codeg.android.core.designsystem.diff

/** One rendered line of a diff. */
data class DiffRow(
    val kind: Kind,
    val oldLine: Int?,
    val newLine: Int?,
    val text: String,
) {
    enum class Kind { CONTEXT, ADDED, DELETED }
}

/** A contiguous block of changes within a file (a `@@ … @@` hunk). */
data class DiffHunk(val header: String?, val rows: List<DiffRow>)

/** A single file's worth of changes inside a unified diff. */
data class DiffFile(
    val path: String,
    val oldPath: String?,
    val mode: Mode,
    val additions: Int,
    val deletions: Int,
    val hunks: List<DiffHunk>,
) {
    enum class Mode { ADDED, MODIFIED, DELETED, RENAMED }

    val isNewFile: Boolean get() = mode == Mode.ADDED
}

/**
 * A dependency-free unified-diff parser. Accepts the three shapes the codeg
 * backend can hand us: git unified diffs, codex `apply_patch` blocks, and the
 * minimal `--- / +++ / -old / +new` form the live ACP stream emits. Returns null
 * when the text isn't a diff. Faithful port of the iOS `UnifiedDiff`.
 */
object UnifiedDiff {

    private val applyPatchHeader = Regex("""(?m)^\*\*\* (Add|Update|Delete|Move) """)
    private val hunkAtStart = Regex("""(?m)^@@+ -\d""")
    private val minusHeader = Regex("""(?m)^--- """)
    private val plusHeader = Regex("""(?m)^\+\+\+ """)
    private val hunkRegex = Regex("""^@@+ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@""")

    fun parse(raw: String): List<DiffFile>? {
        val text = raw.replace("\r\n", "\n")
        val isApplyPatch = text.contains("*** Begin Patch") || applyPatchHeader.containsMatchIn(text)
        val files = if (isApplyPatch) parseApplyPatch(text) else parseGitUnified(text)
        val nonEmpty = files.filter { file -> file.hunks.any { it.rows.isNotEmpty() } }
        return nonEmpty.ifEmpty { null }
    }

    /** Cheap pre-check so callers only run the full parse when text plausibly is a diff. */
    fun looksLikeDiff(raw: String): Boolean {
        if (raw.contains("diff --git") || raw.contains("*** Begin Patch")) return true
        if (hunkAtStart.containsMatchIn(raw)) return true
        if (applyPatchHeader.containsMatchIn(raw)) return true
        if (minusHeader.containsMatchIn(raw) && plusHeader.containsMatchIn(raw)) return true
        return false
    }

    private fun parseGitUnified(text: String): List<DiffFile> {
        val files = mutableListOf<DiffFile>()

        var path: String? = null
        var oldPath: String? = null
        var mode = DiffFile.Mode.MODIFIED
        var hunks = mutableListOf<DiffHunk>()
        var adds = 0
        var dels = 0
        var rows = mutableListOf<DiffRow>()
        var header: String? = null
        var inHunk = false
        var oldNo = 0
        var newNo = 0
        var haveNumbers = false

        fun flushHunk() {
            if (inHunk || rows.isNotEmpty()) hunks.add(DiffHunk(header, rows))
            rows = mutableListOf(); header = null; inHunk = false
        }
        fun flushFile() {
            flushHunk()
            val p = path
            if (p != null && hunks.isNotEmpty()) {
                files.add(DiffFile(p, oldPath, mode, adds, dels, hunks))
            }
            path = null; oldPath = null; mode = DiffFile.Mode.MODIFIED
            hunks = mutableListOf(); adds = 0; dels = 0
            oldNo = 0; newNo = 0; haveNumbers = false
        }

        for (line in text.split("\n")) {
            when {
                line.startsWith("diff --git") -> {
                    flushFile()
                    val parts = line.removePrefix("diff --git").split(" ").filter { it.isNotEmpty() }
                    if (parts.size >= 2) {
                        oldPath = stripABPrefix(parts[parts.size - 2])
                        path = stripABPrefix(parts[parts.size - 1])
                    }
                }
                line.startsWith("new file mode") -> mode = DiffFile.Mode.ADDED
                line.startsWith("deleted file mode") -> mode = DiffFile.Mode.DELETED
                line.startsWith("rename from ") -> { oldPath = line.drop(12); mode = DiffFile.Mode.RENAMED }
                line.startsWith("rename to ") -> { path = line.drop(10); mode = DiffFile.Mode.RENAMED }
                line.startsWith("index ") || line.startsWith("similarity index") ||
                    line.startsWith("old mode") || line.startsWith("new mode") ||
                    line.startsWith("copy from") || line.startsWith("copy to") -> Unit
                line.startsWith("--- ") -> {
                    val p = line.drop(4).trim()
                    when {
                        p == "/dev/null" -> mode = DiffFile.Mode.ADDED
                        path == null -> path = stripABPrefix(p)
                        oldPath == null -> oldPath = stripABPrefix(p)
                    }
                }
                line.startsWith("+++ ") -> {
                    val p = line.drop(4).trim()
                    when {
                        p == "/dev/null" -> mode = DiffFile.Mode.DELETED
                        path == null -> path = stripABPrefix(p)
                    }
                }
                line.startsWith("@@") -> {
                    flushHunk()
                    header = line
                    inHunk = true
                    val parsed = parseHunkHeader(line)
                    if (parsed != null) { oldNo = parsed.first; newNo = parsed.second; haveNumbers = true } else haveNumbers = false
                }
                line.startsWith("\\") -> Unit // "\ No newline at end of file"
                line.startsWith("+") -> {
                    if (!inHunk) { inHunk = true; header = null }
                    rows.add(DiffRow(DiffRow.Kind.ADDED, null, if (haveNumbers) newNo else null, line.drop(1)))
                    if (haveNumbers) newNo += 1
                    adds += 1
                }
                line.startsWith("-") -> {
                    if (!inHunk) { inHunk = true; header = null }
                    rows.add(DiffRow(DiffRow.Kind.DELETED, if (haveNumbers) oldNo else null, null, line.drop(1)))
                    if (haveNumbers) oldNo += 1
                    dels += 1
                }
                line.startsWith(" ") -> {
                    if (inHunk) {
                        rows.add(DiffRow(DiffRow.Kind.CONTEXT, if (haveNumbers) oldNo else null, if (haveNumbers) newNo else null, line.drop(1)))
                        if (haveNumbers) { oldNo += 1; newNo += 1 }
                    }
                }
                line.isEmpty() && inHunk -> {
                    rows.add(DiffRow(DiffRow.Kind.CONTEXT, if (haveNumbers) oldNo else null, if (haveNumbers) newNo else null, ""))
                    if (haveNumbers) { oldNo += 1; newNo += 1 }
                }
                else -> Unit
            }
        }
        flushFile()
        return files
    }

    private fun parseApplyPatch(text: String): List<DiffFile> {
        val files = mutableListOf<DiffFile>()

        var path: String? = null
        var oldPath: String? = null
        var mode = DiffFile.Mode.MODIFIED
        var hunks = mutableListOf<DiffHunk>()
        var adds = 0
        var dels = 0
        var rows = mutableListOf<DiffRow>()
        var header: String? = null
        var inHunk = false

        fun flushHunk() {
            if (inHunk || rows.isNotEmpty()) hunks.add(DiffHunk(header, rows))
            rows = mutableListOf(); header = null; inHunk = false
        }
        fun flushFile() {
            flushHunk()
            val p = path
            if (p != null && hunks.isNotEmpty()) {
                files.add(DiffFile(p, oldPath, mode, adds, dels, hunks))
            }
            path = null; oldPath = null; mode = DiffFile.Mode.MODIFIED
            hunks = mutableListOf(); adds = 0; dels = 0
        }

        for (line in text.split("\n")) {
            when {
                line.startsWith("*** Begin Patch") || line.startsWith("*** End Patch") -> Unit
                line.startsWith("*** Add File: ") -> { flushFile(); path = line.drop(14).trim(); mode = DiffFile.Mode.ADDED }
                line.startsWith("*** Update File: ") -> { flushFile(); path = line.drop(17).trim(); mode = DiffFile.Mode.MODIFIED }
                line.startsWith("*** Delete File: ") -> { flushFile(); path = line.drop(17).trim(); mode = DiffFile.Mode.DELETED }
                line.startsWith("*** Move to: ") -> { oldPath = path; path = line.drop(13).trim(); mode = DiffFile.Mode.RENAMED }
                line.startsWith("@@") -> { flushHunk(); header = if (line == "@@") null else line; inHunk = true }
                line.startsWith("+") -> { inHunk = true; rows.add(DiffRow(DiffRow.Kind.ADDED, null, null, line.drop(1))); adds += 1 }
                line.startsWith("-") -> { inHunk = true; rows.add(DiffRow(DiffRow.Kind.DELETED, null, null, line.drop(1))); dels += 1 }
                line.startsWith(" ") -> { inHunk = true; rows.add(DiffRow(DiffRow.Kind.CONTEXT, null, null, line.drop(1))) }
                line.isEmpty() && inHunk -> rows.add(DiffRow(DiffRow.Kind.CONTEXT, null, null, ""))
                else -> Unit
            }
        }
        flushFile()
        return files
    }

    private fun parseHunkHeader(line: String): Pair<Int, Int>? {
        val m = hunkRegex.find(line) ?: return null
        val o = m.groupValues[1].toIntOrNull() ?: 1
        val n = m.groupValues[2].toIntOrNull() ?: 1
        return o to n
    }

    private fun stripABPrefix(p: String): String {
        var s = p.trim()
        if (s.startsWith("\"") && s.endsWith("\"") && s.length >= 2) s = s.substring(1, s.length - 1)
        if (s.startsWith("a/") || s.startsWith("b/")) s = s.drop(2)
        return s
    }
}
