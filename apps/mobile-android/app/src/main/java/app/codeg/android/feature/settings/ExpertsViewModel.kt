package app.codeg.android.feature.settings

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.codeg.android.core.data.ServerRepository
import app.codeg.android.core.model.AgentType
import app.codeg.android.core.model.ExpertInstallStatus
import app.codeg.android.core.model.ExpertLinkState
import app.codeg.android.core.model.ExpertListItem
import app.codeg.android.core.network.CodegClient
import app.codeg.android.core.network.displayMessage
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import javax.inject.Inject

/**
 * Built-in expert catalog (`experts_list`) plus the per-expert link matrix. Mirrors
 * iOS `ExpertsSettingsModel` + `ExpertDetailModel`: the catalog is grouped by
 * category in the web's fixed pipeline order, and a selected expert loads its
 * markdown + per-agent install status, with link/unlink toggles.
 */
@HiltViewModel
class ExpertsViewModel @Inject constructor(
    private val repository: ServerRepository,
) : ViewModel() {

    private val _ui = MutableStateFlow(ExpertsUiState())
    val ui: StateFlow<ExpertsUiState> = _ui.asStateFlow()

    private var client: CodegClient? = null

    init { load() }

    fun load() {
        viewModelScope.launch {
            try {
                val profile = repository.selectedProfile.first() ?: throw IllegalStateException("No server selected")
                val c = repository.client(profile) ?: throw IllegalStateException("Missing token")
                client = c
                if (_ui.value.experts.isEmpty()) _ui.update { it.copy(phase = ExpertsPhase.LOADING) }
                val experts = c.expertsList()
                // Agents are best-effort — a failure here shouldn't blank the catalog.
                val agents = runCatching {
                    c.acpListAgents().filter { it.available && it.enabled }.sortedBy { it.sortOrder }.map { it.agentType }
                }.getOrDefault(emptyList())
                _ui.update { it.copy(phase = ExpertsPhase.LOADED, experts = experts, agents = agents, refreshError = null) }
            } catch (e: Exception) {
                if (_ui.value.experts.isEmpty()) _ui.update { it.copy(phase = ExpertsPhase.FAILED, error = e.displayMessage()) }
                else _ui.update { it.copy(refreshError = e.displayMessage()) }
            }
        }
    }

    fun dismissRefreshError() = _ui.update { it.copy(refreshError = null) }

    // region Detail (markdown + link matrix)

    fun openDetail(expert: ExpertListItem) {
        _ui.update { it.copy(detail = DetailState(expert = expert, loading = true)) }
        val c = client ?: return
        viewModelScope.launch {
            try {
                val content = c.expertReadContent(expert.id)
                val statuses = c.expertInstallStatus(expert.id).associateBy { it.agentType }
                _ui.update { st ->
                    val d = st.detail
                    if (d == null || d.expert.id != expert.id) st // superseded
                    else st.copy(detail = d.copy(loading = false, content = content, statusByAgent = statuses, error = null))
                }
            } catch (e: Exception) {
                _ui.update { st ->
                    val d = st.detail
                    if (d == null || d.expert.id != expert.id) st
                    else st.copy(detail = d.copy(loading = false, error = e.displayMessage()))
                }
            }
        }
    }

    fun closeDetail() = _ui.update { it.copy(detail = null) }

    fun dismissDetailError() = updateDetail { it.copy(error = null) }

    /** Apply [transform] to the open detail, if any (centralizes the nullable map). */
    private fun updateDetail(transform: (DetailState) -> DetailState) =
        _ui.update { st -> st.detail?.let { d -> st.copy(detail = transform(d)) } ?: st }

    fun toggleLink(agent: AgentType, on: Boolean) {
        val c = client ?: return
        val detail = _ui.value.detail ?: return
        val expertId = detail.expert.id
        if (detail.toggling.contains(agent)) return
        updateDetail { it.copy(toggling = it.toggling + agent) }
        viewModelScope.launch {
            try {
                if (on) {
                    val status = c.expertLink(expertId, agent)
                    updateDetail { it.copy(statusByAgent = it.statusByAgent + (agent to status)) }
                } else {
                    c.expertUnlink(expertId, agent)
                    updateDetail { d ->
                        val prev = d.statusByAgent[agent]
                        val cleared = ExpertInstallStatus(
                            expertId = expertId, agentType = agent, state = ExpertLinkState.NOT_LINKED,
                            linkPath = prev?.linkPath ?: "", targetPath = null,
                            expectedTargetPath = prev?.expectedTargetPath ?: "", copyMode = false,
                        )
                        d.copy(statusByAgent = d.statusByAgent + (agent to cleared))
                    }
                }
            } catch (e: Exception) {
                updateDetail { it.copy(error = e.displayMessage()) }
                openDetail(detail.expert) // re-sync real state after a failed toggle
            } finally {
                updateDetail { it.copy(toggling = it.toggling - agent) }
            }
        }
    }

    // endregion
}

enum class ExpertsPhase { LOADING, LOADED, FAILED }

data class DetailState(
    val expert: ExpertListItem,
    val loading: Boolean = true,
    val content: String = "",
    val statusByAgent: Map<AgentType, ExpertInstallStatus> = emptyMap(),
    val toggling: Set<AgentType> = emptySet(),
    val error: String? = null,
)

data class ExpertsUiState(
    val phase: ExpertsPhase = ExpertsPhase.LOADING,
    val experts: List<ExpertListItem> = emptyList(),
    val agents: List<AgentType> = emptyList(),
    val refreshError: String? = null,
    val error: String? = null,
    val detail: DetailState? = null,
) {
    /** Experts grouped by category in pipeline order, each group's items by sortOrder then id. */
    val grouped: List<Triple<String, String, List<ExpertListItem>>>
        get() = experts.groupBy { it.metadata.category }
            .toList()
            .sortedWith(compareBy({ ExpertCategory.rank(it.first) }, { it.first }))
            .map { (key, items) ->
                Triple(
                    key,
                    ExpertCategory.label(key),
                    items.sortedWith(compareBy({ it.metadata.sortOrder }, { it.metadata.id })),
                )
            }
}

/** Built-in expert categories in the web's fixed pipeline order (mirrors `CATEGORY_SORT`). */
object ExpertCategory {
    private val known = mapOf(
        "discovery" to (1 to "Discovery"),
        "planning" to (2 to "Planning"),
        "execution" to (3 to "Execution"),
        "quality" to (4 to "Quality"),
        "debugging" to (5 to "Debugging"),
        "review" to (6 to "Review"),
        "meta" to (7 to "Meta"),
    )

    fun rank(key: String): Int = known[key]?.first ?: 99

    fun label(key: String): String {
        known[key]?.second?.let { return it }
        val cleaned = key.replace('_', ' ').replace('-', ' ').trim()
        return if (cleaned.isEmpty()) "Other" else cleaned.replaceFirstChar { it.uppercase() }
    }
}
