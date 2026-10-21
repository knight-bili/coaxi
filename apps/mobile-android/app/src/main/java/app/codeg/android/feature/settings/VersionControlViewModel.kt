package app.codeg.android.feature.settings

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.codeg.android.core.data.ServerRepository
import app.codeg.android.core.model.GitDetectResult
import app.codeg.android.core.model.GitHubAccount
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
import javax.inject.Inject

/** Version Control: git installation/custom-path + linked GitHub accounts (read). */
@HiltViewModel
class VersionControlViewModel @Inject constructor(
    private val repository: ServerRepository,
) : ViewModel() {

    private val _ui = MutableStateFlow(VersionControlUiState())
    val ui: StateFlow<VersionControlUiState> = _ui.asStateFlow()

    private var client: CodegClient? = null

    init { load() }

    fun load() {
        _ui.update { it.copy(loading = true, error = null) }
        viewModelScope.launch {
            try {
                val profile = repository.selectedProfile.first() ?: throw IllegalStateException("No server selected")
                val c = repository.client(profile) ?: throw IllegalStateException("Missing token")
                client = c
                val (detect, settings, accounts) = coroutineScope {
                    val d = async { runCatching { c.detectGit() }.getOrDefault(GitDetectResult()) }
                    val s = async { runCatching { c.getGitSettings().customPath }.getOrNull() }
                    val a = async { runCatching { c.getGitHubAccounts() }.getOrDefault(emptyList()) }
                    Triple(d.await(), s.await(), a.await())
                }
                _ui.update { it.copy(loading = false, detect = detect, customPath = settings ?: "", accounts = accounts, error = null) }
            } catch (e: Exception) {
                _ui.update { it.copy(loading = false, error = e.displayMessage()) }
            }
        }
    }

    fun onPath(path: String) = _ui.update { it.copy(customPath = path, testResult = null) }

    fun test() {
        val c = client ?: return
        val path = _ui.value.customPath.trim()
        if (path.isEmpty()) return
        _ui.update { it.copy(testing = true, testResult = null) }
        viewModelScope.launch {
            val result = runCatching { c.testGitPath(path) }.getOrNull()
            _ui.update { it.copy(testing = false, testResult = result) }
        }
    }

    fun save() {
        val c = client ?: return
        val path = _ui.value.customPath.trim().ifEmpty { null }
        _ui.update { it.copy(saving = true) }
        viewModelScope.launch {
            try {
                c.updateGitSettings(path)
                _ui.update { it.copy(saving = false, detect = runCatching { c.detectGit() }.getOrDefault(it.detect)) }
            } catch (e: Exception) {
                _ui.update { it.copy(saving = false, error = e.displayMessage()) }
            }
        }
    }
}

data class VersionControlUiState(
    val loading: Boolean = false,
    val detect: GitDetectResult = GitDetectResult(),
    val customPath: String = "",
    val accounts: List<GitHubAccount> = emptyList(),
    val testing: Boolean = false,
    val testResult: GitDetectResult? = null,
    val saving: Boolean = false,
    val error: String? = null,
)
