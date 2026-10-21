package app.codeg.android.core.designsystem.component

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import app.codeg.android.core.designsystem.theme.CodegTheme

/** Centered empty-state (iOS `EmptyStateView`): icon, title, message, optional CTA. */
@Composable
fun EmptyState(
    icon: ImageVector,
    title: String,
    message: String,
    modifier: Modifier = Modifier,
    actionLabel: String? = null,
    onAction: (() -> Unit)? = null,
) {
    Column(
        modifier = modifier.fillMaxWidth().padding(32.dp).widthIn(max = 320.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        Icon(icon, contentDescription = null, tint = CodegTheme.colors.textTertiary, modifier = Modifier.size(40.dp))
        Text(title, style = MaterialTheme.typography.titleSmall, color = CodegTheme.colors.textPrimary)
        Text(
            message,
            style = MaterialTheme.typography.bodyMedium,
            color = CodegTheme.colors.textSecondary,
            textAlign = TextAlign.Center,
        )
        if (actionLabel != null && onAction != null) {
            Button(
                onClick = onAction,
                colors = ButtonDefaults.buttonColors(
                    containerColor = CodegTheme.colors.accent,
                    contentColor = CodegTheme.colors.onAccent,
                ),
            ) { Text(actionLabel) }
        }
    }
}

/** Centered loading spinner with a label (iOS `LoadingView`). */
@Composable
fun LoadingView(message: String, modifier: Modifier = Modifier) {
    Column(
        modifier = modifier.fillMaxWidth().padding(32.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        CircularProgressIndicator(color = CodegTheme.colors.accent)
        Text(message, style = MaterialTheme.typography.bodyMedium, color = CodegTheme.colors.textSecondary)
    }
}

/** Centered error with a retry (iOS `InlineErrorView`). */
@Composable
fun InlineError(
    icon: ImageVector,
    title: String,
    message: String,
    onRetry: () -> Unit,
    modifier: Modifier = Modifier,
    retryLabel: String = "Retry",
) {
    Column(
        modifier = modifier.fillMaxWidth().padding(28.dp).widthIn(max = 340.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Icon(icon, contentDescription = null, tint = CodegTheme.colors.danger, modifier = Modifier.size(32.dp))
        Text(title, style = MaterialTheme.typography.titleSmall, color = CodegTheme.colors.textPrimary)
        Text(
            message,
            style = MaterialTheme.typography.bodyMedium,
            color = CodegTheme.colors.textSecondary,
            textAlign = TextAlign.Center,
        )
        FilledTonalButton(
            onClick = onRetry,
            colors = ButtonDefaults.filledTonalButtonColors(
                containerColor = CodegTheme.colors.accent.copy(alpha = 0.16f),
                contentColor = CodegTheme.colors.accent,
            ),
        ) { Text(retryLabel) }
    }
}
