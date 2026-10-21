package app.codeg.android.feature.server

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.Delete
import androidx.compose.material.icons.rounded.Dns
import androidx.compose.material.icons.rounded.Edit
import androidx.compose.material.icons.rounded.MoreVert
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import app.codeg.android.R
import app.codeg.android.core.datastore.ServerProfile
import app.codeg.android.core.designsystem.component.EmptyState
import app.codeg.android.core.designsystem.component.GlassRow
import app.codeg.android.core.designsystem.theme.CodegTheme

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ServerListScreen(
    servers: List<ServerProfile>,
    selectedId: String?,
    onSelect: (String) -> Unit,
    onDelete: (String) -> Unit,
    onAdd: () -> Unit,
    onEdit: (String) -> Unit,
    onBack: () -> Unit,
) {
    val colors = CodegTheme.colors
    var pendingDelete by remember { mutableStateOf<ServerProfile?>(null) }

    Scaffold(
        containerColor = androidx.compose.ui.graphics.Color.Transparent,
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.servers_title)) },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Rounded.ArrowBack, contentDescription = null, tint = colors.textSecondary)
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = androidx.compose.ui.graphics.Color.Transparent,
                    titleContentColor = colors.textPrimary,
                ),
            )
        },
        floatingActionButton = {
            FloatingActionButton(
                onClick = onAdd,
                containerColor = colors.accent,
                contentColor = colors.onAccent,
            ) {
                Icon(Icons.Rounded.Add, contentDescription = stringResource(R.string.server_add))
            }
        },
    ) { padding ->
        if (servers.isEmpty()) {
            EmptyState(
                icon = Icons.Rounded.Dns,
                title = stringResource(R.string.servers_title),
                message = stringResource(R.string.onboarding_footnote),
                actionLabel = stringResource(R.string.server_add),
                onAction = onAdd,
                modifier = Modifier.padding(padding),
            )
        } else {
            LazyColumn(
                modifier = Modifier.fillMaxSize().padding(padding),
                contentPadding = androidx.compose.foundation.layout.PaddingValues(start = 16.dp, end = 16.dp, top = 16.dp, bottom = 88.dp),
                verticalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                items(servers, key = { it.id }) { profile ->
                    ServerRow(
                        profile = profile,
                        selected = profile.id == selectedId,
                        onClick = { onSelect(profile.id) },
                        onEdit = { onEdit(profile.id) },
                        onDelete = { pendingDelete = profile },
                    )
                }
            }
        }
    }

    val toDelete = pendingDelete
    if (toDelete != null) {
        AlertDialog(
            onDismissRequest = { pendingDelete = null },
            title = { Text(stringResource(R.string.server_delete)) },
            text = { Text(toDelete.name) },
            confirmButton = {
                TextButton(onClick = { onDelete(toDelete.id); pendingDelete = null }) {
                    Text(stringResource(R.string.server_delete), color = colors.danger)
                }
            },
            dismissButton = {
                TextButton(onClick = { pendingDelete = null }) {
                    Text(stringResource(R.string.common_cancel))
                }
            },
        )
    }
}

@Composable
private fun ServerRow(
    profile: ServerProfile,
    selected: Boolean,
    onClick: () -> Unit,
    onEdit: () -> Unit,
    onDelete: () -> Unit,
) {
    val colors = CodegTheme.colors
    var menuOpen by remember { mutableStateOf(false) }

    GlassRow(selected = selected, modifier = Modifier.fillMaxWidth().clickable(onClick = onClick)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Column(modifier = Modifier.weight(1f)) {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    if (selected) {
                        Icon(Icons.Rounded.Check, contentDescription = null, tint = colors.accent, modifier = Modifier.size(18.dp))
                    }
                    Text(profile.name, style = MaterialTheme.typography.titleSmall, color = colors.textPrimary)
                }
                Text(
                    profile.baseUrl,
                    style = MaterialTheme.typography.bodySmall,
                    color = colors.textSecondary,
                    fontFamily = FontFamily.Monospace,
                    modifier = Modifier.padding(top = 2.dp),
                )
            }
            IconButton(onClick = { menuOpen = true }) {
                Icon(Icons.Rounded.MoreVert, contentDescription = null, tint = colors.textTertiary)
            }
            DropdownMenu(expanded = menuOpen, onDismissRequest = { menuOpen = false }) {
                DropdownMenuItem(
                    text = { Text(stringResource(R.string.server_edit)) },
                    leadingIcon = { Icon(Icons.Rounded.Edit, contentDescription = null) },
                    onClick = { menuOpen = false; onEdit() },
                )
                DropdownMenuItem(
                    text = { Text(stringResource(R.string.server_delete), color = colors.danger) },
                    leadingIcon = { Icon(Icons.Rounded.Delete, contentDescription = null, tint = colors.danger) },
                    onClick = { menuOpen = false; onDelete() },
                )
            }
        }
    }
}
