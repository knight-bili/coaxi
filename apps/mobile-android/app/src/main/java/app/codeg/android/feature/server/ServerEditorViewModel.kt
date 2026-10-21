package app.codeg.android.feature.server

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.codeg.android.core.data.ServerRepository
import app.codeg.android.core.datastore.ServerProfile
import app.codeg.android.core.network.CodegClient
import app.codeg.android.core.network.displayMessage
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import java.util.UUID
import javax.inject.Inject

/** Add/edit form state for a server profile, plus the Test Connection probe. */
@HiltViewModel
class ServerEditorViewModel @Inject constructor(
    private val repository: ServerRepository,
    savedStateHandle: SavedStateHandle,
) : ViewModel() {

    private val editingId: String? = savedStateHandle["serverId"]
    val isEditing: Boolean = editingId != null

    var name by mutableStateOf("")
        private set
    var url by mutableStateOf("")
        private set
    var token by mutableStateOf("")
        private set
    var test by mutableStateOf<TestState>(TestState.Idle)
        private set

    init {
        if (editingId != null) loadExisting(editingId)
    }

    fun onNameChange(value: String) { name = value }
    fun onUrlChange(value: String) { url = value }
    fun onTokenChange(value: String) { token = value }

    val canSave: Boolean
        get() = name.isNotBlank() && url.isNotBlank() && token.isNotBlank()

    private fun loadExisting(id: String) {
        viewModelScope.launch {
            val profile = repository.profiles.first().firstOrNull { it.id == id } ?: return@launch
            name = profile.name
            url = profile.baseUrl
            token = repository.token(id).orEmpty()
        }
    }

    fun testConnection() {
        viewModelScope.launch {
            test = TestState.Testing
            test = try {
                val health = repository.transientClient(url.trim(), token.trim()).health()
                TestState.Success(health.version)
            } catch (e: Exception) {
                TestState.Failure(e.displayMessage())
            }
        }
    }

    fun save(onDone: () -> Unit) {
        viewModelScope.launch {
            val id = editingId ?: UUID.randomUUID().toString()
            val profile = ServerProfile(
                id = id,
                name = name.trim(),
                baseUrl = CodegClient.normalizeBaseUrl(url),
            )
            repository.save(profile, token.trim())
            if (editingId == null) repository.select(id)
            onDone()
        }
    }
}

sealed interface TestState {
    data object Idle : TestState
    data object Testing : TestState
    data class Success(val version: String) : TestState
    data class Failure(val message: String) : TestState
}
