package app.codeg.android.feature.settings

import android.graphics.BitmapFactory
import android.util.Base64
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.Chat
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material.icons.automirrored.rounded.Send
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.Bolt
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.Forum
import androidx.compose.material.icons.rounded.NotificationsActive
import androidx.compose.material.icons.rounded.QrCode
import androidx.compose.material.icons.rounded.Refresh
import androidx.compose.material.icons.rounded.Tune
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.codeg.android.R
import app.codeg.android.core.designsystem.component.CodegFilterChip
import app.codeg.android.core.designsystem.component.CodegTextField
import app.codeg.android.core.designsystem.component.EmptyState
import app.codeg.android.core.designsystem.component.InlineError
import app.codeg.android.core.designsystem.component.LoadingView
import app.codeg.android.core.designsystem.component.SecretField
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.model.ChannelConfig
import app.codeg.android.core.model.ChannelConnectionStatus
import app.codeg.android.core.model.ChannelType
import app.codeg.android.core.model.ChatChannelInfo
import app.codeg.android.core.model.ChatChannelMessageLog
import app.codeg.android.core.model.FieldEdit
import app.codeg.android.core.model.UpdateChatChannelBody
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject

// region Visuals

internal fun channelTint(type: ChannelType): Color = when (type) {
    ChannelType.LARK -> Color(0xFF338CFF)
    ChannelType.TELEGRAM -> Color(0xFF26A6F2)
    ChannelType.WEIXIN -> Color(0xFF1AB833)
}

internal fun channelIcon(type: ChannelType): ImageVector = when (type) {
    ChannelType.LARK -> Icons.Rounded.Forum
    ChannelType.TELEGRAM -> Icons.AutoMirrored.Rounded.Send
    ChannelType.WEIXIN -> Icons.AutoMirrored.Rounded.Chat
}

@Composable
internal fun statusTint(status: ChannelConnectionStatus): Color = when (status) {
    ChannelConnectionStatus.CONNECTED -> Color(0xFF4DC762)
    ChannelConnectionStatus.CONNECTING -> Color(0xFFFF9E4D)
    ChannelConnectionStatus.DISCONNECTED -> CodegTheme.colors.textTertiary
    ChannelConnectionStatus.ERROR -> CodegTheme.colors.danger
}

@Composable
internal fun ChannelTypeAvatar(type: ChannelType, size: Dp) {
    val tint = channelTint(type)
    Box(Modifier.size(size).clip(CircleShape).background(tint.copy(alpha = 0.16f)), contentAlignment = Alignment.Center) {
        Icon(channelIcon(type), null, tint = tint, modifier = Modifier.size(size * 0.5f))
    }
}

@Composable
internal fun ChannelStatusPill(status: ChannelConnectionStatus) {
    val tint = statusTint(status)
    Text(
        status.label, fontSize = 10.sp, fontWeight = FontWeight.SemiBold, color = tint,
        modifier = Modifier.clip(RoundedCornerShape(50)).background(tint.copy(alpha = 0.14f)).padding(horizontal = 7.dp, vertical = 2.dp),
    )
}

// endregion

private sealed interface ChannelEditorTarget {
    data object Add : ChannelEditorTarget
    data class Edit(val channel: ChatChannelInfo) : ChannelEditorTarget
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
fun ChatChannelsContent(viewModel: ChatChannelsViewModel = hiltViewModel()) {
    val ui by viewModel.ui.collectAsStateWithLifecycle()
    val colors = CodegTheme.colors
    var editor by remember { mutableStateOf<ChannelEditorTarget?>(null) }
    var showGlobal by remember { mutableStateOf(false) }
    var pendingDelete by remember { mutableStateOf<ChatChannelInfo?>(null) }

    Box(Modifier.fillMaxSize()) {
        when {
            ui.phase == ChatChannelsPhase.LOADING && ui.channels.isEmpty() ->
                Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { LoadingView(stringResource(R.string.cc_loading_channels)) }
            ui.phase == ChatChannelsPhase.FAILED && ui.channels.isEmpty() ->
                Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                    InlineError(Icons.Rounded.NotificationsActive, stringResource(R.string.cc_couldnt_load), ui.error ?: stringResource(R.string.cc_unknown_error), onRetry = { viewModel.load() })
                }
            else -> LazyColumn(
                Modifier.fillMaxSize(),
                contentPadding = androidx.compose.foundation.layout.PaddingValues(horizontal = 12.dp, vertical = 8.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                if (ui.refreshError != null) {
                    item("err") { ChatRefreshBanner(ui.refreshError!!, onRetry = { viewModel.load() }, onDismiss = { viewModel.dismissRefreshError() }) }
                }
                if (ui.channels.isEmpty()) {
                    item("empty") {
                        EmptyState(
                            Icons.Rounded.NotificationsActive, stringResource(R.string.cc_no_chat_channels),
                            stringResource(R.string.cc_empty_subtitle),
                            actionLabel = stringResource(R.string.cc_add_channel), onAction = { editor = ChannelEditorTarget.Add },
                        )
                    }
                } else {
                    items(ui.channels, key = { it.id }) { channel ->
                        ChannelRow(
                            channel = channel,
                            status = viewModel.statusFor(channel),
                            toggling = ui.togglingEnabled.contains(channel.id),
                            onOpen = { viewModel.openDetail(channel) },
                            onToggle = { viewModel.setEnabled(channel, it) },
                            onDelete = { pendingDelete = channel },
                        )
                    }
                }
                item("general") {
                    Spacer(Modifier.height(8.dp))
                    Text(stringResource(R.string.cc_section_general), fontSize = 11.sp, fontWeight = FontWeight.SemiBold, color = colors.textTertiary, modifier = Modifier.padding(start = 4.dp, bottom = 6.dp))
                    Row(
                        Modifier.fillMaxWidth().clip(RoundedCornerShape(14.dp)).background(colors.bgElevated.copy(alpha = 0.5f)).clickable { showGlobal = true }.padding(13.dp),
                        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(13.dp),
                    ) {
                        Box(Modifier.size(40.dp).clip(RoundedCornerShape(11.dp)).background(colors.accent.copy(alpha = 0.16f)), contentAlignment = Alignment.Center) {
                            Icon(Icons.Rounded.Tune, null, tint = colors.accent, modifier = Modifier.size(20.dp))
                        }
                        Column(Modifier.weight(1f)) {
                            Text(stringResource(R.string.cc_message_settings), fontSize = 15.sp, fontWeight = FontWeight.SemiBold, color = colors.textPrimary)
                            Text(stringResource(R.string.cc_message_settings_subtitle), fontSize = 12.sp, color = colors.textSecondary, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        }
                        Icon(Icons.AutoMirrored.Rounded.KeyboardArrowRight, null, tint = colors.textTertiary, modifier = Modifier.size(16.dp))
                    }
                }
            }
        }

        // Add FAB.
        FloatingActionButton(
            onClick = { editor = ChannelEditorTarget.Add },
            modifier = Modifier.align(Alignment.BottomEnd).padding(20.dp),
            containerColor = colors.accent,
            contentColor = colors.onAccent,
        ) { Icon(Icons.Rounded.Add, contentDescription = stringResource(R.string.cc_add_channel)) }

        // Transient toast → Material Snackbar.
        val snackbarHostState = remember { SnackbarHostState() }
        LaunchedEffect(ui.toast) {
            ui.toast?.let {
                snackbarHostState.showSnackbar(it)
                viewModel.dismissToast()
            }
        }
        SnackbarHost(snackbarHostState, modifier = Modifier.align(Alignment.BottomCenter))
    }

    // Detail dialog.
    ui.detail?.let { detail ->
        val channel = viewModel.channelForDetail(detail)
        if (channel != null) {
            ChatChannelDetailDialog(
                channel = channel, detail = detail, viewModel = viewModel,
                onEdit = { editor = ChannelEditorTarget.Edit(channel) },
                onDismiss = { viewModel.closeDetail() },
            )
        }
    }

    // Editor dialog.
    editor?.let { target ->
        val existing = (target as? ChannelEditorTarget.Edit)?.channel
        ChatChannelEditorDialog(
            existing = existing,
            viewModel = viewModel,
            onDismiss = { editor = null },
        )
    }

    // Global settings dialog.
    if (showGlobal) ChatGlobalSettingsDialog(onDismiss = { showGlobal = false })

    // Delete confirm.
    pendingDelete?.let { channel ->
        AlertDialog(
            onDismissRequest = { pendingDelete = null },
            containerColor = colors.bgElevated,
            title = { Text(stringResource(R.string.cc_delete_channel), color = colors.textPrimary) },
            text = { Text(stringResource(R.string.cc_delete_channel_message, channel.name), color = colors.textSecondary) },
            confirmButton = { TextButton(onClick = { viewModel.delete(channel); pendingDelete = null }) { Text(stringResource(R.string.common_delete), color = colors.danger) } },
            dismissButton = { TextButton(onClick = { pendingDelete = null }) { Text(stringResource(R.string.common_cancel), color = colors.textSecondary) } },
        )
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun ChannelRow(
    channel: ChatChannelInfo,
    status: ChannelConnectionStatus,
    toggling: Boolean,
    onOpen: () -> Unit,
    onToggle: (Boolean) -> Unit,
    onDelete: () -> Unit,
) {
    val colors = CodegTheme.colors
    val summary = remember(channel.configJson, channel.channelType) { ChannelConfig.parse(channel.configJson).summary(channel.channelType) }
    Row(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(14.dp)).background(colors.bgElevated.copy(alpha = 0.5f))
            .combinedClickable(onClick = onOpen, onLongClick = onDelete).padding(12.dp),
        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        ChannelTypeAvatar(channel.channelType, 40.dp)
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(7.dp)) {
                Text(channel.name, fontSize = 15.sp, fontWeight = FontWeight.SemiBold, color = colors.textPrimary, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f, fill = false))
                ChannelStatusPill(status)
            }
            if (summary.isNotEmpty()) Text(summary, fontSize = 12.sp, color = colors.textSecondary, maxLines = 1, overflow = TextOverflow.Ellipsis)
            if (channel.dailyReportEnabled && channel.dailyReportTime != null) {
                Text(stringResource(R.string.cc_daily_report_at, channel.dailyReportTime ?: ""), fontSize = 11.sp, color = colors.textTertiary)
            }
        }
        Switch(
            checked = channel.enabled,
            onCheckedChange = onToggle,
            enabled = !toggling,
            colors = SwitchDefaults.colors(checkedTrackColor = colors.accent, checkedThumbColor = colors.onAccent),
        )
    }
}

@Composable
private fun ChatRefreshBanner(message: String, onRetry: () -> Unit, onDismiss: () -> Unit) {
    val colors = CodegTheme.colors
    Row(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp)).background(colors.danger.copy(alpha = 0.12f)).padding(horizontal = 12.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Text(message, fontSize = 12.sp, color = colors.danger, modifier = Modifier.weight(1f), maxLines = 2, overflow = TextOverflow.Ellipsis)
        Text(stringResource(R.string.common_retry), fontSize = 12.sp, fontWeight = FontWeight.SemiBold, color = colors.accent, modifier = Modifier.clickable { onRetry() })
        Icon(Icons.Rounded.Close, stringResource(R.string.common_dismiss), tint = colors.textTertiary, modifier = Modifier.size(16.dp).clickable { onDismiss() })
    }
}

// region Detail dialog

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ChatChannelDetailDialog(
    channel: ChatChannelInfo,
    detail: ChannelDetailState,
    viewModel: ChatChannelsViewModel,
    onEdit: () -> Unit,
    onDismiss: () -> Unit,
) {
    val colors = CodegTheme.colors
    var showQR by remember { mutableStateOf(false) }
    var confirmRemoveToken by remember { mutableStateOf(false) }

    Dialog(onDismissRequest = onDismiss, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        Column(Modifier.fillMaxSize().background(colors.bg).windowInsetsPadding(WindowInsets.safeDrawing)) {
            TopAppBar(
                title = { Text(channel.name, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                navigationIcon = {
                    IconButton(onClick = onDismiss) { Icon(Icons.Rounded.Close, contentDescription = stringResource(R.string.cc_close), tint = colors.textPrimary) }
                },
                actions = {
                    TextButton(onClick = onEdit) { Text(stringResource(R.string.common_edit), fontWeight = FontWeight.SemiBold, color = colors.accent) }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent, titleContentColor = colors.textPrimary),
            )
            Column(Modifier.weight(1f).verticalScroll(rememberScrollState()).padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
                // Header.
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp), modifier = Modifier.padding(top = 4.dp)) {
                    ChannelTypeAvatar(channel.channelType, 44.dp)
                    Column(verticalArrangement = Arrangement.spacedBy(3.dp)) {
                        Text(channel.name, fontSize = 18.sp, fontWeight = FontWeight.SemiBold, color = colors.textPrimary)
                        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                            Box(Modifier.size(7.dp).clip(CircleShape).background(statusTint(detail.status)))
                            Text(detail.status.label, fontSize = 12.sp, color = colors.textSecondary)
                            Text("·", color = colors.textTertiary)
                            Text(channel.channelType.displayName, fontSize = 12.sp, color = colors.textTertiary)
                        }
                    }
                }

                // Status / enable.
                DetailSection(stringResource(R.string.cc_section_status)) {
                    SettingRow {
                        Text(stringResource(R.string.cc_enabled), fontSize = 15.sp, color = colors.textPrimary, modifier = Modifier.weight(1f))
                        Switch(checked = channel.enabled, onCheckedChange = { viewModel.setEnabled(channel, it) }, colors = SwitchDefaults.colors(checkedTrackColor = colors.accent, checkedThumbColor = colors.onAccent))
                    }
                    if (channel.dailyReportEnabled && channel.dailyReportTime != null) {
                        Divider()
                        SettingRow {
                            Text(stringResource(R.string.cc_daily_report), fontSize = 15.sp, color = colors.textPrimary, modifier = Modifier.weight(1f))
                            Text(channel.dailyReportTime!!, fontSize = 14.sp, fontFamily = FontFamily.Monospace, color = colors.textSecondary)
                        }
                    }
                }

                // Connection actions.
                DetailSection(stringResource(R.string.cc_section_connection)) {
                    Column(Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                        when {
                            channel.channelType == ChannelType.WEIXIN ->
                                ActionButton(stringResource(R.string.cc_scan_qr_to_connect), Icons.Rounded.QrCode, colors.accent, enabled = !detail.busy) { showQR = true }
                            detail.status == ChannelConnectionStatus.CONNECTED ->
                                ActionButton(stringResource(R.string.cc_disconnect), Icons.Rounded.Bolt, colors.danger, enabled = !detail.busy) { viewModel.disconnect() }
                            else ->
                                ActionButton(stringResource(R.string.cc_connect), Icons.Rounded.Bolt, colors.accent, enabled = !detail.busy) { viewModel.connect() }
                        }
                        ActionButton(stringResource(R.string.cc_send_test_message), Icons.AutoMirrored.Rounded.Send, colors.accent, enabled = !detail.busy && detail.status == ChannelConnectionStatus.CONNECTED) { viewModel.test() }
                        if (detail.busy) Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            CircularProgressIndicator(color = colors.accent, modifier = Modifier.size(16.dp))
                            Text(stringResource(R.string.cc_working), fontSize = 12.sp, color = colors.textSecondary)
                        }
                    }
                }

                // Config.
                DetailSection(stringResource(R.string.cc_section_configuration)) {
                    SettingRow {
                        Text(stringResource(R.string.cc_config), fontSize = 15.sp, color = colors.textSecondary)
                        Spacer(Modifier.weight(1f))
                        Text(ChannelConfig.parse(channel.configJson).summary(channel.channelType), fontSize = 13.sp, fontFamily = FontFamily.Monospace, color = colors.textPrimary)
                    }
                }

                // Token.
                if (channel.channelType.secretLabel != null) {
                    DetailSection(stringResource(R.string.cc_section_token)) {
                        SettingRow {
                            Text(if (detail.hasToken) stringResource(R.string.cc_secret_is_set, channel.channelType.secretLabel ?: "") else stringResource(R.string.cc_no_token_set), fontSize = 15.sp, color = colors.textPrimary, modifier = Modifier.weight(1f))
                            if (detail.hasToken) Text(stringResource(R.string.common_remove), fontSize = 14.sp, color = colors.danger, modifier = Modifier.clickable { confirmRemoveToken = true })
                        }
                    }
                }

                // Recent messages.
                DetailSection(stringResource(R.string.cc_section_recent_messages)) {
                    if (detail.messages.isEmpty()) {
                        Text(stringResource(R.string.cc_no_messages_yet), fontSize = 13.sp, color = colors.textTertiary, modifier = Modifier.padding(16.dp))
                    } else {
                        detail.messages.take(20).forEachIndexed { index, msg ->
                            if (index > 0) Divider()
                            MessageLogRow(msg)
                        }
                    }
                }
                Spacer(Modifier.height(24.dp))
            }
        }
    }

    if (showQR) WeixinQRDialog(channelId = channel.id, viewModel = viewModel, onConnected = { viewModel.qrConnected() }, onDismiss = { showQR = false })

    if (confirmRemoveToken) {
        AlertDialog(
            onDismissRequest = { confirmRemoveToken = false },
            containerColor = colors.bgElevated,
            title = { Text(stringResource(R.string.cc_remove_token), color = colors.textPrimary) },
            text = { Text(stringResource(R.string.cc_remove_token_message), color = colors.textSecondary) },
            confirmButton = { TextButton(onClick = { viewModel.removeToken(); confirmRemoveToken = false }) { Text(stringResource(R.string.common_remove), color = colors.danger) } },
            dismissButton = { TextButton(onClick = { confirmRemoveToken = false }) { Text(stringResource(R.string.common_cancel), color = colors.textSecondary) } },
        )
    }

    detail.actionError?.let { err ->
        AlertDialog(
            onDismissRequest = { viewModel.dismissActionError() },
            containerColor = colors.bgElevated,
            title = { Text(stringResource(R.string.cc_something_went_wrong), color = colors.textPrimary) },
            text = { Text(err, color = colors.textSecondary) },
            confirmButton = { TextButton(onClick = { viewModel.dismissActionError() }) { Text(stringResource(R.string.common_ok), color = colors.accent) } },
        )
    }

    detail.toast?.let { toast ->
        LaunchedEffect(toast) { delay(3000); viewModel.dismissDetailToast() }
    }
}

@Composable
private fun MessageLogRow(message: ChatChannelMessageLog) {
    val colors = CodegTheme.colors
    Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 11.dp), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        Text(if (message.isInbound) "↙" else "↗", fontSize = 13.sp, fontWeight = FontWeight.Bold, color = if (message.failed) colors.danger else colors.textSecondary)
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(message.contentPreview.ifEmpty { "(${message.messageType})" }, fontSize = 13.sp, color = colors.textPrimary, maxLines = 2, overflow = TextOverflow.Ellipsis)
            if (message.failed && !message.errorDetail.isNullOrEmpty()) Text(message.errorDetail!!, fontSize = 11.sp, color = colors.danger, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
    }
}

@Composable
private fun ActionButton(title: String, icon: ImageVector, tint: Color, enabled: Boolean, onClick: () -> Unit) {
    FilledTonalButton(
        onClick = onClick,
        enabled = enabled,
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(12.dp),
        colors = ButtonDefaults.filledTonalButtonColors(
            containerColor = tint.copy(alpha = 0.14f),
            contentColor = tint,
        ),
    ) {
        Icon(icon, null, modifier = Modifier.size(18.dp))
        Spacer(Modifier.width(8.dp))
        Text(title, fontWeight = FontWeight.Medium)
    }
}

@Composable
private fun DetailSection(title: String, content: @Composable () -> Unit) {
    val colors = CodegTheme.colors
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Text(title, fontSize = 11.sp, fontWeight = FontWeight.SemiBold, color = colors.textTertiary, modifier = Modifier.padding(start = 4.dp))
        Column(Modifier.fillMaxWidth().clip(RoundedCornerShape(14.dp)).background(colors.bgElevated.copy(alpha = 0.5f))) { content() }
    }
}

@Composable
private fun SettingRow(content: @Composable androidx.compose.foundation.layout.RowScope.() -> Unit) {
    Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 13.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp), content = content)
}

@Composable
private fun Divider() {
    HorizontalDivider(modifier = Modifier.padding(start = 16.dp), thickness = Dp.Hairline, color = CodegTheme.colors.textPrimary.copy(alpha = 0.06f))
}

// endregion

// region Weixin QR

private enum class QrPhase { LOADING, SHOWING, SCANNED, EXPIRED, CONFIRMED, FAILED }

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun WeixinQRDialog(channelId: Int, viewModel: ChatChannelsViewModel, onConnected: () -> Unit, onDismiss: () -> Unit) {
    val colors = CodegTheme.colors
    val loadingQrText = stringResource(R.string.cc_loading_qr_code)
    val couldntLoadQrText = stringResource(R.string.cc_couldnt_load_qr)
    val scanWithWechatText = stringResource(R.string.cc_scan_with_wechat)
    val connectedText = stringResource(R.string.cc_connected)
    val qrExpiredText = stringResource(R.string.cc_qr_expired)
    val scannedConfirmText = stringResource(R.string.cc_scanned_confirm)
    var attempt by remember { mutableStateOf(0) }
    var phase by remember { mutableStateOf(QrPhase.LOADING) }
    var statusText by remember { mutableStateOf(loadingQrText) }
    var image by remember { mutableStateOf<androidx.compose.ui.graphics.ImageBitmap?>(null) }

    LaunchedEffect(attempt) {
        phase = QrPhase.LOADING; statusText = loadingQrText; image = null
        val qr = viewModel.weixinQrcode()
        if (qr == null) { phase = QrPhase.FAILED; statusText = couldntLoadQrText; return@LaunchedEffect }
        image = decodeQrImage(qr.qrcodeImgContent)
        phase = QrPhase.SHOWING; statusText = scanWithWechatText
        repeat(60) {
            delay(2000)
            val status = viewModel.weixinCheck(channelId, qr.qrcodeId)?.lowercase() ?: return@repeat
            when {
                status.contains("confirm") || status == "success" || status == "connected" || status.contains("logged") -> {
                    phase = QrPhase.CONFIRMED; statusText = connectedText; delay(800); onConnected(); onDismiss(); return@LaunchedEffect
                }
                status.contains("expire") -> { phase = QrPhase.EXPIRED; statusText = qrExpiredText; return@LaunchedEffect }
                status.contains("scan") -> { phase = QrPhase.SCANNED; statusText = scannedConfirmText }
            }
        }
        phase = QrPhase.EXPIRED; statusText = qrExpiredText
    }

    Dialog(onDismissRequest = onDismiss, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        Column(Modifier.fillMaxSize().background(colors.bg).windowInsetsPadding(WindowInsets.safeDrawing)) {
            TopAppBar(
                title = { Text(stringResource(R.string.cc_connect_wechat), maxLines = 1, overflow = TextOverflow.Ellipsis) },
                navigationIcon = {
                    IconButton(onClick = onDismiss) { Icon(Icons.Rounded.Close, contentDescription = stringResource(R.string.common_cancel), tint = colors.textPrimary) }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent, titleContentColor = colors.textPrimary),
            )
            Column(Modifier.fillMaxSize().padding(24.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(20.dp)) {
                Spacer(Modifier.weight(1f))
                Box(Modifier.size(240.dp).clip(RoundedCornerShape(18.dp)).background(Color.White), contentAlignment = Alignment.Center) {
                    when (phase) {
                        QrPhase.CONFIRMED -> Icon(Icons.Rounded.Bolt, null, tint = Color(0xFF1AB833), modifier = Modifier.size(64.dp))
                        QrPhase.EXPIRED, QrPhase.FAILED -> Icon(Icons.Rounded.Refresh, null, tint = Color.Black.copy(alpha = 0.4f), modifier = Modifier.size(56.dp))
                        else -> image?.let { Image(it, stringResource(R.string.cc_qr_code), Modifier.size(210.dp)) } ?: CircularProgressIndicator(color = Color.Black)
                    }
                }
                Text(statusText, fontSize = 14.sp, color = colors.textSecondary)
                if (phase == QrPhase.EXPIRED || phase == QrPhase.FAILED) {
                    FilledTonalButton(
                        onClick = { attempt++ },
                        shape = RoundedCornerShape(12.dp),
                        colors = ButtonDefaults.filledTonalButtonColors(containerColor = colors.accent.copy(alpha = 0.14f), contentColor = colors.accent),
                    ) {
                        Icon(Icons.Rounded.Refresh, null, modifier = Modifier.size(18.dp))
                        Spacer(Modifier.width(8.dp))
                        Text(stringResource(R.string.cc_refresh_qr_code))
                    }
                }
                Spacer(Modifier.weight(1f))
                Text(stringResource(R.string.cc_wechat_scan_instruction), fontSize = 12.sp, color = colors.textTertiary, modifier = Modifier.padding(horizontal = 16.dp))
            }
        }
    }
}

/** Decode a base64 PNG (optionally a `data:` URI) into an ImageBitmap. */
private fun decodeQrImage(raw: String): androidx.compose.ui.graphics.ImageBitmap? {
    var b64 = raw.trim()
    val comma = b64.indexOf(',')
    if (b64.startsWith("data:") && comma >= 0) b64 = b64.substring(comma + 1)
    return runCatching {
        val bytes = Base64.decode(b64, Base64.DEFAULT)
        BitmapFactory.decodeByteArray(bytes, 0, bytes.size)?.asImageBitmap()
    }.getOrNull()
}

// endregion

// region Editor dialog

private val lenientJson = Json { ignoreUnknownKeys = true; isLenient = true }

private fun normalizedObjectJson(raw: String): String? {
    val obj = runCatching { lenientJson.parseToJsonElement(raw) }.getOrNull() as? JsonObject ?: return null
    return JsonObject(obj.toSortedMap()).toString()
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ChatChannelEditorDialog(existing: ChatChannelInfo?, viewModel: ChatChannelsViewModel, onDismiss: () -> Unit) {
    val colors = CodegTheme.colors
    val scope = rememberCoroutineScope()
    val isEdit = existing != null

    var name by remember { mutableStateOf(existing?.name ?: "") }
    var type by remember { mutableStateOf(existing?.channelType ?: ChannelType.TELEGRAM) }
    val parsed = remember(existing?.id) { ChannelConfig.parse(existing?.configJson) }
    var chatId by remember { mutableStateOf(parsed.chatId) }
    var appId by remember { mutableStateOf(parsed.appId) }
    var baseUrl by remember { mutableStateOf(parsed.baseUrl) }
    var rawMode by remember { mutableStateOf(false) }
    var rawText by remember { mutableStateOf(existing?.configJson ?: "") }
    var token by remember { mutableStateOf("") }
    var hasToken by remember { mutableStateOf(false) }
    var enabled by remember { mutableStateOf(existing?.enabled ?: true) }
    var dailyEnabled by remember { mutableStateOf(existing?.dailyReportEnabled ?: false) }
    var dailyTime by remember { mutableStateOf(existing?.dailyReportTime ?: "18:00") }
    var saving by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    val couldntSaveText = stringResource(R.string.cc_couldnt_save_channel)

    LaunchedEffect(existing?.id) {
        if (existing != null) hasToken = viewModel.hasTokenFor(existing.id)
    }

    fun currentConfig() = ChannelConfig(chatId = chatId, appId = appId, baseUrl = baseUrl)
    val configJson: String? = if (rawMode) normalizedObjectJson(rawText) else currentConfig().toJson(type)
    val configValid = if (rawMode) normalizedObjectJson(rawText) != null else when (type) {
        ChannelType.TELEGRAM -> chatId.trim().isNotEmpty()
        ChannelType.LARK -> appId.trim().isNotEmpty() && chatId.trim().isNotEmpty()
        ChannelType.WEIXIN -> true
    }
    val tokenRequired = !isEdit && type.secretLabel != null && token.trim().isEmpty()
    val canSave = name.trim().isNotEmpty() && configValid && !saving && !tokenRequired

    fun save() {
        val cfg = configJson ?: return
        saving = true; error = null
        val tokenToSend = token.trim().ifEmpty { null }
        val timeString = if (dailyEnabled) dailyTime.trim() else null
        scope.launch {
            try {
                if (existing != null) {
                    val body = buildUpdateBody(existing, name.trim(), enabled, cfg, dailyEnabled, timeString)
                    viewModel.update(body, tokenToSend)
                } else {
                    viewModel.create(name.trim(), type, cfg, enabled, dailyEnabled, timeString, tokenToSend)
                }
                onDismiss()
            } catch (e: Exception) {
                error = e.message ?: couldntSaveText; saving = false
            }
        }
    }

    Dialog(onDismissRequest = onDismiss, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        Column(Modifier.fillMaxSize().background(colors.bg).windowInsetsPadding(WindowInsets.safeDrawing)) {
            TopAppBar(
                title = { Text(if (isEdit) stringResource(R.string.cc_edit_channel) else stringResource(R.string.cc_add_channel), maxLines = 1, overflow = TextOverflow.Ellipsis) },
                navigationIcon = {
                    IconButton(onClick = onDismiss) { Icon(Icons.Rounded.Close, contentDescription = stringResource(R.string.common_cancel), tint = colors.textPrimary) }
                },
                actions = {
                    TextButton(onClick = { save() }, enabled = canSave) {
                        Text(stringResource(R.string.common_save), fontWeight = FontWeight.SemiBold, color = if (canSave) colors.accent else colors.textTertiary)
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent, titleContentColor = colors.textPrimary),
            )
            Column(Modifier.weight(1f).verticalScroll(rememberScrollState()).padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(14.dp)) {
                Spacer(Modifier.height(2.dp))
                CodegTextField(name, { name = it }, label = stringResource(R.string.server_name), placeholder = stringResource(R.string.cc_name_placeholder))
                // Type selector (immutable on edit).
                Text(stringResource(R.string.cc_type), fontSize = 13.sp, fontWeight = FontWeight.SemiBold, color = colors.textSecondary)
                if (isEdit) {
                    Text(type.displayName, fontSize = 14.sp, color = colors.textSecondary)
                } else {
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        for (t in ChannelType.entries) {
                            CodegFilterChip(label = t.displayName, selected = t == type, onClick = { type = t })
                        }
                    }
                }
                // Config.
                Text(stringResource(R.string.cc_configuration), fontSize = 13.sp, fontWeight = FontWeight.SemiBold, color = colors.textSecondary)
                if (rawMode) {
                    CodegTextField(rawText, { rawText = it }, label = stringResource(R.string.cc_spec_json), mono = true, singleLine = false)
                    if (normalizedObjectJson(rawText) == null) Text(stringResource(R.string.cc_must_be_json_object), fontSize = 12.sp, color = colors.danger)
                } else when (type) {
                    ChannelType.TELEGRAM -> CodegTextField(chatId, { chatId = it }, label = stringResource(R.string.cc_chat_id), placeholder = "-100123456789", mono = true)
                    ChannelType.LARK -> {
                        CodegTextField(appId, { appId = it }, label = stringResource(R.string.cc_app_id), placeholder = "cli_xxxxx", mono = true)
                        CodegTextField(chatId, { chatId = it }, label = stringResource(R.string.cc_chat_id), placeholder = "oc_xxxxx", mono = true)
                    }
                    ChannelType.WEIXIN -> CodegTextField(baseUrl, { baseUrl = it }, label = stringResource(R.string.cc_base_url), placeholder = ChannelConfig.WEIXIN_DEFAULT_BASE_URL, mono = true)
                }
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(stringResource(R.string.cc_edit_raw_json), fontSize = 14.sp, color = colors.textPrimary, modifier = Modifier.weight(1f))
                    Switch(checked = rawMode, onCheckedChange = { on -> if (on && rawText.trim().isEmpty()) rawText = currentConfig().toJson(type); rawMode = on }, colors = SwitchDefaults.colors(checkedTrackColor = colors.accent, checkedThumbColor = colors.onAccent))
                }
                // Auth.
                if (type.secretLabel != null) {
                    Text(stringResource(R.string.server_section_auth), fontSize = 13.sp, fontWeight = FontWeight.SemiBold, color = colors.textSecondary)
                    SecretField(token, { token = it }, label = if (isEdit && hasToken) stringResource(R.string.cc_keep_current) else type.secretLabel!!)
                    if (isEdit && hasToken) Text(stringResource(R.string.cc_leave_blank_secret, type.secretLabel ?: ""), fontSize = 12.sp, color = colors.textTertiary)
                } else {
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Icon(Icons.Rounded.QrCode, null, tint = colors.textSecondary, modifier = Modifier.size(18.dp))
                        Text(stringResource(R.string.cc_wechat_connects_after_saving), fontSize = 12.sp, color = colors.textSecondary)
                    }
                }
                // Daily report.
                Text(stringResource(R.string.cc_daily_report_title), fontSize = 13.sp, fontWeight = FontWeight.SemiBold, color = colors.textSecondary)
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(stringResource(R.string.cc_enabled), fontSize = 14.sp, color = colors.textPrimary, modifier = Modifier.weight(1f))
                    Switch(checked = dailyEnabled, onCheckedChange = { dailyEnabled = it }, colors = SwitchDefaults.colors(checkedTrackColor = colors.accent, checkedThumbColor = colors.onAccent))
                }
                if (dailyEnabled) CodegTextField(dailyTime, { dailyTime = it }, label = stringResource(R.string.cc_time_hhmm), placeholder = "18:00", mono = true, keyboardType = KeyboardType.Number)
                error?.let { Text(it, fontSize = 13.sp, color = colors.danger) }
                Spacer(Modifier.height(24.dp))
            }
        }
    }
}

/** Partial update body: only changed fields (null = keep); time is tri-state. */
private fun buildUpdateBody(existing: ChatChannelInfo, name: String, enabled: Boolean, configJson: String, dailyEnabled: Boolean, timeString: String?): UpdateChatChannelBody {
    val configChanged = normalizedObjectJson(configJson) != normalizedObjectJson(existing.configJson)
    val timeEdit: FieldEdit = when {
        dailyEnabled && timeString != null && timeString != existing.dailyReportTime -> FieldEdit.Set(timeString)
        !dailyEnabled && existing.dailyReportTime != null -> FieldEdit.Clear
        else -> FieldEdit.Keep
    }
    return UpdateChatChannelBody(
        id = existing.id,
        name = if (name != existing.name) name else null,
        enabled = if (enabled != existing.enabled) enabled else null,
        configJson = if (configChanged) configJson else null,
        dailyReportEnabled = if (dailyEnabled != existing.dailyReportEnabled) dailyEnabled else null,
        dailyReportTime = timeEdit,
    )
}

// endregion
