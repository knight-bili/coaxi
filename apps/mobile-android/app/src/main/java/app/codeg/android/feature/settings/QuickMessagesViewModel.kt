package app.codeg.android.feature.settings

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.codeg.android.core.data.ServerRepository
import app.codeg.android.core.model.QuickMessage
import app.codeg.android.core.network.CodegClient
import app.codeg.android.core.network.displayMessage
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import javax.inject.Inject

/** CRUD for reusable Quick Messages (the canonical server-settings list pattern). */
@HiltViewModel
class QuickMessagesViewModel @Inject constructor(
    private val repository: ServerRepository,
) : ViewModel() {

    private val _ui = MutableStateFlow(QuickMessagesUiState())
    val ui: StateFlow<QuickMessagesUiState> = _ui.asStateFlow()

    private var client: CodegClient? = null

    init { load() }

    fun load() {
        _ui.update { it.copy(loading = true, error = null) }
        viewModelScope.launch {
            try {
                val profile = repository.selectedProfile.first() ?: throw IllegalStateException("No server selected")
                val c = repository.client(profile) ?: throw IllegalStateException("Missing token")
                client = c
                val list = c.quickMessagesList().sortedBy { it.sortOrder }
                _ui.update { it.copy(loading = false, messages = list, error = null) }
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                _ui.update { it.copy(loading = false, error = e.displayMessage()) }
            }
        }
    }

    fun save(existing: QuickMessage?, title: String, content: String) {
        val c = client ?: return
        viewModelScope.launch {
            try {
                if (existing == null) c.quickMessageCreate(title.trim(), content) else c.quickMessageUpdate(existing.id, title.trim(), content)
                load()
            } catch (e: Exception) {
                _ui.update { it.copy(error = e.displayMessage()) }
            }
        }
    }

    fun delete(message: QuickMessage) {
        val c = client ?: return
        val previous = _ui.value.messages
        _ui.update { it.copy(messages = it.messages.filterNot { m -> m.id == message.id }) }
        viewModelScope.launch {
            try {
                c.quickMessageDelete(message.id)
            } catch (e: Exception) {
                _ui.update { it.copy(messages = previous, error = e.displayMessage()) }
            }
        }
    }
}

data class QuickMessagesUiState(
    val messages: List<QuickMessage> = emptyList(),
    val loading: Boolean = false,
    val error: String? = null,
)
