package app.codeg.android.feature.settings

import android.content.Context
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.codeg.android.R
import app.codeg.android.core.data.ServerRepository
import app.codeg.android.core.model.LocalMcpServer
import app.codeg.android.core.network.CodegClient
import app.codeg.android.core.network.displayMessage
import dagger.hilt.android.lifecycle.HiltViewModel
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.serialization.json.Json
import javax.inject.Inject

/** CRUD for local MCP servers (free-form JSON spec + per-app enablement). */
@HiltViewModel
class McpViewModel @Inject constructor(
    private val repository: ServerRepository,
    @ApplicationContext private val appContext: Context,
) : ViewModel() {

    private val _ui = MutableStateFlow(McpUiState())
    val ui: StateFlow<McpUiState> = _ui.asStateFlow()

    private var client: CodegClient? = null
    private val lenient = Json { ignoreUnknownKeys = true; isLenient = true }

    init { load() }

    fun load() {
        _ui.update { it.copy(loading = true, error = null) }
        viewModelScope.launch {
            try {
                val profile = repository.selectedProfile.first() ?: throw IllegalStateException("No server selected")
                val c = repository.client(profile) ?: throw IllegalStateException("Missing token")
                client = c
                _ui.update { it.copy(loading = false, servers = c.mcpScanLocal().sortedBy { s -> s.id.lowercase() }, error = null) }
            } catch (e: Exception) {
                _ui.update { it.copy(loading = false, error = e.displayMessage()) }
            }
        }
    }

    /** Returns an error string (e.g. invalid JSON) or null on success. */
    fun save(originalId: String?, serverId: String, specText: String, apps: List<String>, onResult: (String?) -> Unit) {
        val c = client ?: return onResult(appContext.getString(R.string.server_none_selected))
        val spec = runCatching { lenient.parseToJsonElement(specText) }.getOrNull()
            ?: return onResult(appContext.getString(R.string.mcp_spec_invalid_json))
        viewModelScope.launch {
            try {
                c.mcpUpsertLocalServer(serverId.trim(), spec, apps)
                if (originalId != null && originalId != serverId.trim()) runCatching { c.mcpRemoveServer(originalId) }
                load()
                onResult(null)
            } catch (e: Exception) {
                onResult(e.displayMessage())
            }
        }
    }

    fun delete(server: LocalMcpServer) {
        val c = client ?: return
        val previous = _ui.value.servers
        _ui.update { it.copy(servers = it.servers.filterNot { s -> s.id == server.id }) }
        viewModelScope.launch {
            runCatching { c.mcpRemoveServer(server.id) }.onFailure { e -> _ui.update { it.copy(servers = previous, error = e.displayMessage()) } }
        }
    }
}

data class McpUiState(
    val servers: List<LocalMcpServer> = emptyList(),
    val loading: Boolean = false,
    val error: String? = null,
)
