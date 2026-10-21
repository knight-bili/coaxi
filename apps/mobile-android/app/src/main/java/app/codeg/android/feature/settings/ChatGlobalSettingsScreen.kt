package app.codeg.android.feature.settings

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.Delete
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledIconButton
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
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
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.model.ChatEventCatalog
import app.codeg.android.core.model.ChatLanguageCatalog

@OptIn(ExperimentalLayoutApi::class, ExperimentalMaterial3Api::class)
@Composable
fun ChatGlobalSettingsDialog(onDismiss: () -> Unit, viewModel: ChatGlobalSettingsViewModel = hiltViewModel()) {
    val ui by viewModel.ui.collectAsStateWithLifecycle()
    val colors = CodegTheme.colors
    var newWebhook by remember { mutableStateOf("") }
    var prefixField by remember(ui.commandPrefix) { mutableStateOf(ui.commandPrefix) }

    Dialog(onDismissRequest = onDismiss, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        Column(Modifier.fillMaxSize().background(colors.bg).windowInsetsPadding(WindowInsets.safeDrawing)) {
            TopAppBar(
                title = { Text(stringResource(R.string.chatglobal_title), maxLines = 1, overflow = TextOverflow.Ellipsis) },
                navigationIcon = {
                    IconButton(onClick = onDismiss) { Icon(Icons.Rounded.Close, contentDescription = stringResource(R.string.chatglobal_close_cd), tint = colors.textPrimary) }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent, titleContentColor = colors.textPrimary),
            )
            if (ui.loading) {
                Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { CircularProgressIndicator(color = colors.accent) }
            } else {
                Column(Modifier.weight(1f).verticalScroll(rememberScrollState()).padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(20.dp)) {
                    Spacer(Modifier.height(2.dp))

                    // Command prefix.
                    GlobalSection(stringResource(R.string.chatglobal_command_prefix_title), stringResource(R.string.chatglobal_command_prefix_subtitle)) {
                        CodegTextField(prefixField, { prefixField = it }, label = stringResource(R.string.chatglobal_prefix_label), mono = true)
                        FilledTonalButton(
                            onClick = { viewModel.setCommandPrefix(prefixField) },
                            modifier = Modifier.padding(top = 8.dp),
                            shape = RoundedCornerShape(8.dp),
                            colors = ButtonDefaults.filledTonalButtonColors(containerColor = colors.accent.copy(alpha = 0.14f), contentColor = colors.accent),
                        ) {
                            Text(stringResource(R.string.chatglobal_save_prefix))
                        }
                    }

                    // Language.
                    GlobalSection(stringResource(R.string.chatglobal_reply_language_title), stringResource(R.string.chatglobal_reply_language_subtitle)) {
                        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                            for ((code, label) in ChatLanguageCatalog.options) {
                                CodegFilterChip(label = label, selected = ui.language.lowercase() == code, onClick = { viewModel.setLanguage(code) })
                            }
                        }
                    }

                    // Event filter.
                    GlobalSection(stringResource(R.string.chatglobal_forwarded_events_title), stringResource(R.string.chatglobal_forwarded_events_subtitle)) {
                        Column(Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp)).background(colors.bgElevated.copy(alpha = 0.5f))) {
                            ChatEventCatalog.all.forEachIndexed { index, item ->
                                if (index > 0) HorizontalDivider(thickness = Dp.Hairline, color = colors.textPrimary.copy(alpha = 0.06f))
                                Row(Modifier.fillMaxWidth().padding(horizontal = 14.dp, vertical = 11.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                    Column(Modifier.weight(1f)) {
                                        Text(item.label, fontSize = 14.sp, color = colors.textPrimary)
                                        item.note?.let { Text(it, fontSize = 11.sp, color = colors.textTertiary) }
                                    }
                                    Switch(checked = ui.enabledEvents.contains(item.id), onCheckedChange = { viewModel.toggleEvent(item.id, it) }, colors = SwitchDefaults.colors(checkedTrackColor = colors.accent, checkedThumbColor = colors.onAccent))
                                }
                            }
                        }
                    }

                    // Webhooks.
                    GlobalSection(stringResource(R.string.chatglobal_webhooks_title), stringResource(R.string.chatglobal_webhooks_subtitle)) {
                        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                            ui.webhooks.forEachIndexed { index, hook ->
                                Row(Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp)).background(colors.bgElevated.copy(alpha = 0.5f)).padding(start = 12.dp, top = 4.dp, bottom = 4.dp, end = 4.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                    Text(hook.url, fontSize = 12.sp, fontFamily = FontFamily.Monospace, color = colors.textPrimary, modifier = Modifier.weight(1f), maxLines = 1, overflow = TextOverflow.Ellipsis)
                                    Switch(checked = hook.enabled, onCheckedChange = { viewModel.setWebhookEnabled(index, it) }, colors = SwitchDefaults.colors(checkedTrackColor = colors.accent, checkedThumbColor = colors.onAccent))
                                    IconButton(onClick = { viewModel.removeWebhook(index) }) {
                                        Icon(Icons.Rounded.Delete, stringResource(R.string.common_remove), tint = colors.danger, modifier = Modifier.size(18.dp))
                                    }
                                }
                            }
                            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                Box(Modifier.weight(1f)) { CodegTextField(newWebhook, { newWebhook = it }, label = "https://…", mono = true) }
                                FilledIconButton(
                                    onClick = { viewModel.addWebhook(newWebhook); newWebhook = "" },
                                    enabled = newWebhook.isNotBlank(),
                                    modifier = Modifier.size(48.dp),
                                    shape = RoundedCornerShape(12.dp),
                                    colors = IconButtonDefaults.filledIconButtonColors(containerColor = colors.accent.copy(alpha = 0.16f), contentColor = colors.accent),
                                ) {
                                    Icon(Icons.Rounded.Add, stringResource(R.string.chatglobal_add_webhook_cd))
                                }
                            }
                        }
                    }

                    ui.error?.let { Text(it, fontSize = 13.sp, color = colors.danger) }
                    Spacer(Modifier.height(24.dp))
                }
            }
        }
    }
}

@Composable
private fun GlobalSection(title: String, subtitle: String, content: @Composable () -> Unit) {
    val colors = CodegTheme.colors
    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Text(title, fontSize = 11.sp, fontWeight = FontWeight.SemiBold, color = colors.textTertiary, modifier = Modifier.padding(start = 4.dp))
        content()
        Text(subtitle, fontSize = 11.sp, color = colors.textTertiary, modifier = Modifier.padding(start = 4.dp))
    }
}
