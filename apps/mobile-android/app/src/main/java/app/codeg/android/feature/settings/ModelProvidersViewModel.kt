package app.codeg.android.feature.settings

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.codeg.android.core.data.ServerRepository
import app.codeg.android.core.model.AgentType
import app.codeg.android.core.model.ModelProviderInfo
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

/** CRUD for custom model providers (OpenAI-compatible endpoints), grouped by agent. */
@HiltViewModel
class ModelProvidersViewModel @Inject constructor(
    private val repository: ServerRepository,
) : ViewModel() {

    private val _ui = MutableStateFlow(ModelProvidersUiState())
    val ui: StateFlow<ModelProvidersUiState> = _ui.asStateFlow()

    private var client: CodegClient? = null

    init { load() }

    fun load() {
        _ui.update { it.copy(loading = true, error = null) }
        viewModelScope.launch {
            try {
                val profile = repository.selectedProfile.first() ?: throw IllegalStateException("No server selected")
                val c = repository.client(profile) ?: throw IllegalStateException("Missing token")
                client = c
                _ui.update { it.copy(loading = false, providers = c.listModelProviders(), error = null) }
            } catch (e: Exception) {
                _ui.update { it.copy(loading = false, error = e.displayMessage()) }
            }
        }
    }

    fun save(existing: ModelProviderInfo?, name: String, apiUrl: String, apiKey: String, agentType: AgentType, model: String?) {
        val c = client ?: return
        viewModelScope.launch {
            try {
                if (existing == null) {
                    c.createModelProvider(name.trim(), apiUrl.trim(), apiKey, agentType, model?.trim()?.ifEmpty { null })
                } else {
                    c.updateModelProvider(
                        existing.id,
                        name = name.trim(),
                        apiUrl = apiUrl.trim(),
                        apiKey = apiKey.ifEmpty { null },
                        agentType = agentType,
                        model = model?.trim(),
                    )
                }
                load()
            } catch (e: Exception) {
                _ui.update { it.copy(error = e.displayMessage()) }
            }
        }
    }

    fun delete(provider: ModelProviderInfo) {
        val c = client ?: return
        val previous = _ui.value.providers
        _ui.update { it.copy(providers = it.providers.filterNot { p -> p.id == provider.id }) }
        viewModelScope.launch {
            runCatching { c.deleteModelProvider(provider.id) }.onFailure { e -> _ui.update { it.copy(providers = previous, error = e.displayMessage()) } }
        }
    }
}

data class ModelProvidersUiState(
    val providers: List<ModelProviderInfo> = emptyList(),
    val loading: Boolean = false,
    val error: String? = null,
) {
    /** Supported agent types for model providers (iOS `modelProviderSupported`). */
    val supportedAgents: List<AgentType> = listOf(AgentType.CLAUDE_CODE, AgentType.CODEX, AgentType.GEMINI)
}
