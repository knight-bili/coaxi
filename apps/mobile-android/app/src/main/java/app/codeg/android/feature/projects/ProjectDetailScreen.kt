package app.codeg.android.feature.projects

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.rounded.FolderOpen
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
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
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.codeg.android.R
import app.codeg.android.core.designsystem.component.CodegSegmented
import app.codeg.android.core.designsystem.component.CountBadge
import app.codeg.android.core.designsystem.component.InlineError
import app.codeg.android.core.designsystem.component.LivePulse
import app.codeg.android.core.designsystem.component.LoadingView
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.designsystem.theme.colorFromHex

private enum class ProjectTab(val labelRes: Int) {
    FILES(R.string.projects_tab_files),
    CHANGES(R.string.projects_tab_changes),
    COMMITS(R.string.projects_tab_commits),
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ProjectDetailScreen(
    onBack: () -> Unit,
    viewModel: ProjectDetailViewModel = hiltViewModel(),
) {
    val ui by viewModel.ui.collectAsStateWithLifecycle()
    val colors = CodegTheme.colors
    var tab by remember { mutableStateOf(ProjectTab.FILES) }
    var showBranchPicker by remember { mutableStateOf(false) }
    var showCommit by remember { mutableStateOf(false) }

    if (showBranchPicker) BranchPickerSheet(viewModel, onDismiss = { showBranchPicker = false })

    val commitClient = ui.client
    val commitFolder = ui.folder
    if (showCommit && commitClient != null && commitFolder != null) {
        CommitSheet(
            viewModel = viewModel,
            client = commitClient,
            rootPath = commitFolder.path,
            onDismiss = { showCommit = false },
            onCommitted = { push ->
                showCommit = false
                // Run the deferred push AFTER the commit sheet dismisses, so a
                // credential prompt doesn't stack on top of the composer.
                if (push) viewModel.push()
            },
        )
    }

    // One credential sheet for every push/pull/fetch, whichever tab initiated it.
    // key(id) gives each (re-)prompt fresh field/submitting state, like iOS's
    // item-keyed sheet — otherwise a retry would reuse the prior prompt's state.
    ui.credentialPrompt?.let { prompt -> key(prompt.id) { GitCredentialSheet(viewModel, prompt) } }

    Scaffold(
        containerColor = Color.Transparent,
        contentWindowInsets = WindowInsets(0, 0, 0, 0),
        topBar = {
            TopAppBar(
                title = { Text(ui.folder?.name ?: stringResource(R.string.tab_folders), maxLines = 1, overflow = TextOverflow.Ellipsis) },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Rounded.ArrowBack, contentDescription = stringResource(R.string.common_back), tint = colors.textPrimary)
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent, titleContentColor = colors.textPrimary),
            )
        },
    ) { padding ->
        Box(Modifier.padding(padding).fillMaxSize()) {
            val folder = ui.folder
            when {
                ui.loading && folder == null ->
                    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { LoadingView(stringResource(R.string.common_loading)) }
                folder == null ->
                    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                        InlineError(Icons.Rounded.FolderOpen, stringResource(R.string.projects_load_failed), ui.error ?: "Folder not found", viewModel::load)
                    }
                else -> Column(Modifier.fillMaxSize()) {
                    Header(
                        name = folder.name,
                        path = folder.path,
                        color = colorFromHex(folder.color) ?: colors.accent,
                        branch = ui.branch,
                        running = ui.runningCount,
                        sessions = ui.sessionCount,
                        onBranchClick = { showBranchPicker = true },
                    )
                    SegmentedTabs(tab) { tab = it }
                    Box(Modifier.fillMaxSize()) {
                        val client = ui.client
                        if (client != null) {
                            when (tab) {
                                ProjectTab.FILES -> FolderFilesView(client, folder.path)
                                ProjectTab.CHANGES -> FolderChangesView(viewModel, client, folder.path, onCommit = { showCommit = true })
                                ProjectTab.COMMITS -> FolderCommitsView(viewModel, client, folder.path)
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun Header(name: String, path: String, color: Color, branch: String?, running: Int, sessions: Int, onBranchClick: () -> Unit = {}) {
    val colors = CodegTheme.colors
    Row(
        Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Box(Modifier.size(48.dp).clip(RoundedCornerShape(12.dp)).background(color.copy(alpha = 0.22f)).border(1.dp, color.copy(alpha = 0.45f), RoundedCornerShape(12.dp)))
        Column(Modifier.weight(1f)) {
            Text(name, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold, color = colors.textPrimary, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(path, fontFamily = FontFamily.Monospace, fontSize = 11.sp, color = colors.textTertiary, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.padding(top = 4.dp)) {
                branch?.let { Box(Modifier.clip(RoundedCornerShape(50)).clickable { onBranchClick() }) { BranchPill(it) } }
                if (running > 0) {
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(3.dp)) {
                        LivePulse(dotSize = 7.dp); Text("$running", fontSize = 11.sp, color = colors.accent)
                    }
                }
                if (sessions > 0) CountBadge(sessions)
            }
        }
    }
}

@Composable
private fun SegmentedTabs(selected: ProjectTab, onSelect: (ProjectTab) -> Unit) {
    val entries = ProjectTab.entries
    CodegSegmented(
        options = entries.map { stringResource(it.labelRes) },
        selectedIndex = entries.indexOf(selected),
        onSelect = { onSelect(entries[it]) },
        modifier = Modifier.padding(horizontal = 16.dp, vertical = 6.dp),
    )
}
