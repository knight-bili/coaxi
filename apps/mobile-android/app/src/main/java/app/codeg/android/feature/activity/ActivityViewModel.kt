package app.codeg.android.feature.activity

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.codeg.android.core.data.ServerRepository
import app.codeg.android.core.datastore.ServerProfile
import app.codeg.android.core.model.ConversationSummary
import app.codeg.android.core.network.CodegClient
import app.codeg.android.core.network.displayMessage
import app.codeg.android.feature.sessions.SessionRowItem
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.flowOn
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import java.time.Duration
import java.time.Instant
import javax.inject.Inject

/**
 * The Activity tab: a 25-second poll loop deriving the "Running" set (live
 * conversations) and "Last 24 Hours" set. Port of the iOS `ActivityModel`
 * (scoped to this screen rather than app-wide; resets when the server changes).
 */
@HiltViewModel
class ActivityViewModel @Inject constructor(
    private val repository: ServerRepository,
) : ViewModel() {

    private val _ui = MutableStateFlow(ActivityUiState())
    val ui: StateFlow<ActivityUiState> = _ui.asStateFlow()

    /**
     * The Running / Last-24h sections the feed renders, sorted and with the folder tag
     * pre-resolved — computed on [Dispatchers.Default] and recomputed only when the
     * conversations/folder names change (not on the 25s poll's spinner toggle). Keeps
     * the filter+sort off the main thread and out of composition, like the Chats list.
     */
    val sections: StateFlow<List<ActivitySection>> =
        _ui
            .map { ActivityInput(it.conversations, it.folderNames) }
            .distinctUntilChanged()
            .map { buildActivitySections(it.conversations, it.folderNames) }
            .flowOn(Dispatchers.Default)
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    private data class ActivityInput(
        val conversations: List<ConversationSummary>,
        val folderNames: Map<Int, String>,
    )

    private var client: CodegClient? = null
    private var pollJob: Job? = null

    init {
        viewModelScope.launch {
            repository.selectedProfile.collectLatest { profile -> onProfile(profile) }
        }
    }

    private suspend fun onProfile(profile: ServerProfile?) {
        pollJob?.cancel()
        if (profile == null) {
            client = null
            _ui.value = ActivityUiState()
            return
        }
        client = repository.client(profile)
        _ui.value = ActivityUiState(loading = true)
        pollJob = viewModelScope.launch {
            while (true) {
                fetch()
                delay(25_000)
            }
        }
    }

    fun refresh() {
        if (_ui.value.refreshing) return
        viewModelScope.launch { fetch(manual = true) }
    }

    private suspend fun fetch(manual: Boolean = false) {
        val c = client ?: return
        if (manual) _ui.update { it.copy(refreshing = true) }
        try {
            val conversations = c.listConversations(sortBy = "updated")
            if (_ui.value.folderNames.isEmpty()) {
                val folders = runCatching { c.listFolders() }.getOrDefault(emptyList())
                _ui.update { it.copy(folderNames = folders.associate { f -> f.id to f.name }) }
            }
            _ui.update { it.copy(conversations = conversations, loading = false, refreshing = false, hasLoaded = true, error = null, lastRefreshed = Instant.now()) }
        } catch (e: CancellationException) {
            throw e
        } catch (e: Exception) {
            _ui.update { it.copy(loading = false, refreshing = false, error = if (it.hasLoaded) it.error else e.displayMessage()) }
        }
    }
}

data class ActivityUiState(
    val conversations: List<ConversationSummary> = emptyList(),
    val folderNames: Map<Int, String> = emptyMap(),
    val loading: Boolean = false,
    val refreshing: Boolean = false,
    val hasLoaded: Boolean = false,
    val error: String? = null,
    val lastRefreshed: Instant? = null,
) {
    /**
     * Whether the feed has nothing to show — no live session and nothing updated in the
     * last 24h. A single unsorted pass (no allocation), read once for the empty-state
     * decision; the displayed, sorted sections come from [ActivityViewModel.sections].
     */
    val isEmptyFeed: Boolean
        get() {
            val cutoff = Instant.now().minus(Duration.ofHours(24))
            return conversations.none { it.status.isLive || it.updatedAt.isAfter(cutoff) }
        }
}

/** What an [ActivitySection] represents — drives its header icon / label / tint. */
enum class ActivityKind { RUNNING, RECENT }

/** One rendered group in the Activity feed (Running / Last 24 Hours), pre-sorted. */
data class ActivitySection(
    val id: String,
    val kind: ActivityKind,
    val rows: List<SessionRowItem>,
) {
    val count: Int get() = rows.size
}

/**
 * Split the conversation list into the Running (live, newest-first) and Last-24h
 * (non-live, updated within 24h, newest-first) sections, dropping an empty one. Pure;
 * the view model runs it on [Dispatchers.Default]. `folderName` is pre-resolved here so
 * the rows need no map lookup at compose time.
 */
fun buildActivitySections(
    conversations: List<ConversationSummary>,
    folderNames: Map<Int, String>,
): List<ActivitySection> {
    fun row(c: ConversationSummary) = SessionRowItem(c, folderNames[c.folderId])

    val running = conversations.filter { it.status.isLive }
        .sortedByDescending { it.updatedAt }
    val cutoff = Instant.now().minus(Duration.ofHours(24))
    val recent = conversations.filter { !it.status.isLive && it.updatedAt.isAfter(cutoff) }
        .sortedByDescending { it.updatedAt }

    val out = ArrayList<ActivitySection>(2)
    if (running.isNotEmpty()) out += ActivitySection("running", ActivityKind.RUNNING, running.map(::row))
    if (recent.isNotEmpty()) out += ActivitySection("recent", ActivityKind.RECENT, recent.map(::row))
    return out
}
