package app.codeg.android.feature.server

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import app.codeg.android.R
import app.codeg.android.core.designsystem.component.CodegTextField
import app.codeg.android.core.designsystem.component.PrimaryButton
import app.codeg.android.core.designsystem.component.SecretField
import app.codeg.android.core.designsystem.theme.CodegTheme

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ServerEditorScreen(
    onDone: () -> Unit,
    viewModel: ServerEditorViewModel = hiltViewModel(),
) {
    val colors = CodegTheme.colors
    Scaffold(
        containerColor = Color.Transparent,
        topBar = {
            TopAppBar(
                title = {
                    Text(stringResource(if (viewModel.isEditing) R.string.server_edit else R.string.server_add))
                },
                navigationIcon = {
                    IconButton(onClick = onDone) {
                        Icon(Icons.AutoMirrored.Rounded.ArrowBack, contentDescription = stringResource(R.string.common_cancel), tint = colors.textSecondary)
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = Color.Transparent,
                    titleContentColor = colors.textPrimary,
                ),
            )
        },
    ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(padding)
                .padding(horizontal = 16.dp, vertical = 8.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            SectionHeader(stringResource(R.string.server_section_connection))
            CodegTextField(
                value = viewModel.name,
                onValueChange = viewModel::onNameChange,
                label = stringResource(R.string.server_name),
            )
            CodegTextField(
                value = viewModel.url,
                onValueChange = viewModel::onUrlChange,
                label = stringResource(R.string.server_url),
                placeholder = stringResource(R.string.server_url_placeholder),
                keyboardType = KeyboardType.Uri,
                mono = true,
            )

            SectionHeader(stringResource(R.string.server_section_auth))
            SecretField(
                value = viewModel.token,
                onValueChange = viewModel::onTokenChange,
                label = stringResource(R.string.server_token),
            )

            PrimaryButton(
                text = stringResource(R.string.server_test_connection),
                onClick = viewModel::testConnection,
                loading = viewModel.test is TestState.Testing,
                enabled = viewModel.url.isNotBlank() && viewModel.token.isNotBlank(),
            )
            when (val test = viewModel.test) {
                is TestState.Success -> Text(
                    stringResource(R.string.server_connection_ok, test.version),
                    style = MaterialTheme.typography.bodySmall,
                    color = Color(0xFF85D18F),
                    fontFamily = FontFamily.Monospace,
                )
                is TestState.Failure -> Text(
                    stringResource(R.string.server_connection_failed, test.message),
                    style = MaterialTheme.typography.bodySmall,
                    color = colors.danger,
                )
                else -> Unit
            }

            PrimaryButton(
                text = stringResource(R.string.server_save),
                onClick = { viewModel.save(onDone) },
                enabled = viewModel.canSave,
            )
        }
    }
}

@Composable
private fun SectionHeader(text: String) {
    Text(
        text = text.uppercase(),
        style = MaterialTheme.typography.labelSmall,
        color = CodegTheme.colors.textTertiary,
        modifier = Modifier.padding(start = 4.dp),
    )
}
