package app.codeg.android.feature.settings

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.codeg.android.core.data.ServerRepository
import app.codeg.android.core.model.ChatEventCatalog
import app.codeg.android.core.model.WebhookConfig
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

/** Cross-channel message behavior: command prefix, bot reply language, the
 *  forwarded-event filter, and global outbound webhooks. Mirrors iOS `ChatGlobalSettingsView`. */
@HiltViewModel
class ChatGlobalSettingsViewModel @Inject constructor(
    private val repository: ServerRepository,
) : ViewModel() {

    private val _ui = MutableStateFlow(ChatGlobalUiState())
    val ui: StateFlow<ChatGlobalUiState> = _ui.asStateFlow()

    private var client: CodegClient? = null

    init { load() }

    fun load() {
        _ui.update { it.copy(loading = true, error = null) }
        viewModelScope.launch {
            try {
                val profile = repository.selectedProfile.first() ?: throw IllegalStateException("No server selected")
                val c = repository.client(profile) ?: throw IllegalStateException("Missing token")
                client = c
                val prefix = c.chatCommandPrefix()
                val language = c.chatMessageLanguage()
                val filter = c.chatEventFilter()
                val webhooks = c.chatEventWebhooks()
                _ui.update {
                    it.copy(
                        loading = false,
                        commandPrefix = prefix,
                        language = language,
                        enabledEvents = filter?.toSet() ?: ChatEventCatalog.defaultEnabled,
                        webhooks = webhooks,
                        error = null,
                    )
                }
            } catch (e: Exception) {
                _ui.update { it.copy(loading = false, error = e.displayMessage()) }
            }
        }
    }

    fun setCommandPrefix(prefix: String) {
        val c = client ?: return
        val clean = prefix.ifBlank { "/" }
        _ui.update { it.copy(commandPrefix = clean) }
        viewModelScope.launch { runCatching { c.setChatCommandPrefix(clean) }.onFailure { e -> _ui.update { it.copy(error = e.displayMessage()) } } }
    }

    fun setLanguage(code: String) {
        val c = client ?: return
        _ui.update { it.copy(language = code) }
        viewModelScope.launch { runCatching { c.setChatMessageLanguage(code) }.onFailure { e -> _ui.update { it.copy(error = e.displayMessage()) } } }
    }

    fun toggleEvent(id: String, on: Boolean) {
        val c = client ?: return
        val next = if (on) _ui.value.enabledEvents + id else _ui.value.enabledEvents - id
        _ui.update { it.copy(enabledEvents = next) }
        // Persist the explicit list (ordered by the catalog).
        val list = ChatEventCatalog.all.map { it.id }.filter { next.contains(it) }
        viewModelScope.launch { runCatching { c.setChatEventFilter(list) }.onFailure { e -> _ui.update { it.copy(error = e.displayMessage()) } } }
    }

    fun addWebhook(url: String) {
        val trimmed = url.trim()
        if (trimmed.isEmpty()) return
        persistWebhooks(_ui.value.webhooks + WebhookConfig(trimmed, true))
    }

    fun setWebhookEnabled(index: Int, on: Boolean) {
        val next = _ui.value.webhooks.toMutableList()
        if (index !in next.indices) return
        next[index] = next[index].copy(enabled = on)
        persistWebhooks(next)
    }

    fun removeWebhook(index: Int) {
        val next = _ui.value.webhooks.toMutableList()
        if (index !in next.indices) return
        next.removeAt(index)
        persistWebhooks(next)
    }

    private fun persistWebhooks(next: List<WebhookConfig>) {
        val c = client ?: return
        val previous = _ui.value.webhooks
        _ui.update { it.copy(webhooks = next) }
        viewModelScope.launch {
            runCatching { c.setChatEventWebhooks(next) }.onFailure { e -> _ui.update { it.copy(webhooks = previous, error = e.displayMessage()) } }
        }
    }

    fun dismissError() = _ui.update { it.copy(error = null) }
}

data class ChatGlobalUiState(
    val loading: Boolean = false,
    val commandPrefix: String = "/",
    val language: String = "en",
    val enabledEvents: Set<String> = ChatEventCatalog.defaultEnabled,
    val webhooks: List<WebhookConfig> = emptyList(),
    val error: String? = null,
)
