package app.codeg.android.feature.projects

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.CheckCircle
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.RadioButtonUnchecked
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import app.codeg.android.R
import app.codeg.android.core.designsystem.component.CodegTextField
import app.codeg.android.core.designsystem.component.EmptyState
import app.codeg.android.core.designsystem.component.InlineError
import app.codeg.android.core.designsystem.component.LoadingView
import app.codeg.android.core.designsystem.component.PrimaryButton
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.model.GitChange
import app.codeg.android.core.model.GitStatusEntry
import app.codeg.android.core.network.CodegClient
import app.codeg.android.core.network.displayMessage
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch

/**
 * The commit composer, presented from the Changes tab. Lists the working-tree
 * changes with per-file selection (tracked auto-selected, untracked unselected —
 * web/iOS parity), takes a message, and commits the selected files (optionally
 * pushing afterward). The server stages the chosen files, so untracked paths can be
 * committed directly. Port of iOS `CommitSheet`.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun CommitSheet(
    viewModel: ProjectDetailViewModel,
    client: CodegClient,
    rootPath: String,
    onDismiss: () -> Unit,
    onCommitted: (push: Boolean) -> Unit,
) {
    val colors = CodegTheme.colors
    val scope = rememberCoroutineScope()

    var entries by remember { mutableStateOf<List<GitStatusEntry>?>(null) }
    var selected by remember { mutableStateOf(setOf<String>()) }
    var message by remember { mutableStateOf("") }
    var loadError by remember { mutableStateOf<String?>(null) }
    var committing by remember { mutableStateOf(false) }
    var commitError by remember { mutableStateOf<String?>(null) }

    suspend fun load() {
        loadError = null
        runCatching { client.gitStatus(rootPath, showAllUntracked = true) }
            .onSuccess { result ->
                entries = result
                // Tracked changes start selected; untracked start unselected.
                selected = result.filter { it.change != GitChange.UNTRACKED }.map { it.path }.toSet()
            }
            .onFailure { loadError = it.displayMessage() }
    }

    androidx.compose.runtime.LaunchedEffect(rootPath) { load() }

    val all = entries.orEmpty()
    val trimmedMessage = message.trim()
    val canCommit = trimmedMessage.isNotEmpty() && selected.isNotEmpty() && !committing

    fun runCommit(push: Boolean) {
        if (!canCommit) return
        committing = true
        commitError = null
        scope.launch {
            try {
                viewModel.gitCommit(trimmedMessage, selected.toList())
                committing = false
                onCommitted(push)
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                committing = false
                commitError = e.displayMessage()
            }
        }
    }

    Dialog(onDismissRequest = { if (!committing) onDismiss() }, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        Column(Modifier.fillMaxSize().background(colors.bg).windowInsetsPadding(WindowInsets.safeDrawing)) {
            TopAppBar(
                title = { Text(stringResource(R.string.commit_title)) },
                navigationIcon = {
                    IconButton(onClick = onDismiss, enabled = !committing) {
                        Icon(Icons.Rounded.Close, contentDescription = stringResource(R.string.common_cancel), tint = colors.textPrimary)
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent, titleContentColor = colors.textPrimary),
            )

            when {
                entries == null && loadError == null ->
                    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { LoadingView(stringResource(R.string.common_loading)) }
                loadError != null ->
                    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                        InlineError(Icons.Rounded.CheckCircle, stringResource(R.string.commit_load_failed), loadError!!, onRetry = { entries = null; scope.launch { load() } })
                    }
                all.isEmpty() ->
                    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                        EmptyState(Icons.Rounded.CheckCircle, stringResource(R.string.commit_nothing_title), stringResource(R.string.commit_nothing_message))
                    }
                else -> {
                    val tracked = all.filter { it.change != GitChange.UNTRACKED }
                    val untracked = all.filter { it.change == GitChange.UNTRACKED }
                    Column(
                        Modifier.weight(1f).fillMaxWidth().verticalScroll(rememberScrollState()).padding(horizontal = 16.dp),
                        verticalArrangement = Arrangement.spacedBy(16.dp),
                    ) {
                        Box(Modifier.padding(top = 8.dp)) {
                            CodegTextField(
                                value = message,
                                onValueChange = { message = it },
                                label = stringResource(R.string.commit_message_label),
                                placeholder = stringResource(R.string.commit_message_placeholder),
                                singleLine = false,
                            )
                        }
                        SelectionHeader(
                            selectedCount = selected.size,
                            total = all.size,
                            allSelected = selected.size == all.size,
                            enabled = !committing,
                            onToggleAll = {
                                selected = if (selected.size == all.size) emptySet() else all.map { it.path }.toSet()
                            },
                        )
                        if (tracked.isNotEmpty()) {
                            FileSection(stringResource(R.string.commit_section_changes), tracked, selected, enabled = !committing) { path ->
                                selected = if (path in selected) selected - path else selected + path
                            }
                        }
                        if (untracked.isNotEmpty()) {
                            FileSection(stringResource(R.string.commit_section_untracked), untracked, selected, enabled = !committing) { path ->
                                selected = if (path in selected) selected - path else selected + path
                            }
                        }
                        commitError?.let {
                            Text(
                                it,
                                fontSize = 13.sp,
                                color = colors.danger,
                                modifier = Modifier.fillMaxWidth().clip(RoundedCornerShape(10.dp)).background(colors.danger.copy(alpha = 0.12f)).padding(12.dp),
                            )
                        }
                    }

                    Column(Modifier.fillMaxWidth().padding(16.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                        PrimaryButton(
                            text = pluralStringResource(R.plurals.commit_button, selected.size, selected.size),
                            onClick = { runCommit(push = false) },
                            enabled = canCommit,
                            loading = committing,
                        )
                        TextButton(
                            onClick = { runCommit(push = true) },
                            enabled = canCommit,
                            modifier = Modifier.fillMaxWidth(),
                        ) {
                            Text(stringResource(R.string.commit_and_push), color = if (canCommit) colors.accent else colors.textTertiary, fontWeight = FontWeight.Medium)
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun SelectionHeader(
    selectedCount: Int,
    total: Int,
    allSelected: Boolean,
    enabled: Boolean,
    onToggleAll: () -> Unit,
) {
    val colors = CodegTheme.colors
    Row(verticalAlignment = Alignment.CenterVertically) {
        Text(
            stringResource(R.string.commit_selected, selectedCount, total),
            fontSize = 12.sp,
            fontWeight = FontWeight.Medium,
            color = colors.textSecondary,
        )
        Box(Modifier.weight(1f))
        TextButton(onClick = onToggleAll, enabled = enabled) {
            Text(
                stringResource(if (allSelected) R.string.commit_deselect_all else R.string.commit_select_all),
                fontSize = 12.sp,
                fontWeight = FontWeight.SemiBold,
                color = colors.accent,
            )
        }
    }
}

@Composable
private fun FileSection(
    title: String,
    entries: List<GitStatusEntry>,
    selected: Set<String>,
    enabled: Boolean,
    onToggle: (String) -> Unit,
) {
    val colors = CodegTheme.colors
    Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
        Text(title.uppercase(), fontSize = 11.sp, fontWeight = FontWeight.SemiBold, color = colors.textTertiary, modifier = Modifier.padding(start = 4.dp, bottom = 4.dp))
        for (entry in entries) {
            val isSelected = entry.path in selected
            Row(
                Modifier.fillMaxWidth().clip(RoundedCornerShape(10.dp)).clickable(enabled = enabled) { onToggle(entry.path) }.padding(vertical = 8.dp, horizontal = 4.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(11.dp),
            ) {
                Icon(
                    if (isSelected) Icons.Rounded.CheckCircle else Icons.Rounded.RadioButtonUnchecked,
                    contentDescription = null,
                    tint = if (isSelected) colors.accent else colors.textTertiary,
                    modifier = Modifier.size(20.dp),
                )
                GitChangeBadge(entry.change)
                Column(Modifier.weight(1f)) {
                    Text(entry.path.substringAfterLast('/'), fontSize = 14.sp, color = colors.textPrimary, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    val dir = entry.path.substringBeforeLast('/', "")
                    if (dir.isNotEmpty()) {
                        Text(dir, fontFamily = FontFamily.Monospace, fontSize = 10.sp, color = colors.textTertiary, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    }
                }
            }
        }
    }
}
