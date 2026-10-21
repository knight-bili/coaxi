package app.codeg.android.feature.settings

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.Memory
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExposedDropdownMenuBox
import androidx.compose.material3.ExposedDropdownMenuDefaults
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MenuAnchorType
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.codeg.android.R
import app.codeg.android.core.designsystem.component.CodegTextField
import app.codeg.android.core.designsystem.component.EmptyState
import app.codeg.android.core.designsystem.component.LoadingView
import app.codeg.android.core.designsystem.component.SecretField
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.model.AgentType
import app.codeg.android.core.model.ModelProviderInfo

@OptIn(ExperimentalFoundationApi::class)
@Composable
fun ModelProvidersContent(viewModel: ModelProvidersViewModel = hiltViewModel()) {
    val ui by viewModel.ui.collectAsStateWithLifecycle()
    val colors = CodegTheme.colors
    var editor by remember { mutableStateOf<ProviderEditorTarget?>(null) }

    Box(Modifier.fillMaxSize()) {
        when {
            ui.loading && ui.providers.isEmpty() -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { LoadingView(stringResource(R.string.common_loading)) }
            else -> LazyColumn(Modifier.fillMaxSize(), contentPadding = androidx.compose.foundation.layout.PaddingValues(start = 12.dp, end = 12.dp, top = 8.dp, bottom = 88.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                if (ui.providers.isEmpty() && !ui.loading) {
                    item("empty") { Box(Modifier.fillMaxWidth().padding(40.dp), contentAlignment = Alignment.Center) { EmptyState(Icons.Rounded.Memory, stringResource(R.string.providers_empty_title), stringResource(R.string.providers_empty_subtitle)) } }
                }
                for (agent in ui.supportedAgents) {
                    val group = ui.providers.filter { it.agentType == agent }
                    if (group.isNotEmpty()) {
                        item("h-${agent.wire}") {
                            Text(agent.displayName, fontSize = 12.sp, fontWeight = FontWeight.SemiBold, color = colors.textTertiary, modifier = Modifier.padding(start = 8.dp, top = 8.dp))
                        }
                        items(group, key = { it.id }) { p -> ProviderRow(p, onEdit = { editor = ProviderEditorTarget(p) }, onDelete = { viewModel.delete(p) }) }
                    }
                }
            }
        }
        if (!(ui.loading && ui.providers.isEmpty())) {
            FloatingActionButton(
                onClick = { editor = ProviderEditorTarget(null) },
                modifier = Modifier.align(Alignment.BottomEnd).padding(16.dp),
                containerColor = colors.accent,
                contentColor = colors.onAccent,
            ) { Icon(Icons.Rounded.Add, contentDescription = stringResource(R.string.providers_new_cd)) }
        }
    }

    editor?.let { target ->
        ProviderEditor(
            existing = target.provider,
            supportedAgents = ui.supportedAgents,
            onDismiss = { editor = null },
            onSave = { name, url, key, agent, model -> viewModel.save(target.provider, name, url, key, agent, model); editor = null },
        )
    }
}

private data class ProviderEditorTarget(val provider: ModelProviderInfo?)

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun ProviderRow(p: ModelProviderInfo, onEdit: () -> Unit, onDelete: () -> Unit) {
    val colors = CodegTheme.colors
    var menu by remember { mutableStateOf(false) }
    Box {
        Column(
            Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp)).background(colors.bgElevated.copy(alpha = 0.5f))
                .combinedClickable(onClick = onEdit, onLongClick = { menu = true }).padding(14.dp),
            verticalArrangement = Arrangement.spacedBy(2.dp),
        ) {
            Text(p.name, fontSize = 15.sp, color = colors.textPrimary, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(p.apiUrl, fontFamily = FontFamily.Monospace, fontSize = 11.sp, color = colors.textTertiary, maxLines = 1, overflow = TextOverflow.Ellipsis)
            if (p.apiKeyMasked.isNotEmpty()) Text(p.apiKeyMasked, fontFamily = FontFamily.Monospace, fontSize = 11.sp, color = colors.textTertiary)
        }
        DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
            DropdownMenuItem(text = { Text(stringResource(R.string.common_delete)) }, onClick = { menu = false; onDelete() })
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ProviderEditor(
    existing: ModelProviderInfo?,
    supportedAgents: List<AgentType>,
    onDismiss: () -> Unit,
    onSave: (String, String, String, AgentType, String?) -> Unit,
) {
    val colors = CodegTheme.colors
    var name by remember { mutableStateOf(existing?.name ?: "") }
    var url by remember { mutableStateOf(existing?.apiUrl ?: "") }
    var key by remember { mutableStateOf("") }
    var agent by remember { mutableStateOf(existing?.agentType ?: supportedAgents.first()) }
    var model by remember { mutableStateOf(existing?.model ?: "") }
    var agentMenu by remember { mutableStateOf(false) }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(if (existing == null) stringResource(R.string.providers_new_title) else stringResource(R.string.providers_edit_title)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                CodegTextField(name, { name = it }, label = stringResource(R.string.server_name))
                CodegTextField(url, { url = it }, label = stringResource(R.string.providers_api_url), mono = true)
                SecretField(key, { key = it }, label = if (existing == null) stringResource(R.string.providers_api_key) else stringResource(R.string.providers_api_key_keep))
                ExposedDropdownMenuBox(expanded = agentMenu, onExpandedChange = { agentMenu = it }) {
                    OutlinedTextField(
                        value = agent.displayName,
                        onValueChange = {},
                        readOnly = true,
                        label = { Text(stringResource(R.string.providers_agent)) },
                        trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(expanded = agentMenu) },
                        colors = OutlinedTextFieldDefaults.colors(
                            focusedBorderColor = colors.accent,
                            unfocusedBorderColor = colors.surfaceStroke,
                            focusedLabelColor = colors.accent,
                            unfocusedLabelColor = colors.textTertiary,
                            focusedTextColor = colors.textPrimary,
                            unfocusedTextColor = colors.textPrimary,
                            focusedTrailingIconColor = colors.textTertiary,
                            unfocusedTrailingIconColor = colors.textTertiary,
                        ),
                        modifier = Modifier.menuAnchor(MenuAnchorType.PrimaryNotEditable).fillMaxWidth(),
                    )
                    ExposedDropdownMenu(expanded = agentMenu, onDismissRequest = { agentMenu = false }) {
                        supportedAgents.forEach { a ->
                            DropdownMenuItem(text = { Text(a.displayName) }, onClick = { agent = a; agentMenu = false })
                        }
                    }
                }
                CodegTextField(model, { model = it }, label = stringResource(R.string.providers_model_optional), mono = true)
            }
        },
        confirmButton = {
            TextButton(onClick = { onSave(name, url, key, agent, model.ifBlank { null }) }, enabled = name.isNotBlank() && url.isNotBlank()) { Text(stringResource(R.string.common_save)) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.common_cancel)) } },
        containerColor = colors.bgElevated,
    )
}
