package app.codeg.android.feature.projects

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material.icons.rounded.ArrowUpward
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.Folder
import androidx.compose.material.icons.rounded.Home
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import app.codeg.android.R
import app.codeg.android.core.designsystem.component.PrimaryButton
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.model.DirectoryEntry
import kotlinx.coroutines.launch

/** Full-screen server-filesystem browser used by Open Folder + Clone. Port of iOS `DirectoryBrowserView`. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DirectoryBrowserSheet(
    title: String,
    confirmLabel: String,
    loadHome: suspend () -> String,
    loadDirs: suspend (String) -> List<DirectoryEntry>,
    onSelect: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    val colors = CodegTheme.colors
    val scope = rememberCoroutineScope()
    var path by remember { mutableStateOf("") }
    var dirs by remember { mutableStateOf<List<DirectoryEntry>>(emptyList()) }
    var loading by remember { mutableStateOf(true) }

    LaunchedEffect(Unit) { path = loadHome() }
    LaunchedEffect(path) {
        if (path.isNotEmpty()) {
            loading = true
            dirs = loadDirs(path)
            loading = false
        }
    }

    Dialog(onDismissRequest = onDismiss, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        Column(Modifier.fillMaxSize().background(colors.bg).windowInsetsPadding(WindowInsets.safeDrawing)) {
            TopAppBar(
                title = { Text(title, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                navigationIcon = {
                    IconButton(onClick = onDismiss) { Icon(Icons.Rounded.Close, contentDescription = stringResource(R.string.dirbrowser_close), tint = colors.textPrimary) }
                },
                actions = {
                    IconButton(onClick = { scope.launch { path = loadHome() } }) {
                        Icon(Icons.Rounded.Home, contentDescription = stringResource(R.string.dirbrowser_home), tint = colors.textSecondary)
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = androidx.compose.ui.graphics.Color.Transparent, titleContentColor = colors.textPrimary),
            )
            Text(path, fontFamily = FontFamily.Monospace, fontSize = 11.sp, color = colors.textTertiary, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp))
            Box(Modifier.weight(1f).fillMaxWidth()) {
                if (loading) {
                    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { CircularProgressIndicator(color = colors.accent) }
                } else {
                    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(horizontal = 12.dp)) {
                        val parent = path.substringBeforeLast('/', "")
                        if (parent.isNotEmpty() && parent != path) {
                            item("up") { BrowserRow("..", hasChildren = false, icon = Icons.Rounded.ArrowUpward) { path = parent } }
                        }
                        items(dirs, key = { it.path }) { entry ->
                            BrowserRow(entry.name, entry.hasChildren, Icons.Rounded.Folder) { path = entry.path }
                        }
                    }
                }
            }
            PrimaryButton(
                text = confirmLabel,
                onClick = { onSelect(path) },
                enabled = path.isNotEmpty(),
                modifier = Modifier.padding(16.dp),
            )
        }
    }
}

@Composable
private fun BrowserRow(name: String, hasChildren: Boolean, icon: ImageVector, onClick: () -> Unit) {
    val colors = CodegTheme.colors
    Row(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(10.dp)).clickable { onClick() }.padding(horizontal = 8.dp, vertical = 11.dp),
        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(11.dp),
    ) {
        Icon(icon, contentDescription = null, tint = colors.accent, modifier = Modifier.size(20.dp))
        Text(name, fontSize = 14.sp, color = colors.textPrimary, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
        if (hasChildren) Icon(Icons.AutoMirrored.Rounded.KeyboardArrowRight, contentDescription = null, tint = colors.textTertiary, modifier = Modifier.size(18.dp))
    }
}
