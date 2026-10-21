package app.codeg.android.feature.sessions

import app.codeg.android.core.model.ConversationSummary
import app.codeg.android.core.model.FolderDetail

/**
 * One rendered group in the Chats list, fully sorted and filtered.
 *
 * Built off the main thread by [buildSessionSections] (exposed as
 * [SessionListViewModel.sections]) so the O(n·log n) grouping/sorting never runs
 * during composition — the `LazyColumn` just iterates these pre-sorted rows, which
 * is what keeps a very large session list scrolling smoothly. Mirrors the iOS
 * `SessionListView` sectioning (Pinned / per-folder / Other), realized on Android
 * as native collapsible sections rather than zoom-drill cards.
 */
data class SessionSection(
    /** Stable id used for the collapse set and the header's LazyColumn key. */
    val id: String,
    val kind: SectionKind,
    val rows: List<SessionRowItem>,
) {
    val count: Int get() = rows.size
}

/** What a [SessionSection] represents — drives its header icon, label, and tint. */
sealed interface SectionKind {
    /** Pinned conversations across all folders. */
    data object Pinned : SectionKind

    /** One folder's non-pinned conversations. [colorHex] is the folder's server color. */
    data class Folder(val name: String, val colorHex: String) : SectionKind

    /** Non-pinned conversations whose folder isn't in the folder list (orphans). */
    data object Other : SectionKind
}

/** One conversation row, with its cross-folder tag pre-resolved. */
data class SessionRowItem(
    val conversation: ConversationSummary,
    /** Dim folder tag shown on cross-folder rows (Pinned/Other); `null` inside a
     * folder group, where the folder is already the header. */
    val folderName: String?,
)

/**
 * Compose [SessionGrouping]'s pure accessors into the ordered section list shown by
 * the Chats screen: Pinned (if any) → one section per folder in
 * [SessionGrouping.sortedFolders] order (empty folders kept, matching iOS so their
 * header still shows) → Other (if any).
 *
 * `folderName` is resolved here, once, for the cross-folder sections only, so each
 * row needs no map lookup at compose time. Pure and allocation-light; the view model
 * runs it on `Dispatchers.Default`.
 */
fun buildSessionSections(
    folders: List<FolderDetail>,
    conversations: List<ConversationSummary>,
): List<SessionSection> {
    val folderNames = SessionGrouping.folderNames(folders)
    val out = ArrayList<SessionSection>()

    val pinned = SessionGrouping.pinned(conversations)
    if (pinned.isNotEmpty()) {
        out += SessionSection(
            id = "pinned",
            kind = SectionKind.Pinned,
            rows = pinned.map { SessionRowItem(it, folderNames[it.folderId]) },
        )
    }

    for (group in SessionGrouping.folderGroups(folders, conversations)) {
        out += SessionSection(
            id = "folder-${group.folder.id}",
            kind = SectionKind.Folder(group.folder.name, group.folder.color),
            rows = group.conversations.map { SessionRowItem(it, folderName = null) },
        )
    }

    val other = SessionGrouping.ungrouped(folders, conversations)
    if (other.isNotEmpty()) {
        out += SessionSection(
            id = "other",
            kind = SectionKind.Other,
            rows = other.map { SessionRowItem(it, folderNames[it.folderId]) },
        )
    }

    return out
}
