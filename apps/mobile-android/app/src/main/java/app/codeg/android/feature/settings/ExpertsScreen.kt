package app.codeg.android.feature.settings

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
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
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material.icons.rounded.AutoAwesome
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.School
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
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
import app.codeg.android.core.designsystem.component.AgentIcon
import app.codeg.android.core.designsystem.component.EmptyState
import app.codeg.android.core.designsystem.component.InlineError
import app.codeg.android.core.designsystem.component.LoadingView
import app.codeg.android.core.designsystem.markdown.MarkdownContent
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.model.AgentType
import app.codeg.android.core.model.ExpertInstallStatus
import app.codeg.android.core.model.ExpertLinkState
import app.codeg.android.core.model.ExpertListItem

@Composable
fun ExpertsContent(viewModel: ExpertsViewModel = hiltViewModel()) {
    val ui by viewModel.ui.collectAsStateWithLifecycle()
    val colors = CodegTheme.colors

    Box(Modifier.fillMaxSize()) {
        if (ui.experts.isEmpty()) {
            Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                when (ui.phase) {
                    ExpertsPhase.LOADING -> LoadingView(stringResource(R.string.experts_loading_experts))
                    ExpertsPhase.FAILED -> InlineError(Icons.Rounded.School, stringResource(R.string.experts_couldnt_load), ui.error ?: stringResource(R.string.experts_unknown_error), onRetry = { viewModel.load() })
                    ExpertsPhase.LOADED -> EmptyState(Icons.Rounded.School, stringResource(R.string.experts_no_experts), stringResource(R.string.experts_no_experts_message))
                }
            }
        } else {
            LazyColumn(
                Modifier.fillMaxSize(),
                contentPadding = androidx.compose.foundation.layout.PaddingValues(horizontal = 12.dp, vertical = 8.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                if (ui.refreshError != null) {
                    item("err") { ExpertsRefreshBanner(ui.refreshError!!, onRetry = { viewModel.load() }, onDismiss = { viewModel.dismissRefreshError() }) }
                }
                for ((category, label, rows) in ui.grouped) {
                    item("h-$category") {
                        Text(
                            "${label.uppercase()} · ${rows.size}",
                            fontSize = 11.sp, fontWeight = FontWeight.SemiBold, color = colors.textTertiary,
                            modifier = Modifier.padding(start = 4.dp, top = 6.dp),
                        )
                    }
                    items(rows, key = { it.id }) { expert -> ExpertRow(expert) { viewModel.openDetail(expert) } }
                }
            }
        }
    }

    ui.detail?.let { detail ->
        ExpertDetailDialog(
            detail = detail,
            agents = ui.agents,
            onToggle = { agent, on -> viewModel.toggleLink(agent, on) },
            onDismiss = { viewModel.closeDetail() },
            onDismissError = { viewModel.dismissDetailError() },
        )
    }
}

@Composable
private fun ExpertRow(expert: ExpertListItem, onClick: () -> Unit) {
    val colors = CodegTheme.colors
    Row(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(14.dp)).background(colors.bgElevated.copy(alpha = 0.5f)).clickable { onClick() }.padding(13.dp),
        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(13.dp),
    ) {
        ExpertIconTile(40.dp)
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
            Text(expert.metadata.title(), fontSize = 15.sp, fontWeight = FontWeight.SemiBold, color = colors.textPrimary, maxLines = 1, overflow = TextOverflow.Ellipsis)
            expert.metadata.desc().takeIf { it.isNotEmpty() }?.let {
                Text(it, fontSize = 13.sp, color = colors.textSecondary, maxLines = 2, overflow = TextOverflow.Ellipsis)
            }
        }
        Icon(Icons.AutoMirrored.Rounded.KeyboardArrowRight, null, tint = colors.textTertiary, modifier = Modifier.size(16.dp))
    }
}

/** Accent-tinted rounded tile with a generic expert glyph (lucide→symbol mapping deferred). */
@Composable
private fun ExpertIconTile(size: androidx.compose.ui.unit.Dp) {
    val colors = CodegTheme.colors
    Box(
        Modifier.size(size).clip(RoundedCornerShape(size * 0.27f)).background(colors.accent.copy(alpha = 0.16f)),
        contentAlignment = Alignment.Center,
    ) {
        Icon(Icons.Rounded.AutoAwesome, null, tint = colors.accent, modifier = Modifier.size(size * 0.44f))
    }
}

@Composable
private fun ExpertsRefreshBanner(message: String, onRetry: () -> Unit, onDismiss: () -> Unit) {
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

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ExpertDetailDialog(
    detail: DetailState,
    agents: List<AgentType>,
    onToggle: (AgentType, Boolean) -> Unit,
    onDismiss: () -> Unit,
    onDismissError: () -> Unit,
) {
    val colors = CodegTheme.colors
    val expert = detail.expert

    Dialog(onDismissRequest = onDismiss, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        Column(Modifier.fillMaxSize().background(colors.bg).windowInsetsPadding(WindowInsets.safeDrawing)) {
            TopAppBar(
                title = { Text(expert.metadata.title(), maxLines = 1, overflow = TextOverflow.Ellipsis) },
                navigationIcon = {
                    IconButton(onClick = onDismiss) { Icon(Icons.Rounded.Close, contentDescription = stringResource(R.string.experts_close), tint = colors.textPrimary) }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent, titleContentColor = colors.textPrimary),
            )
            Column(Modifier.weight(1f).verticalScroll(rememberScrollState()).padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(20.dp)) {
                // Header.
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                    ExpertIconTile(56.dp)
                    Column(verticalArrangement = Arrangement.spacedBy(7.dp)) {
                        Text(expert.metadata.title(), fontSize = 20.sp, fontWeight = FontWeight.SemiBold, color = colors.textPrimary)
                        Text(
                            ExpertCategory.label(expert.metadata.category).uppercase(),
                            fontSize = 10.sp, fontWeight = FontWeight.Bold, color = colors.accent,
                            modifier = Modifier.clip(RoundedCornerShape(50)).background(colors.accent.copy(alpha = 0.16f)).padding(horizontal = 7.dp, vertical = 3.dp),
                        )
                    }
                }
                expert.metadata.desc().takeIf { it.isNotEmpty() }?.let {
                    Text(it, fontSize = 14.sp, color = colors.textSecondary)
                }
                Text("# ${expert.metadata.id}", fontSize = 12.sp, fontFamily = FontFamily.Monospace, color = colors.textTertiary, maxLines = 1, overflow = TextOverflow.Ellipsis)

                // Per-agent link matrix.
                Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    Text(stringResource(R.string.experts_enable_for), fontSize = 11.sp, fontWeight = FontWeight.SemiBold, color = colors.textTertiary)
                    Column(Modifier.fillMaxWidth().clip(RoundedCornerShape(14.dp)).background(colors.bgElevated.copy(alpha = 0.5f))) {
                        when {
                            agents.isEmpty() -> Text(stringResource(R.string.experts_no_agents_available), fontSize = 13.sp, color = colors.textTertiary, modifier = Modifier.padding(16.dp))
                            detail.loading -> Row(Modifier.padding(16.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                                CircularProgressIndicator(color = colors.accent, modifier = Modifier.size(18.dp))
                                Text(stringResource(R.string.experts_loading_status), fontSize = 13.sp, color = colors.textSecondary)
                            }
                            else -> agents.forEachIndexed { index, agent ->
                                if (index > 0) HorizontalDivider(modifier = Modifier.padding(start = 62.dp), thickness = Dp.Hairline, color = colors.textPrimary.copy(alpha = 0.06f))
                                AgentLinkRow(agent, detail.statusByAgent[agent], detail.toggling.contains(agent)) { on -> onToggle(agent, on) }
                            }
                        }
                    }
                    Text(stringResource(R.string.experts_link_hint), fontSize = 11.sp, color = colors.textTertiary)
                }

                // Markdown preview.
                if (detail.content.isNotBlank()) {
                    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                        Text(stringResource(R.string.experts_preview), fontSize = 11.sp, fontWeight = FontWeight.SemiBold, color = colors.textTertiary)
                        Box(Modifier.fillMaxWidth().clip(RoundedCornerShape(14.dp)).background(colors.bgElevated.copy(alpha = 0.5f)).padding(14.dp)) {
                            MarkdownContent(stripFrontmatter(detail.content))
                        }
                    }
                }
                detail.error?.let {
                    Row(Modifier.fillMaxWidth().clip(RoundedCornerShape(10.dp)).background(colors.danger.copy(alpha = 0.12f)).padding(12.dp), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                        Text(it, fontSize = 12.sp, color = colors.danger, modifier = Modifier.weight(1f))
                        Text(stringResource(R.string.common_dismiss), fontSize = 12.sp, color = colors.accent, modifier = Modifier.clickable { onDismissError() })
                    }
                }
                Spacer(Modifier.height(24.dp))
            }
        }
    }
}

@Composable
private fun AgentLinkRow(agent: AgentType, status: ExpertInstallStatus?, toggling: Boolean, onToggle: (Boolean) -> Unit) {
    val colors = CodegTheme.colors
    val state = status?.state
    val isLinked = state?.isLinked ?: false
    val blocked = state == ExpertLinkState.LINKED_ELSEWHERE || state == ExpertLinkState.BLOCKED_BY_REAL_DIRECTORY || state == ExpertLinkState.BROKEN
    val caption = linkCaption(state, status?.copyMode ?: false)

    Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 12.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        Box(Modifier.size(34.dp).clip(RoundedCornerShape(9.dp)).background(colors.textPrimary.copy(alpha = 0.06f)), contentAlignment = Alignment.Center) {
            AgentIcon(agent, size = 20.dp)
        }
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
            Text(agent.displayName, fontSize = 15.sp, color = colors.textPrimary)
            caption?.let { (text, color) -> Text(text, fontSize = 11.sp, color = color) }
        }
        if (toggling) CircularProgressIndicator(color = colors.accent, modifier = Modifier.size(18.dp))
        Switch(
            checked = isLinked,
            onCheckedChange = onToggle,
            enabled = !toggling && !(blocked && !isLinked),
            colors = SwitchDefaults.colors(checkedTrackColor = colors.accent, checkedThumbColor = colors.onAccent),
        )
    }
}

/** Sub-label under an agent row: positive accent when linked, the copy-mode note, or the block/broken reason. */
@Composable
private fun linkCaption(state: ExpertLinkState?, copyMode: Boolean): Pair<String, Color>? {
    val colors = CodegTheme.colors
    return when (state) {
        ExpertLinkState.LINKED_TO_CODEG -> if (copyMode) stringResource(R.string.experts_link_copied) to colors.textTertiary else stringResource(R.string.experts_link_linked) to colors.accent
        ExpertLinkState.LINKED_ELSEWHERE -> stringResource(R.string.experts_link_elsewhere) to colors.textTertiary
        ExpertLinkState.BLOCKED_BY_REAL_DIRECTORY -> stringResource(R.string.experts_link_blocked) to colors.textTertiary
        ExpertLinkState.BROKEN -> stringResource(R.string.experts_link_broken) to colors.danger
        ExpertLinkState.NOT_LINKED, null -> null
    }
}

/** Drop a leading YAML frontmatter block (`---\n…\n---`) before rendering (matches the web's `stripFrontmatter`). */
private fun stripFrontmatter(content: String): String {
    val lines = content.split("\n")
    if (lines.firstOrNull()?.trim() != "---") return content
    val end = lines.drop(1).indexOfFirst { it.trim() == "---" }
    if (end < 0) return content
    return lines.drop(end + 2).joinToString("\n").trim()
}
