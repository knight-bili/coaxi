package app.codeg.android.feature.settings

import androidx.compose.foundation.background
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.ExperimentalFoundationApi
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
import androidx.compose.material.icons.automirrored.rounded.Chat
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
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
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.codeg.android.R
import app.codeg.android.core.designsystem.component.CodegTextField
import app.codeg.android.core.designsystem.component.EmptyState
import app.codeg.android.core.designsystem.component.LoadingView
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.model.QuickMessage

@OptIn(ExperimentalFoundationApi::class)
@Composable
fun QuickMessagesContent(viewModel: QuickMessagesViewModel = hiltViewModel()) {
    val ui by viewModel.ui.collectAsStateWithLifecycle()
    val colors = CodegTheme.colors
    var editor by remember { mutableStateOf<EditorTarget?>(null) }

    Box(Modifier.fillMaxSize()) {
        when {
            ui.loading && ui.messages.isEmpty() ->
                Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { LoadingView(stringResource(R.string.common_loading)) }
            else -> LazyColumn(Modifier.fillMaxSize(), contentPadding = androidx.compose.foundation.layout.PaddingValues(start = 12.dp, end = 12.dp, top = 8.dp, bottom = 88.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                if (ui.messages.isEmpty() && !ui.loading) {
                    item("empty") { Box(Modifier.fillMaxWidth().padding(40.dp), contentAlignment = Alignment.Center) { EmptyState(Icons.AutoMirrored.Rounded.Chat, stringResource(R.string.quickmsg_empty_title), stringResource(R.string.quickmsg_empty_subtitle)) } }
                }
                items(ui.messages, key = { it.id }) { msg ->
                    MessageRow(msg, onEdit = { editor = EditorTarget(msg) }, onDelete = { viewModel.delete(msg) })
                }
            }
        }
        if (!(ui.loading && ui.messages.isEmpty())) {
            FloatingActionButton(
                onClick = { editor = EditorTarget(null) },
                modifier = Modifier.align(Alignment.BottomEnd).padding(16.dp),
                containerColor = colors.accent,
                contentColor = colors.onAccent,
            ) { Icon(Icons.Rounded.Add, contentDescription = stringResource(R.string.quickmsg_new_cd)) }
        }
    }

    editor?.let { target ->
        QuickMessageEditor(
            existing = target.message,
            onDismiss = { editor = null },
            onSave = { title, content -> viewModel.save(target.message, title, content); editor = null },
        )
    }
}

private data class EditorTarget(val message: QuickMessage?)

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun MessageRow(msg: QuickMessage, onEdit: () -> Unit, onDelete: () -> Unit) {
    val colors = CodegTheme.colors
    var menu by remember { mutableStateOf(false) }
    Box {
        Column(
            Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp)).background(colors.bgElevated.copy(alpha = 0.5f))
                .combinedClickable(onClick = onEdit, onLongClick = { menu = true }).padding(14.dp),
            verticalArrangement = Arrangement.spacedBy(3.dp),
        ) {
            Text(msg.title, fontSize = 15.sp, color = colors.textPrimary, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(msg.content, fontSize = 13.sp, color = colors.textTertiary, maxLines = 2, overflow = TextOverflow.Ellipsis)
        }
        DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
            DropdownMenuItem(text = { Text(stringResource(R.string.common_delete)) }, onClick = { menu = false; onDelete() })
        }
    }
}

@Composable
private fun QuickMessageEditor(existing: QuickMessage?, onDismiss: () -> Unit, onSave: (String, String) -> Unit) {
    var title by remember { mutableStateOf(existing?.title ?: "") }
    var content by remember { mutableStateOf(existing?.content ?: "") }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(if (existing == null) stringResource(R.string.quickmsg_new_title) else stringResource(R.string.quickmsg_edit_title)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                CodegTextField(title, { title = it }, label = stringResource(R.string.quickmsg_title_label))
                CodegTextField(content, { content = it }, label = stringResource(R.string.quickmsg_content_label), singleLine = false)
            }
        },
        confirmButton = {
            TextButton(onClick = { onSave(title, content) }, enabled = title.isNotBlank() && content.isNotBlank()) { Text(stringResource(R.string.common_save)) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.common_cancel)) } },
        containerColor = CodegTheme.colors.bgElevated,
    )
}
