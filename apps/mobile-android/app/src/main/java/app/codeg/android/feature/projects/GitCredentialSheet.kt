package app.codeg.android.feature.projects

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.Key
import androidx.compose.material3.Checkbox
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import app.codeg.android.R
import app.codeg.android.core.designsystem.component.CodegTextField
import app.codeg.android.core.designsystem.component.PrimaryButton
import app.codeg.android.core.designsystem.component.SecretField
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.model.GitCredentials
import app.codeg.android.core.network.displayMessage
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch

/**
 * Prompts for git remote credentials when a push/pull/fetch fails to authenticate,
 * then hands them back to [ProjectDetailViewModel]'s credential-retry loop. Port of
 * iOS `GitCredentialSheet`:
 * - **GitHub** hosts: a personal-access-token field (with a "Generate" link); the
 *   token is validated and saved as a GitHub account here so a later op reuses it.
 * - **Other** hosts: username + password/token, optionally saved on success.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun GitCredentialSheet(
    viewModel: ProjectDetailViewModel,
    prompt: GitCredentialPrompt,
) {
    val colors = CodegTheme.colors
    val scope = rememberCoroutineScope()
    val uriHandler = LocalUriHandler.current

    var token by remember { mutableStateOf("") }
    var username by remember { mutableStateOf("") }
    var password by remember { mutableStateOf("") }
    var saveCredentials by remember { mutableStateOf(true) }
    var submitting by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }

    val hostLabel = prompt.host ?: "github.com"
    val serverUrl = prompt.host?.let { "https://$it" } ?: "https://github.com"
    val invalidTokenMsg = stringResource(R.string.cred_invalid_token)

    val canSubmit = when {
        submitting -> false
        prompt.github -> token.isNotBlank()
        else -> username.isNotBlank() && password.isNotBlank()
    }

    fun submit() {
        if (!canSubmit) return
        if (prompt.github) {
            submitting = true
            error = null
            scope.launch {
                try {
                    val trimmed = token.trim()
                    val validation = viewModel.validateGithubToken(serverUrl, trimmed)
                    if (validation == null || !validation.success) {
                        error = validation?.message?.takeIf { it.isNotBlank() } ?: invalidTokenMsg
                        submitting = false
                        return@launch
                    }
                    viewModel.saveGithubAccount(serverUrl, prompt.host, validation, trimmed)
                    viewModel.submitCredentials(
                        GitCredentialOutcome(GitCredentials(validation.username ?: "unknown", trimmed), saveAfterSuccess = false),
                    )
                } catch (e: CancellationException) {
                    throw e
                } catch (e: Exception) {
                    error = e.displayMessage()
                    submitting = false
                }
            }
        } else {
            viewModel.submitCredentials(
                GitCredentialOutcome(GitCredentials(username.trim(), password.trim()), saveAfterSuccess = saveCredentials),
            )
        }
    }

    // Safety net: a teardown without Cancel/Authenticate releases the pending
    // request so the awaiting push/pull/fetch doesn't hang (scoped to this prompt).
    DisposableEffect(prompt.id) {
        onDispose { viewModel.cancelCredentialsIfShowing(prompt.id) }
    }

    Dialog(onDismissRequest = { viewModel.cancelCredentials() }, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        Column(Modifier.fillMaxSize().background(colors.bg).windowInsetsPadding(WindowInsets.safeDrawing)) {
            TopAppBar(
                title = { Text(stringResource(if (prompt.github) R.string.cred_title_github else R.string.cred_title_generic)) },
                navigationIcon = {
                    IconButton(onClick = { viewModel.cancelCredentials() }, enabled = !submitting) {
                        Icon(Icons.Rounded.Close, contentDescription = stringResource(R.string.common_cancel), tint = colors.textPrimary)
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent, titleContentColor = colors.textPrimary),
            )
            Column(
                Modifier.weight(1f).fillMaxWidth().verticalScroll(rememberScrollState()).padding(horizontal = 16.dp),
                verticalArrangement = Arrangement.spacedBy(14.dp),
            ) {
                if (prompt.isRetry) {
                    Banner(stringResource(R.string.cred_retry_hint), WarnTint, colors.textSecondary)
                }
                if (prompt.github) {
                    Text(stringResource(R.string.cred_github_footer, hostLabel), fontSize = 12.sp, color = colors.textTertiary)
                    Row(verticalAlignment = Alignment.Bottom, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        SecretField(token, { token = it }, label = stringResource(R.string.cred_token_label), modifier = Modifier.weight(1f))
                        TextButton(onClick = {
                            val url = "$serverUrl/settings/tokens/new?description=codeg&scopes=repo,read:org,workflow,gist,read:user,user:email"
                            runCatching { uriHandler.openUri(url) }
                        }, enabled = !submitting) {
                            Text(stringResource(R.string.cred_generate), color = colors.accent, fontWeight = FontWeight.SemiBold)
                        }
                    }
                } else {
                    Text(stringResource(R.string.cred_generic_footer, hostLabel), fontSize = 12.sp, color = colors.textTertiary)
                    CodegTextField(username, { username = it }, label = stringResource(R.string.cred_username))
                    SecretField(password, { password = it }, label = stringResource(R.string.cred_password_label))
                    Row(
                        Modifier.fillMaxWidth().clip(RoundedCornerShape(10.dp)),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Checkbox(checked = saveCredentials, onCheckedChange = { saveCredentials = it }, enabled = !submitting)
                        Text(stringResource(R.string.cred_save), fontSize = 14.sp, color = colors.textPrimary)
                    }
                }
                error?.let { Banner(it, colors.danger, colors.danger) }
            }
            PrimaryButton(
                text = stringResource(R.string.cred_authenticate),
                onClick = { submit() },
                icon = Icons.Rounded.Key,
                enabled = canSubmit,
                loading = submitting,
                modifier = Modifier.padding(16.dp),
            )
        }
    }
}

private val WarnTint = Color(0xFFF0A030)

@Composable
private fun Banner(message: String, tint: Color, textColor: Color) {
    Text(
        message,
        fontSize = 13.sp,
        color = textColor,
        modifier = Modifier.fillMaxWidth().clip(RoundedCornerShape(10.dp)).background(tint.copy(alpha = 0.12f)).padding(12.dp),
    )
}
