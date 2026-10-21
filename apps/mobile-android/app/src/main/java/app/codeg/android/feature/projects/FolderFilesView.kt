package app.codeg.android.feature.projects

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.InsertDriveFile
import androidx.compose.material.icons.rounded.ArrowUpward
import androidx.compose.material.icons.rounded.Folder
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
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.codeg.android.R
import app.codeg.android.core.designsystem.component.CopyButton
import app.codeg.android.core.designsystem.component.EmptyState
import app.codeg.android.core.designsystem.component.InlineError
import app.codeg.android.core.designsystem.component.LoadingView
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.model.DirectoryItem
import app.codeg.android.core.network.CodegClient

/** Recursive file browser with an internal directory stack + file preview. */
@Composable
fun FolderFilesView(client: CodegClient, rootPath: String) {
    val colors = CodegTheme.colors
    var stack by remember { mutableStateOf(listOf(rootPath)) }
    var previewPath by remember { mutableStateOf<String?>(null) }

    BackHandler(enabled = previewPath != null || stack.size > 1) {
        if (previewPath != null) previewPath = null else stack = stack.dropLast(1)
    }

    val preview = previewPath
    if (preview != null) {
        FilePreview(client, rootPath, preview)
        return
    }

    val dir = stack.last()
    var result by remember(dir) { mutableStateOf<Result<List<DirectoryItem>>?>(null) }
    LaunchedEffect(dir) { result = runCatching { client.listDirectoryWithFiles(dir) } }

    val r = result
    when {
        r == null -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { LoadingView(message = stringResource(R.string.common_loading)) }
        r.isFailure -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
            InlineError(Icons.AutoMirrored.Rounded.InsertDriveFile, stringResource(R.string.files_load_failed), r.exceptionOrNull()?.message ?: stringResource(R.string.files_error), onRetry = { result = null })
        }
        else -> {
            val items = sortItems(r.getOrDefault(emptyList()))
            LazyColumn(Modifier.fillMaxSize(), contentPadding = androidx.compose.foundation.layout.PaddingValues(horizontal = 12.dp, vertical = 4.dp)) {
                if (stack.size > 1) {
                    item("up") {
                        Row(
                            Modifier.fillMaxWidth().clip(RoundedCornerShape(10.dp)).clickable { stack = stack.dropLast(1) }.padding(8.dp),
                            verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp),
                        ) {
                            Icon(Icons.Rounded.ArrowUpward, null, tint = colors.textSecondary, modifier = Modifier.size(20.dp))
                            Text("..", fontFamily = FontFamily.Monospace, color = colors.textSecondary)
                        }
                    }
                }
                if (items.isEmpty()) {
                    item("empty") { Box(Modifier.fillMaxWidth().padding(40.dp), contentAlignment = Alignment.Center) { EmptyState(Icons.Rounded.Folder, stringResource(R.string.files_empty_folder), stringResource(R.string.files_empty_folder_message)) } }
                }
                items(items, key = { it.path }) { item ->
                    FileRow(item) {
                        if (item.isDir) stack = stack + item.path else previewPath = item.path
                    }
                }
            }
        }
    }
}

@Composable
private fun FileRow(item: DirectoryItem, onClick: () -> Unit) {
    val colors = CodegTheme.colors
    val sizeLabel = item.size?.takeIf { !item.isDir }?.let { formatBytes(it) }
    ListItem(
        headlineContent = { Text(item.name, fontSize = 14.sp, maxLines = 1, overflow = TextOverflow.Ellipsis) },
        leadingContent = {
            Icon(
                if (item.isDir) Icons.Rounded.Folder else Icons.AutoMirrored.Rounded.InsertDriveFile,
                contentDescription = null,
                tint = if (item.isDir) colors.accent else colors.textSecondary,
                modifier = Modifier.size(20.dp),
            )
        },
        trailingContent = sizeLabel?.let { { Text(it, fontSize = 11.sp, color = colors.textTertiary) } },
        colors = ListItemDefaults.colors(containerColor = Color.Transparent, headlineColor = colors.textPrimary),
        modifier = Modifier.clip(RoundedCornerShape(10.dp)).clickable { onClick() },
    )
}

@Composable
private fun FilePreview(client: CodegClient, rootPath: String, absPath: String) {
    val colors = CodegTheme.colors
    val rel = if (absPath.startsWith("$rootPath/")) absPath.removePrefix("$rootPath/") else absPath.substringAfterLast('/')
    var result by remember(absPath) { mutableStateOf<Result<String>?>(null) }
    LaunchedEffect(absPath) { result = runCatching { client.readFilePreview(rootPath, rel).content } }

    Column(Modifier.fillMaxSize()) {
        Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(rel, fontFamily = FontFamily.Monospace, fontSize = 12.sp, color = colors.textSecondary, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
            result?.getOrNull()?.let { if (it.isNotEmpty()) CopyButton(it) }
        }
        val r = result
        when {
            r == null -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { LoadingView(stringResource(R.string.common_loading)) }
            r.isFailure -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                InlineError(Icons.AutoMirrored.Rounded.InsertDriveFile, stringResource(R.string.files_read_failed), r.exceptionOrNull()?.message ?: stringResource(R.string.files_error), onRetry = { result = null })
            }
            else -> {
                val content = r.getOrDefault("")
                if (content.isEmpty()) {
                    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { EmptyState(Icons.AutoMirrored.Rounded.InsertDriveFile, stringResource(R.string.files_empty_file), stringResource(R.string.files_empty_file_message)) }
                } else {
                    FileContent(content)
                }
            }
        }
    }
}

@Composable
private fun FileContent(content: String) {
    val colors = CodegTheme.colors
    val truncationNote = stringResource(R.string.files_truncated_note, 4000)
    val lines = remember(content, truncationNote) {
        val all = content.split("\n")
        if (all.size > 4000) all.take(4000) + truncationNote else all
    }
    val gutterWidth = (lines.size.toString().length * 8 + 12).dp
    LazyColumn(
        Modifier.fillMaxSize().padding(horizontal = 12.dp).clip(RoundedCornerShape(10.dp)).background(colors.codeSurface).border(0.5.dp, colors.hairline, RoundedCornerShape(10.dp)),
    ) {
        items(lines.size) { i ->
            Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState())) {
                Text("${i + 1}", fontFamily = FontFamily.Monospace, fontSize = 11.sp, color = colors.textTertiary, modifier = Modifier.width(gutterWidth).padding(start = 6.dp, end = 6.dp))
                Text(lines[i].ifEmpty { " " }, fontFamily = FontFamily.Monospace, fontSize = 12.sp, color = colors.textSecondary)
            }
        }
    }
}

private fun sortItems(items: List<DirectoryItem>): List<DirectoryItem> =
    items.sortedWith(compareByDescending<DirectoryItem> { it.isDir }.thenBy { it.name.lowercase() })

private fun formatBytes(bytes: Long): String = when {
    bytes >= 1_073_741_824 -> String.format("%.1f GB", bytes / 1_073_741_824.0)
    bytes >= 1_048_576 -> String.format("%.1f MB", bytes / 1_048_576.0)
    bytes >= 1024 -> String.format("%.1f KB", bytes / 1024.0)
    else -> "$bytes B"
}
