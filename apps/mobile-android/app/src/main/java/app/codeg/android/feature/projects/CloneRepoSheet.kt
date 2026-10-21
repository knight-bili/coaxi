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
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.CreateNewFolder
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Text
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
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import app.codeg.android.R
import app.codeg.android.core.designsystem.component.CodegTextField
import app.codeg.android.core.designsystem.component.PrimaryButton
import app.codeg.android.core.designsystem.component.SecretField
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.model.DirectoryEntry
import app.codeg.android.core.model.GitCredentials
import kotlinx.coroutines.launch

/** Clone-a-repository sheet. Port of iOS `CloneRepoView`. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun CloneRepoSheet(
    loadHome: suspend () -> String,
    loadDirs: suspend (String) -> List<DirectoryEntry>,
    clone: suspend (String, String, GitCredentials?) -> String?,
    onCloned: () -> Unit,
    onDismiss: () -> Unit,
) {
    val colors = CodegTheme.colors
    val scope = rememberCoroutineScope()
    var url by remember { mutableStateOf("") }
    var parentDir by remember { mutableStateOf("") }
    var username by remember { mutableStateOf("") }
    var password by remember { mutableStateOf("") }
    var cloning by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    var showBrowser by remember { mutableStateOf(false) }

    val repoName = remember(url) { url.trim().trimEnd('/').substringAfterLast('/').removeSuffix(".git").ifEmpty { "repo" } }
    val targetDir = if (parentDir.isBlank()) "" else "${parentDir.trim().trimEnd('/')}/$repoName"
    val canClone = url.isNotBlank() && parentDir.isNotBlank() && !cloning

    Dialog(onDismissRequest = onDismiss, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        Column(Modifier.fillMaxSize().background(colors.bg).windowInsetsPadding(WindowInsets.safeDrawing)) {
            TopAppBar(
                title = { Text(stringResource(R.string.projects_clone_repo)) },
                navigationIcon = {
                    IconButton(onClick = onDismiss, enabled = !cloning) { Icon(Icons.Rounded.Close, contentDescription = stringResource(R.string.clone_close), tint = colors.textPrimary) }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = androidx.compose.ui.graphics.Color.Transparent, titleContentColor = colors.textPrimary),
            )
            Column(
                Modifier.weight(1f).fillMaxWidth().verticalScroll(rememberScrollState()).padding(horizontal = 16.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                CodegTextField(url, { url = it }, label = stringResource(R.string.clone_repository_url), placeholder = stringResource(R.string.clone_url_placeholder), keyboardType = KeyboardType.Uri, mono = true)
                Row(verticalAlignment = Alignment.Bottom, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    CodegTextField(parentDir, { parentDir = it }, label = stringResource(R.string.clone_parent_directory), mono = true, modifier = Modifier.weight(1f))
                    IconButton(onClick = { showBrowser = true }) { Icon(Icons.Rounded.CreateNewFolder, contentDescription = stringResource(R.string.clone_browse), tint = colors.accent) }
                }
                CodegTextField(username, { username = it }, label = stringResource(R.string.clone_username_optional))
                SecretField(password, { password = it }, label = stringResource(R.string.clone_password_token_optional))
                if (targetDir.isNotEmpty()) {
                    Text(stringResource(R.string.clone_clones_into, targetDir), fontFamily = FontFamily.Monospace, fontSize = 11.sp, color = colors.textTertiary)
                }
                error?.let {
                    Text(it, fontSize = 13.sp, color = colors.danger, modifier = Modifier.fillMaxWidth().clip(RoundedCornerShape(8.dp)).background(colors.danger.copy(alpha = 0.12f)).padding(10.dp))
                }
            }
            PrimaryButton(
                text = stringResource(R.string.clone_clone),
                onClick = {
                    cloning = true; error = null
                    scope.launch {
                        val creds = if (username.isBlank() && password.isBlank()) null else GitCredentials(username.trim(), password)
                        val err = clone(url.trim(), targetDir, creds)
                        cloning = false
                        if (err == null) onCloned() else error = err
                    }
                },
                enabled = canClone,
                loading = cloning,
                modifier = Modifier.padding(16.dp),
            )
        }
    }

    if (showBrowser) {
        DirectoryBrowserSheet(
            title = stringResource(R.string.clone_choose_directory),
            confirmLabel = stringResource(R.string.clone_select_this_folder),
            loadHome = loadHome,
            loadDirs = loadDirs,
            onSelect = { parentDir = it; showBrowser = false },
            onDismiss = { showBrowser = false },
        )
    }
}
