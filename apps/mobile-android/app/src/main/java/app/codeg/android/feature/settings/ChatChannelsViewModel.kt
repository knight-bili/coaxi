package app.codeg.android.feature.settings

import android.content.Context
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.codeg.android.R
import app.codeg.android.core.data.ServerRepository
import app.codeg.android.core.model.ChannelConnectionStatus
import app.codeg.android.core.model.ChannelType
import app.codeg.android.core.model.ChatChannelInfo
import app.codeg.android.core.model.ChatChannelMessageLog
import app.codeg.android.core.model.UpdateChatChannelBody
import app.codeg.android.core.model.WeixinQrcode
import app.codeg.android.core.network.CodegClient
import app.codeg.android.core.network.displayMessage
import dagger.hilt.android.lifecycle.HiltViewModel
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import javax.inject.Inject

/**
 * Chat channels: the channel list joined with live connection status, plus
 * create/update (incl. keyring token) and optimistic delete. The detail
 * sub-state (status/messages/token + connect/disconnect/test) is folded in so a
 * single VM drives both the list and the per-channel detail dialog. Mirrors iOS
 * `ChatChannelsSettingsModel` + `ChatChannelDetailModel`.
 */
@HiltViewModel
class ChatChannelsViewModel @Inject constructor(
    private val repository: ServerRepository,
    @ApplicationContext private val appContext: Context,
) : ViewModel() {

    private val _ui = MutableStateFlow(ChatChannelsUiState())
    val ui: StateFlow<ChatChannelsUiState> = _ui.asStateFlow()

    private var client: CodegClient? = null

    init { load() }

    fun load() {
        viewModelScope.launch {
            try {
                val profile = repository.selectedProfile.first() ?: throw IllegalStateException("No server selected")
                val c = repository.client(profile) ?: throw IllegalStateException("Missing token")
                client = c
                if (_ui.value.channels.isEmpty()) _ui.update { it.copy(phase = ChatChannelsPhase.LOADING) }
                val channels = c.listChatChannels()
                _ui.update { it.copy(phase = ChatChannelsPhase.LOADED, channels = channels, refreshError = null) }
                loadStatuses()
            } catch (e: Exception) {
                if (_ui.value.channels.isEmpty()) _ui.update { it.copy(phase = ChatChannelsPhase.FAILED, error = e.displayMessage()) }
                else _ui.update { it.copy(refreshError = e.displayMessage()) }
            }
        }
    }

    private suspend fun loadStatuses() {
        val c = client ?: return
        val list = runCatching { c.chatChannelStatus() }.getOrNull() ?: return
        _ui.update { it.copy(statuses = list.associate { s -> s.channelId to s.status }) }
    }

    fun statusFor(channel: ChatChannelInfo): ChannelConnectionStatus =
        _ui.value.statuses[channel.id] ?: ChannelConnectionStatus.DISCONNECTED

    fun dismissRefreshError() = _ui.update { it.copy(refreshError = null) }
    fun dismissToast() = _ui.update { it.copy(toast = null) }

    /** Instant enable/disable from the list row. Disabling a connected channel
     *  disconnects it first (web parity), then persists; reconciles by id. */
    fun setEnabled(channel: ChatChannelInfo, on: Boolean) {
        val c = client ?: return
        if (_ui.value.togglingEnabled.contains(channel.id)) return
        val previous = _ui.value.channels.firstOrNull { it.id == channel.id }?.enabled ?: return
        if (previous == on) return
        _ui.update { it.copy(togglingEnabled = it.togglingEnabled + channel.id, channels = it.channels.map { ch -> if (ch.id == channel.id) ch.withEnabled(on) else ch }) }
        viewModelScope.launch {
            try {
                if (!on) {
                    loadStatuses()
                    if (_ui.value.statuses[channel.id] == ChannelConnectionStatus.CONNECTED) {
                        c.disconnectChatChannel(channel.id)
                        _ui.update { it.copy(statuses = it.statuses + (channel.id to ChannelConnectionStatus.DISCONNECTED)) }
                    }
                }
                val updated = c.updateChatChannel(UpdateChatChannelBody(id = channel.id, enabled = on))
                _ui.update { it.copy(channels = it.channels.map { ch -> if (ch.id == channel.id) updated else ch }) }
            } catch (e: Exception) {
                _ui.update { it.copy(channels = it.channels.map { ch -> if (ch.id == channel.id) ch.withEnabled(previous) else ch }, refreshError = e.displayMessage()) }
            } finally {
                _ui.update { it.copy(togglingEnabled = it.togglingEnabled - channel.id) }
            }
        }
    }

    /** Create a channel (+ optional token), rolling the channel back if the token write fails. */
    suspend fun create(name: String, type: ChannelType, configJson: String, enabled: Boolean, dailyReportEnabled: Boolean, dailyReportTime: String?, token: String?) {
        val c = client ?: return
        val created = c.createChatChannel(name, type, configJson, enabled, dailyReportEnabled, dailyReportTime)
        if (!token.isNullOrEmpty()) {
            try {
                c.saveChatChannelToken(created.id, token)
            } catch (e: Exception) {
                runCatching { c.deleteChatChannel(created.id) } // keep create atomic
                load()
                throw e
            }
        }
        load()
    }

    suspend fun update(body: UpdateChatChannelBody, token: String?) {
        val c = client ?: return
        c.updateChatChannel(body)
        if (!token.isNullOrEmpty()) c.saveChatChannelToken(body.id, token)
        load()
        // Keep an open detail in sync.
        _ui.value.detail?.let { if (it.channelId == body.id) loadDetail(body.id) }
    }

    fun delete(channel: ChatChannelInfo) {
        val c = client ?: return
        val previous = _ui.value.channels
        _ui.update { it.copy(channels = it.channels.filterNot { ch -> ch.id == channel.id }) }
        viewModelScope.launch {
            try {
                c.deleteChatChannel(channel.id)
                _ui.update { it.copy(toast = appContext.getString(R.string.cc_toast_deleted, channel.name)) }
            } catch (e: Exception) {
                _ui.update { it.copy(channels = previous, toast = appContext.getString(R.string.cc_toast_delete_failed, e.displayMessage())) }
            }
        }
    }

    // region Detail

    fun openDetail(channel: ChatChannelInfo) {
        _ui.update { it.copy(detail = ChannelDetailState(channelId = channel.id)) }
        viewModelScope.launch { loadDetail(channel.id) }
    }

    fun closeDetail() = _ui.update { it.copy(detail = null) }
    fun dismissActionError() = updateDetail { it.copy(actionError = null) }
    fun dismissDetailToast() = updateDetail { it.copy(toast = null) }

    private fun updateDetail(transform: (ChannelDetailState) -> ChannelDetailState) =
        _ui.update { st -> st.detail?.let { d -> st.copy(detail = transform(d)) } ?: st }

    /** The channel a detail belongs to, resolved live from the list. */
    fun channelForDetail(detail: ChannelDetailState): ChatChannelInfo? =
        _ui.value.channels.firstOrNull { it.id == detail.channelId }

    private suspend fun loadDetail(channelId: Int) {
        val c = client ?: return
        loadStatuses()
        val status = _ui.value.statuses[channelId] ?: ChannelConnectionStatus.DISCONNECTED
        val messages = runCatching { c.listChatChannelMessages(channelId, 50) }.getOrDefault(emptyList())
        val hasToken = c.chatChannelHasToken(channelId)
        updateDetail { if (it.channelId == channelId) it.copy(status = status, messages = messages, hasToken = hasToken) else it }
    }

    private suspend fun refreshDetailStatus(channelId: Int) {
        loadStatuses()
        val status = _ui.value.statuses[channelId] ?: ChannelConnectionStatus.DISCONNECTED
        updateDetail { if (it.channelId == channelId) it.copy(status = status) else it }
    }

    fun connect() = detailAction("Connecting…", poll = true) { c, id -> c.connectChatChannel(id) }
    fun disconnect() = detailAction("Disconnected.") { c, id -> c.disconnectChatChannel(id) }
    fun test() = detailAction(appContext.getString(R.string.cc_test_message_sent)) { c, id -> c.testChatChannel(id) }

    private fun detailAction(note: String, poll: Boolean = false, op: suspend (CodegClient, Int) -> Unit) {
        val c = client ?: return
        val detail = _ui.value.detail ?: return
        val id = detail.channelId
        updateDetail { it.copy(busy = true) }
        viewModelScope.launch {
            try {
                op(c, id)
                updateDetail { it.copy(toast = note) }
                refreshDetailStatus(id)
                val messages = runCatching { c.listChatChannelMessages(id, 50) }.getOrDefault(emptyList())
                updateDetail { if (it.channelId == id) it.copy(messages = messages) else it }
                if (poll) pollUntilSettled(id)
            } catch (e: Exception) {
                updateDetail { it.copy(actionError = e.displayMessage()) }
            } finally {
                updateDetail { it.copy(busy = false) }
            }
        }
    }

    private suspend fun pollUntilSettled(channelId: Int) {
        repeat(6) {
            if (_ui.value.detail?.status != ChannelConnectionStatus.CONNECTING) return
            delay(1500)
            refreshDetailStatus(channelId)
        }
    }

    fun removeToken() {
        val c = client ?: return
        val id = _ui.value.detail?.channelId ?: return
        viewModelScope.launch {
            try {
                c.deleteChatChannelToken(id)
                updateDetail { it.copy(hasToken = false, toast = appContext.getString(R.string.cc_token_removed)) }
            } catch (e: Exception) {
                updateDetail { it.copy(actionError = e.displayMessage()) }
            }
        }
    }

    /** Called once the WeChat QR login is confirmed. */
    fun qrConnected() {
        val id = _ui.value.detail?.channelId ?: return
        viewModelScope.launch { loadDetail(id); load() }
    }

    // endregion

    // region UI helpers (suspend delegates for the editor + QR dialogs)

    suspend fun hasTokenFor(channelId: Int): Boolean = client?.chatChannelHasToken(channelId) ?: false

    suspend fun weixinQrcode(): WeixinQrcode? = runCatching { client?.weixinGetQrcode() }.getOrNull()

    suspend fun weixinCheck(channelId: Int, qrcode: String): String? =
        runCatching { client?.weixinCheckQrcode(channelId, qrcode) }.getOrNull()

    // endregion
}

enum class ChatChannelsPhase { LOADING, LOADED, FAILED }

data class ChannelDetailState(
    val channelId: Int,
    val status: ChannelConnectionStatus = ChannelConnectionStatus.DISCONNECTED,
    val messages: List<ChatChannelMessageLog> = emptyList(),
    val hasToken: Boolean = false,
    val busy: Boolean = false,
    val actionError: String? = null,
    val toast: String? = null,
)

data class ChatChannelsUiState(
    val phase: ChatChannelsPhase = ChatChannelsPhase.LOADING,
    val channels: List<ChatChannelInfo> = emptyList(),
    val statuses: Map<Int, ChannelConnectionStatus> = emptyMap(),
    val togglingEnabled: Set<Int> = emptySet(),
    val refreshError: String? = null,
    val error: String? = null,
    val toast: String? = null,
    val detail: ChannelDetailState? = null,
)
