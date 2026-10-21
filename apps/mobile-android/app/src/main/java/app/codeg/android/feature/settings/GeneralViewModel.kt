package app.codeg.android.feature.settings

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.codeg.android.core.data.ServerRepository
import app.codeg.android.core.network.CodegClient
import app.codeg.android.core.network.displayMessage
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.async
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.intOrNull
import kotlinx.serialization.json.jsonPrimitive
import javax.inject.Inject

/** General settings: conversation-tool toggles + multi-agent delegation. */
@HiltViewModel
class GeneralViewModel @Inject constructor(
    private val repository: ServerRepository,
) : ViewModel() {

    private val _ui = MutableStateFlow(GeneralUiState())
    val ui: StateFlow<GeneralUiState> = _ui.asStateFlow()

    private var client: CodegClient? = null
    private var delegationRaw: JsonObject = JsonObject(emptyMap())

    init { load() }

    fun load() {
        _ui.update { it.copy(loading = true, error = null) }
        viewModelScope.launch {
            try {
                val profile = repository.selectedProfile.first() ?: throw IllegalStateException("No server selected")
                val c = repository.client(profile) ?: throw IllegalStateException("Missing token")
                client = c
                val result = coroutineScope {
                    val feedback = async { runCatching { c.getFeedbackEnabled() }.getOrDefault(true) }
                    val question = async { runCatching { c.getQuestionEnabled() }.getOrDefault(true) }
                    val delegation = async { runCatching { c.getDelegationSettings() }.getOrDefault(JsonObject(emptyMap())) }
                    Triple(feedback.await(), question.await(), delegation.await())
                }
                delegationRaw = result.third
                _ui.update {
                    it.copy(
                        loading = false,
                        feedbackEnabled = result.first,
                        questionEnabled = result.second,
                        delegationEnabled = delegationRaw["enabled"]?.jsonPrimitive?.booleanOrNull ?: false,
                        depthLimit = delegationRaw["depth_limit"]?.jsonPrimitive?.intOrNull ?: 3,
                        error = null,
                    )
                }
            } catch (e: Exception) {
                _ui.update { it.copy(loading = false, error = e.displayMessage()) }
            }
        }
    }

    fun setFeedback(enabled: Boolean) {
        _ui.update { it.copy(feedbackEnabled = enabled) }
        val c = client ?: return
        viewModelScope.launch { runCatching { c.setFeedbackEnabled(enabled) }.onFailure { reload() } }
    }

    fun setQuestion(enabled: Boolean) {
        _ui.update { it.copy(questionEnabled = enabled) }
        val c = client ?: return
        viewModelScope.launch { runCatching { c.setQuestionEnabled(enabled) }.onFailure { reload() } }
    }

    fun setDelegationEnabled(enabled: Boolean) {
        _ui.update { it.copy(delegationEnabled = enabled) }
        patchDelegation("enabled", JsonPrimitive(enabled))
    }

    fun setDepth(depth: Int) {
        val clamped = depth.coerceIn(1, 8)
        _ui.update { it.copy(depthLimit = clamped) }
        patchDelegation("depth_limit", JsonPrimitive(clamped))
    }

    private fun patchDelegation(key: String, value: JsonPrimitive) {
        val c = client ?: return
        delegationRaw = JsonObject(delegationRaw + (key to value))
        val snapshot = delegationRaw
        viewModelScope.launch { runCatching { c.setDelegationSettings(snapshot) }.onFailure { reload() } }
    }

    private fun reload() = load()
}

data class GeneralUiState(
    val loading: Boolean = false,
    val feedbackEnabled: Boolean = true,
    val questionEnabled: Boolean = true,
    val delegationEnabled: Boolean = false,
    val depthLimit: Int = 3,
    val error: String? = null,
)
