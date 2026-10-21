package app.codeg.android.feature.projects

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.Undo
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.CheckCircle
import androidx.compose.material.icons.rounded.DeleteOutline
import androidx.compose.material.icons.rounded.Difference
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
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
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.codeg.android.R
import app.codeg.android.core.designsystem.component.EmptyState
import app.codeg.android.core.designsystem.component.InlineError
import app.codeg.android.core.designsystem.component.LoadingView
import app.codeg.android.core.designsystem.component.PrimaryButton
import app.codeg.android.core.designsystem.diff.DiffView
import app.codeg.android.core.designsystem.diff.UnifiedDiff
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.model.GitChange
import app.codeg.android.core.model.GitStatusEntry
import app.codeg.android.core.network.ApiError
import app.codeg.android.core.network.CodegClient

/**
 * Working-tree changes (git_status) with a per-file working diff, a commit
 * affordance, and long-press file actions (discard / stage / delete). The shared
 * [ProjectDetailViewModel] drives the operation status strip + reload signal.
 */
@OptIn(ExperimentalFoundationApi::class, ExperimentalMaterial3Api::class)
@Composable
fun FolderChangesView(
    viewModel: ProjectDetailViewModel,
    client: CodegClient,
    rootPath: String,
    onCommit: () -> Unit,
) {
    val ui by viewModel.ui.collectAsStateWithLifecycle()
    var diffFile by remember { mutableStateOf<String?>(null) }
    BackHandler(enabled = diffFile != null) { diffFile = null }

    val df = diffFile
    if (df != null) {
        WorkingDiff(client, rootPath, df)
        return
    }

    var result by remember(rootPath) { mutableStateOf<Result<List<GitStatusEntry>>?>(null) }
    // Reload when the working tree mutates (commit / discard / stage / delete / pull).
    LaunchedEffect(rootPath, ui.reloadToken) { result = runCatching { client.gitStatus(rootPath) } }

    var actionEntry by remember { mutableStateOf<GitStatusEntry?>(null) }
    var confirm by remember { mutableStateOf<Pair<GitStatusEntry, Boolean>?>(null) } // entry to (isDelete)

    Column(Modifier.fillMaxSize()) {
        GitStatusStrip(
            busy = ui.gitBusy,
            busyTitle = ui.gitBusyTitle,
            banner = ui.gitBanner,
            onDismissBanner = viewModel::dismissBanner,
            modifier = Modifier.padding(horizontal = 12.dp, vertical = 4.dp),
        )

        val r = result
        Box(Modifier.weight(1f).fillMaxSize()) {
            when {
                r == null -> Centered { LoadingView(stringResource(R.string.common_loading)) }
                r.isFailure -> {
                    val e = r.exceptionOrNull()
                    if (e is ApiError.Server && e.code == "not_a_git_repository") {
                        Centered { EmptyState(Icons.Rounded.Difference, stringResource(R.string.commits_not_a_git_repo), stringResource(R.string.commits_not_a_git_repo_message)) }
                    } else {
                        Centered { InlineError(Icons.Rounded.Difference, stringResource(R.string.changes_load_failed), e?.message ?: stringResource(R.string.files_error), onRetry = { result = null }) }
                    }
                }
                else -> {
                    val entries = r.getOrDefault(emptyList())
                    if (entries.isEmpty()) {
                        Centered { EmptyState(Icons.Rounded.CheckCircle, stringResource(R.string.changes_clean_title), stringResource(R.string.changes_clean_message)) }
                    } else {
                        LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(horizontal = 12.dp, vertical = 4.dp)) {
                            items(entries, key = { it.file }) { entry ->
                                ChangeRow(entry, onClick = { diffFile = entry.path }, onLongClick = { actionEntry = entry })
                            }
                        }
                    }
                }
            }
        }

        val entries = result?.getOrNull().orEmpty()
        if (entries.isNotEmpty()) {
            PrimaryButton(
                text = stringResource(R.string.changes_commit_button),
                onClick = onCommit,
                icon = Icons.Rounded.CheckCircle,
                enabled = !ui.gitBusy,
                modifier = Modifier.padding(horizontal = 12.dp, vertical = 8.dp),
            )
        }
    }

    // Long-press file actions.
    val target = actionEntry
    if (target != null) {
        ModalBottomSheet(onDismissRequest = { actionEntry = null }, sheetState = rememberModalBottomSheetState()) {
            FileActions(
                entry = target,
                onViewDiff = { actionEntry = null; diffFile = target.path },
                onStage = { actionEntry = null; viewModel.stage(target.path, target.path.substringAfterLast('/')) },
                onDiscard = { actionEntry = null; confirm = target to false },
                onDelete = { actionEntry = null; confirm = target to true },
            )
        }
    }

    // Destructive-action confirmation (discard / delete lose working-tree changes).
    confirm?.let { (entry, isDelete) ->
        val name = entry.path.substringAfterLast('/')
        AlertDialog(
            onDismissRequest = { confirm = null },
            title = { Text(stringResource(if (isDelete) R.string.changes_delete_title else R.string.changes_discard_title)) },
            text = { Text(stringResource(if (isDelete) R.string.changes_delete_message else R.string.changes_discard_message, name)) },
            confirmButton = {
                TextButton(onClick = {
                    confirm = null
                    if (isDelete) viewModel.deleteUntracked(entry.path, name) else viewModel.discard(entry.path, name)
                }) { Text(stringResource(if (isDelete) R.string.changes_action_delete else R.string.changes_action_discard), color = CodegTheme.colors.danger) }
            },
            dismissButton = { TextButton(onClick = { confirm = null }) { Text(stringResource(R.string.common_cancel)) } },
        )
    }
}

@Composable
private fun FileActions(
    entry: GitStatusEntry,
    onViewDiff: () -> Unit,
    onStage: () -> Unit,
    onDiscard: () -> Unit,
    onDelete: () -> Unit,
) {
    val colors = CodegTheme.colors
    Column(Modifier.fillMaxWidth().padding(bottom = 12.dp)) {
        Row(Modifier.fillMaxWidth().padding(horizontal = 20.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            GitChangeBadge(entry.change)
            Text(entry.path.substringAfterLast('/'), fontWeight = FontWeight.SemiBold, color = colors.textPrimary, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        ActionRow(Icons.Rounded.Difference, stringResource(R.string.changes_action_view_diff), colors.textPrimary, onViewDiff)
        ActionRow(Icons.Rounded.Add, stringResource(R.string.changes_action_stage), colors.textPrimary, onStage)
        if (entry.change == GitChange.UNTRACKED) {
            ActionRow(Icons.Rounded.DeleteOutline, stringResource(R.string.changes_action_delete), colors.danger, onDelete)
        } else {
            ActionRow(Icons.AutoMirrored.Rounded.Undo, stringResource(R.string.changes_action_discard), colors.danger, onDiscard)
        }
    }
}

@Composable
private fun ActionRow(icon: androidx.compose.ui.graphics.vector.ImageVector, label: String, tint: Color, onClick: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().clickable { onClick() }.padding(horizontal = 20.dp, vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        Icon(icon, contentDescription = null, tint = tint, modifier = Modifier.size(20.dp))
        Text(label, color = tint, fontSize = 15.sp)
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun ChangeRow(entry: GitStatusEntry, onClick: () -> Unit, onLongClick: () -> Unit) {
    val colors = CodegTheme.colors
    val sub = entry.renamedFrom?.let { stringResource(R.string.changes_renamed_from, it) } ?: entry.path.substringBeforeLast('/', "")
    ListItem(
        headlineContent = { Text(entry.path.substringAfterLast('/'), fontSize = 14.sp, maxLines = 1, overflow = TextOverflow.Ellipsis) },
        supportingContent = sub.takeIf { it.isNotEmpty() }?.let {
            { Text(it, fontFamily = FontFamily.Monospace, fontSize = 10.sp, maxLines = 1, overflow = TextOverflow.Ellipsis) }
        },
        leadingContent = { GitChangeBadge(entry.change) },
        colors = ListItemDefaults.colors(containerColor = Color.Transparent, headlineColor = colors.textPrimary, supportingColor = colors.textTertiary),
        modifier = Modifier.clip(RoundedCornerShape(10.dp)).combinedClickable(onClick = onClick, onLongClick = onLongClick),
    )
}

@Composable
private fun WorkingDiff(client: CodegClient, rootPath: String, file: String) {
    var result by remember(file) { mutableStateOf<Result<String>?>(null) }
    LaunchedEffect(file) { result = runCatching { client.gitDiff(rootPath, file) } }
    val r = result
    when {
        r == null -> Centered { LoadingView(stringResource(R.string.common_loading)) }
        r.isFailure -> Centered { InlineError(Icons.Rounded.Difference, stringResource(R.string.changes_diff_load_failed), r.exceptionOrNull()?.message ?: stringResource(R.string.files_error), onRetry = { result = null }) }
        else -> {
            val files = UnifiedDiff.parse(r.getOrDefault(""))
            if (files == null) Centered { EmptyState(Icons.Rounded.Difference, stringResource(R.string.changes_no_diff_title), stringResource(R.string.changes_no_diff_message)) }
            else LazyColumn(Modifier.fillMaxSize().padding(horizontal = 12.dp, vertical = 8.dp)) { item { DiffView(files) } }
        }
    }
}

@Composable
private fun Centered(content: @Composable () -> Unit) {
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { content() }
}
