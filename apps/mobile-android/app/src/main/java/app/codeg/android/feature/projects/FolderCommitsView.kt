package app.codeg.android.feature.projects

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
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
import androidx.compose.material.icons.automirrored.rounded.CallSplit
import androidx.compose.material.icons.rounded.Cloud
import androidx.compose.material.icons.rounded.CloudDownload
import androidx.compose.material.icons.rounded.CloudOff
import androidx.compose.material.icons.rounded.CloudUpload
import androidx.compose.material.icons.rounded.History
import androidx.compose.material.icons.rounded.Sync
import androidx.compose.material3.Icon
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.Text
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
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.codeg.android.R
import app.codeg.android.core.common.RelativeTime
import app.codeg.android.core.designsystem.component.CopyButton
import app.codeg.android.core.designsystem.component.EmptyState
import app.codeg.android.core.designsystem.component.InlineError
import app.codeg.android.core.designsystem.component.LoadingView
import app.codeg.android.core.designsystem.diff.DiffView
import app.codeg.android.core.designsystem.diff.UnifiedDiff
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.model.GitLogEntry
import app.codeg.android.core.model.GitLogResult
import app.codeg.android.core.model.GitPushInfo
import app.codeg.android.core.network.ApiError
import app.codeg.android.core.network.CodegClient
import java.time.OffsetDateTime

/**
 * A folder's commit history (git_log) plus a sync header (branch → remote, with
 * pull / push / fetch). The shared [ProjectDetailViewModel] drives the operation
 * status strip + credential retry + reload signal.
 */
@Composable
fun FolderCommitsView(viewModel: ProjectDetailViewModel, client: CodegClient, rootPath: String) {
    val ui by viewModel.ui.collectAsStateWithLifecycle()
    var commit by remember { mutableStateOf<GitLogEntry?>(null) }
    BackHandler(enabled = commit != null) { commit = null }

    val sel = commit
    if (sel != null) {
        CommitDetail(client, rootPath, sel)
        return
    }

    var result by remember(rootPath) { mutableStateOf<Result<GitLogResult>?>(null) }
    var pushInfo by remember(rootPath) { mutableStateOf<GitPushInfo?>(null) }
    LaunchedEffect(rootPath, ui.reloadToken) {
        result = runCatching { client.gitLog(rootPath, limit = 50) }
        pushInfo = runCatching { client.gitPushInfo(rootPath) }.getOrNull()
    }

    val r = result
    val isRepo = !(r?.exceptionOrNull().let { it is ApiError.Server && it.code == "not_a_git_repository" })

    Column(Modifier.fillMaxSize()) {
        GitStatusStrip(
            busy = ui.gitBusy,
            busyTitle = ui.gitBusyTitle,
            banner = ui.gitBanner,
            onDismissBanner = viewModel::dismissBanner,
            modifier = Modifier.padding(horizontal = 12.dp, vertical = 4.dp),
        )

        if (r != null && r.isSuccess && isRepo) {
            val log = r.getOrNull() ?: GitLogResult()
            SyncHeader(
                branch = pushInfo?.branch?.takeIf { it.isNotEmpty() } ?: ui.branch,
                trackingRemote = pushInfo?.trackingRemote,
                hasRemote = pushInfo?.uniqueRemotes?.isNotEmpty() ?: true,
                unpushed = log.entries.count { it.pushed == false },
                busy = ui.gitBusy,
                onPull = viewModel::pull,
                onPush = viewModel::push,
                onFetch = viewModel::fetch,
            )
        }

        Box(Modifier.weight(1f).fillMaxSize()) {
            when {
                r == null -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { LoadingView(stringResource(R.string.common_loading)) }
                r.isFailure -> {
                    val e = r.exceptionOrNull()
                    if (!isRepo) {
                        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { EmptyState(Icons.Rounded.History, stringResource(R.string.commits_not_a_git_repo), stringResource(R.string.commits_not_a_git_repo_message)) }
                    } else {
                        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { InlineError(Icons.Rounded.History, stringResource(R.string.commits_load_failed), e?.message ?: stringResource(R.string.commits_error), onRetry = { result = null }) }
                    }
                }
                else -> {
                    val log = r.getOrNull() ?: GitLogResult()
                    if (log.entries.isEmpty()) {
                        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { EmptyState(Icons.Rounded.History, stringResource(R.string.commits_empty_title), stringResource(R.string.commits_empty_message)) }
                    } else {
                        LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(horizontal = 12.dp, vertical = 4.dp)) {
                            items(log.entries, key = { it.fullHash }) { entry -> CommitRow(entry) { commit = entry } }
                            if (log.entries.size >= 50) item("more") {
                                Text(stringResource(R.string.commits_showing_latest_50), fontSize = 11.sp, color = CodegTheme.colors.textTertiary, modifier = Modifier.fillMaxWidth().padding(12.dp))
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun SyncHeader(
    branch: String?,
    trackingRemote: String?,
    hasRemote: Boolean,
    unpushed: Int,
    busy: Boolean,
    onPull: () -> Unit,
    onPush: () -> Unit,
    onFetch: () -> Unit,
) {
    val colors = CodegTheme.colors
    Column(
        Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 6.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            Icon(Icons.AutoMirrored.Rounded.CallSplit, contentDescription = null, tint = colors.textSecondary, modifier = Modifier.size(14.dp))
            Text(branch ?: "—", fontSize = 12.sp, fontWeight = FontWeight.Medium, color = colors.textPrimary, fontFamily = FontFamily.Monospace)
            if (trackingRemote != null) {
                Text("→", fontSize = 12.sp, color = colors.textTertiary)
                Text(trackingRemote, fontSize = 12.sp, color = colors.textSecondary, fontFamily = FontFamily.Monospace)
            }
            Box(Modifier.weight(1f))
            if (!hasRemote) {
                Text(stringResource(R.string.sync_no_remote), fontSize = 11.sp, color = colors.textTertiary)
            }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            SyncButton(Icons.Rounded.CloudDownload, stringResource(R.string.sync_pull), enabled = hasRemote && !busy, onClick = onPull, modifier = Modifier.weight(1f))
            SyncButton(Icons.Rounded.CloudUpload, stringResource(R.string.sync_push), enabled = hasRemote && !busy, onClick = onPush, badge = unpushed.takeIf { it > 0 }, modifier = Modifier.weight(1f))
            SyncButton(Icons.Rounded.Sync, stringResource(R.string.sync_fetch), enabled = hasRemote && !busy, onClick = onFetch, modifier = Modifier.weight(1f))
        }
    }
}

@Composable
private fun SyncButton(
    icon: ImageVector,
    label: String,
    enabled: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    badge: Int? = null,
) {
    val colors = CodegTheme.colors
    val tint = if (enabled) colors.accent else colors.textTertiary
    Row(
        modifier
            .clip(RoundedCornerShape(10.dp))
            .border(1.dp, colors.surfaceStroke, RoundedCornerShape(10.dp))
            .background(if (enabled) colors.accent.copy(alpha = 0.08f) else Color.Transparent)
            .clickable(enabled = enabled) { onClick() }
            .padding(vertical = 9.dp),
        horizontalArrangement = Arrangement.Center,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(icon, contentDescription = null, tint = tint, modifier = Modifier.size(16.dp))
        Text(label, fontSize = 13.sp, fontWeight = FontWeight.Medium, color = tint, modifier = Modifier.padding(start = 6.dp))
        if (badge != null) {
            Text(
                badge.toString(),
                fontSize = 10.sp,
                fontWeight = FontWeight.Bold,
                color = colors.onAccent,
                modifier = Modifier.padding(start = 5.dp).clip(RoundedCornerShape(50)).background(colors.accent).padding(horizontal = 5.dp, vertical = 1.dp),
            )
        }
    }
}

@Composable
private fun CommitRow(entry: GitLogEntry, onClick: () -> Unit) {
    val colors = CodegTheme.colors
    ListItem(
        headlineContent = { Text(entry.subject, fontSize = 14.sp, fontWeight = FontWeight.Medium, maxLines = 2) },
        supportingContent = {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(entry.author, fontSize = 11.sp, color = colors.textTertiary, maxLines = 1)
                Text(relativeDate(entry.date), fontSize = 11.sp, color = colors.textTertiary)
                if (entry.totalAdditions > 0) Text("+${entry.totalAdditions}", fontSize = 11.sp, color = Color_Add)
                if (entry.totalDeletions > 0) Text("−${entry.totalDeletions}", fontSize = 11.sp, color = colors.danger)
                Box(Modifier.weight(1f))
                Text(entry.hash, fontFamily = FontFamily.Monospace, fontSize = 10.sp, color = colors.textTertiary, modifier = Modifier.clip(RoundedCornerShape(50)).background(colors.textPrimary.copy(alpha = 0.06f)).padding(horizontal = 6.dp, vertical = 1.dp))
            }
        },
        trailingContent = entry.pushed?.let { pushed ->
            { Icon(if (pushed) Icons.Rounded.Cloud else Icons.Rounded.CloudOff, null, tint = colors.textTertiary, modifier = Modifier.size(14.dp)) }
        },
        colors = ListItemDefaults.colors(containerColor = Color.Transparent, headlineColor = colors.textPrimary),
        modifier = Modifier.clip(RoundedCornerShape(10.dp)).clickable { onClick() },
    )
}

@Composable
private fun CommitDetail(client: CodegClient, rootPath: String, entry: GitLogEntry) {
    val colors = CodegTheme.colors
    var result by remember(entry.fullHash) { mutableStateOf<Result<String>?>(null) }
    LaunchedEffect(entry.fullHash) { result = runCatching { client.gitShowDiff(rootPath, entry.fullHash) } }

    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(horizontal = 12.dp, vertical = 10.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        item("header") {
            Column(Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp)).background(colors.bgElevated.copy(alpha = 0.5f)).padding(12.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                Text(entry.subject, fontSize = 16.sp, fontWeight = FontWeight.SemiBold, color = colors.textPrimary)
                if (entry.body.isNotEmpty()) Text(entry.body, fontSize = 13.sp, color = colors.textSecondary)
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(entry.author, fontSize = 12.sp, color = colors.textSecondary)
                    Text(relativeDate(entry.date), fontSize = 12.sp, color = colors.textTertiary)
                    entry.pushed?.let { Icon(if (it) Icons.Rounded.Cloud else Icons.Rounded.CloudOff, null, tint = colors.textTertiary, modifier = Modifier.size(13.dp)) }
                }
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(entry.fullHash, fontFamily = FontFamily.Monospace, fontSize = 11.sp, color = colors.textTertiary, maxLines = 1, modifier = Modifier.weight(1f, fill = false))
                    CopyButton(entry.fullHash)
                }
            }
        }
        item("summary") {
            Text(stringResource(R.string.commits_files_changed_summary, entry.files.size, entry.totalAdditions, entry.totalDeletions), fontSize = 12.sp, color = colors.textSecondary)
        }
        item("diff") {
            val r = result
            when {
                r == null -> Box(Modifier.fillMaxWidth().padding(20.dp), contentAlignment = Alignment.Center) { LoadingView(stringResource(R.string.commits_loading_diff)) }
                else -> {
                    val files = UnifiedDiff.parse(r.getOrDefault(""))
                    if (files != null) DiffView(files)
                    else Column(verticalArrangement = Arrangement.spacedBy(3.dp)) {
                        for (f in entry.files) Text("${f.status}  ${f.path}   +${f.additions} −${f.deletions}", fontFamily = FontFamily.Monospace, fontSize = 11.sp, color = colors.textSecondary)
                    }
                }
            }
        }
    }
}

private val Color_Add = Color(0xFF85D18F)

private fun relativeDate(iso: String): String =
    runCatching { RelativeTime.compact(OffsetDateTime.parse(iso).toInstant()) }.getOrDefault(iso.take(10))
