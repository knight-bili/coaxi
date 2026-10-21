package app.codeg.android.core.model

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/** A directory in the server filesystem browser (`list_directory_entries`, dirs only). */
@Serializable
data class DirectoryEntry(
    val name: String,
    val path: String,
    val hasChildren: Boolean = false,
)

/**
 * A file or directory in the file browser (`list_directory_with_files`). NOTE:
 * this endpoint returns **camelCase** wire keys (`isDir`/`hasChildren`), so it is
 * decoded with the camelCase codec, not the snake_case response codec.
 */
@Serializable
data class DirectoryItem(
    val name: String,
    val path: String,
    val isDir: Boolean = false,
    val hasChildren: Boolean = false,
    val size: Long? = null,
)

/** Read-only file preview (`read_file_preview`). */
@Serializable
data class FilePreviewContent(
    val path: String,
    val content: String,
)

/** One working-tree change (`git_status`). */
@Serializable
data class GitStatusEntry(
    val status: String,
    val file: String,
) {
    /** Current path — right side of `old -> new` for renames, else [file]. */
    val path: String
        get() = if (file.contains(" -> ")) file.substringAfter(" -> ").trim() else file

    /** Prior path for renames, else null. */
    val renamedFrom: String?
        get() = if (file.contains(" -> ")) file.substringBefore(" -> ").trim() else null

    val change: GitChange get() = GitChange.from(status)
}

/** Category of a working-tree change, with its badge letter. Port of iOS `GitChange`. */
enum class GitChange(val badge: String) {
    UNTRACKED("U"), CONFLICTED("!"), RENAMED("R"), COPIED("C"),
    ADDED("A"), DELETED("D"), TYPE_CHANGED("T"), MODIFIED("M"), OTHER("•");

    companion object {
        fun from(status: String): GitChange {
            val s = status.trim()
            if (s == "??") return UNTRACKED
            if (s.contains("U") || s == "AA" || s == "DD") return CONFLICTED
            if (s.contains("R")) return RENAMED
            if (s.contains("C")) return COPIED
            if (s.contains("A")) return ADDED
            if (s.contains("D")) return DELETED
            if (s.contains("T")) return TYPE_CHANGED
            if (s.contains("M")) return MODIFIED
            return OTHER
        }
    }
}

/** Result of `git_log`. */
@Serializable
data class GitLogResult(
    val entries: List<GitLogEntry> = emptyList(),
    val hasUpstream: Boolean = false,
)

/** One commit (`git_log`). */
@Serializable
data class GitLogEntry(
    val hash: String,
    val fullHash: String,
    val author: String = "",
    val date: String = "",
    val message: String = "",
    val files: List<GitLogFileChange> = emptyList(),
    val pushed: Boolean? = null,
) {
    val subject: String get() = message.lineSequence().firstOrNull { it.isNotBlank() }?.trim() ?: ""
    val body: String
        get() = message.substringAfter('\n', "").trim()
    val totalAdditions: Int get() = files.sumOf { it.additions }
    val totalDeletions: Int get() = files.sumOf { it.deletions }
}

/** Per-file change summary inside a commit. */
@Serializable
data class GitLogFileChange(
    val path: String,
    val status: String = "",
    val additions: Int = 0,
    val deletions: Int = 0,
)

/** Branches in a repo (`git_list_all_branches`). */
@Serializable
data class GitBranchList(
    val local: List<String> = emptyList(),
    val remote: List<String> = emptyList(),
    @SerialName("worktree_branches")
    val worktreeBranches: List<String> = emptyList(),
)

/** Result of `git_commit` — how many files were committed. */
@Serializable
data class GitCommitResult(
    val committedFiles: Int = 0,
)

/**
 * Result of `git_push`. [upstreamSet] is true when the push also established the
 * branch's upstream (the first push of a new branch).
 */
@Serializable
data class GitPushResult(
    val pushedCommits: Int = 0,
    val upstreamSet: Boolean = false,
)

/**
 * Result of `git_pull` (fetch + merge). [conflict] is present when the merge
 * produced conflicts the user must resolve elsewhere (desktop / an agent).
 */
@Serializable
data class GitPullResult(
    val updatedFiles: Int = 0,
    val conflict: GitConflictInfo? = null,
)

/**
 * Conflict detail attached to a pull/merge result. [operation] is the in-progress
 * git operation (e.g. "merge"); [conflictedFiles] are repo-relative paths.
 */
@Serializable
data class GitConflictInfo(
    val hasConflicts: Boolean = false,
    val conflictedFiles: List<String> = emptyList(),
    val operation: String = "",
    val upstreamCommit: String? = null,
)

/**
 * Branch + remotes + tracking remote for a repo (`git_push_info`). Drives the
 * Commits tab's "branch → remote" sync header and its no-remote disabled state.
 */
@Serializable
data class GitPushInfo(
    val branch: String = "",
    val remotes: List<GitRemote> = emptyList(),
    val trackingRemote: String? = null,
) {
    /** Remotes deduped by name (git lists fetch+push rows separately), order kept. */
    val uniqueRemotes: List<GitRemote>
        get() {
            val seen = mutableSetOf<String>()
            return remotes.filter { seen.add(it.name) }
        }
}

/** A configured git remote (`git_list_remotes` / `git_push_info`). */
@Serializable
data class GitRemote(
    val name: String,
    val url: String,
)

/**
 * Where a branch is currently checked out (`resolve_worktree_folder`). Drives the
 * worktree-aware branch switch:
 * - [path] null → the branch isn't checked out in any worktree (check it out in
 *   the repo root);
 * - [path] set, [folderId] null → it lives in a worktree dir not yet registered
 *   as a folder (register it via `open_worktree_folder`);
 * - [path] set, [folderId] set → it lives in an already-registered folder
 *   (navigate there). Mirrors iOS `WorktreeResolution`.
 */
@Serializable
data class WorktreeResolution(
    val path: String? = null,
    @SerialName("folder_id")
    val folderId: Int? = null,
)
