package app.codeg.android.feature.search

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.codeg.android.core.data.ServerRepository
import app.codeg.android.core.datastore.RecentSearchesStore
import app.codeg.android.core.model.ConversationSummary
import app.codeg.android.core.network.CodegClient
import app.codeg.android.core.network.displayMessage
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.FlowPreview
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.debounce
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import javax.inject.Inject

/** Global conversation search: debounced server query + persisted recent terms. Port of iOS `SearchView`. */
@OptIn(FlowPreview::class)
@HiltViewModel
class SearchViewModel @Inject constructor(
    private val repository: ServerRepository,
    private val recentStore: RecentSearchesStore,
) : ViewModel() {

    private val _query = MutableStateFlow("")
    val query: StateFlow<String> = _query.asStateFlow()

    private val _ui = MutableStateFlow(SearchUiState())
    val ui: StateFlow<SearchUiState> = _ui.asStateFlow()

    val recent: StateFlow<List<String>> =
        recentStore.searches.stateIn(viewModelScope, SharingStarted.Eagerly, emptyList())

    private var client: CodegClient? = null
    private var folderNames: Map<Int, String> = emptyMap()

    init {
        viewModelScope.launch {
            repository.selectedProfile.collectLatest { profile ->
                client = profile?.let { repository.client(it) }
                folderNames = client?.let { c -> runCatching { c.listFolders() }.getOrNull()?.associate { it.id to it.name } } ?: emptyMap()
                _ui.update { it.copy(folderNames = folderNames) }
            }
        }
        viewModelScope.launch {
            _query.debounce(300).collectLatest { q -> runSearch(q) }
        }
    }

    fun onQueryChange(value: String) { _query.value = value }

    fun submit() {
        val q = _query.value.trim()
        if (q.isNotEmpty()) viewModelScope.launch { recentStore.add(q) }
    }

    fun useRecent(term: String) { _query.value = term }

    fun clearRecent() { viewModelScope.launch { recentStore.clear() } }

    /** Re-run the current query after a failure (drives the inline-error Retry button). */
    fun retry() { viewModelScope.launch { runSearch(_query.value) } }

    private suspend fun runSearch(raw: String) {
        val q = raw.trim()
        if (q.isEmpty()) {
            _ui.update { it.copy(results = emptyList(), searching = false, query = "", error = null) }
            return
        }
        val c = client ?: return
        _ui.update { it.copy(searching = true, query = q, error = null) }
        try {
            val results = c.listConversations(search = q, sortBy = "updated")
            _ui.update { it.copy(results = results, searching = false) }
            recentStore.add(q)
        } catch (e: CancellationException) {
            throw e
        } catch (e: Exception) {
            _ui.update { it.copy(searching = false, error = e.displayMessage()) }
        }
    }
}

data class SearchUiState(
    val query: String = "",
    val results: List<ConversationSummary> = emptyList(),
    val folderNames: Map<Int, String> = emptyMap(),
    val searching: Boolean = false,
    val error: String? = null,
)
