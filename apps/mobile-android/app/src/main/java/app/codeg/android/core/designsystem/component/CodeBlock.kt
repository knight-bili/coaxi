package app.codeg.android.core.designsystem.component

import androidx.compose.animation.Crossfade
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.ContentCopy
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
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
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.codeg.android.R
import app.codeg.android.core.designsystem.theme.CodegTheme
import kotlinx.coroutines.delay

private val SuccessGreen = Color(0xFF85D18F)

/**
 * A fenced code block: a header (language label + copy button) over a
 * horizontally-scrollable monospaced body. Long blocks collapse at
 * [collapseAfter] lines with a "Show N more lines" toggle. Dependency-free (no
 * syntax highlighting), matching the iOS `CodeBlockView`.
 */
@Composable
fun CodeBlock(
    code: String,
    modifier: Modifier = Modifier,
    language: String? = null,
    collapseAfter: Int = 20,
) {
    val colors = CodegTheme.colors
    val lines = remember(code) { code.split("\n") }
    val collapsible = lines.size > collapseAfter
    var expanded by remember(code) { mutableStateOf(false) }
    val shown = if (collapsible && !expanded) {
        lines.take(collapseAfter).joinToString("\n")
    } else {
        code
    }

    Column(
        modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(10.dp))
            .background(colors.codeSurface)
            .border(0.5.dp, colors.hairline, RoundedCornerShape(10.dp)),
    ) {
        Row(
            Modifier.fillMaxWidth().padding(start = 10.dp, end = 6.dp, top = 4.dp, bottom = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceBetween,
        ) {
            Text(
                text = (language ?: "").ifBlank { stringResource(R.string.code_default_language) },
                fontFamily = FontFamily.Monospace,
                fontSize = 11.sp,
                color = colors.textTertiary,
            )
            CopyButton(text = code)
        }
        Row(
            Modifier
                .fillMaxWidth()
                .horizontalScroll(rememberScrollState())
                .padding(horizontal = 10.dp, vertical = 8.dp),
        ) {
            Text(
                text = shown,
                fontFamily = FontFamily.Monospace,
                fontSize = 12.5.sp,
                lineHeight = 18.sp,
                color = colors.textPrimary,
            )
        }
        if (collapsible) {
            Text(
                text = if (expanded) stringResource(R.string.code_show_less) else stringResource(R.string.code_show_more_lines, lines.size - collapseAfter),
                style = MaterialTheme.typography.labelSmall,
                color = colors.accent,
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable { expanded = !expanded }
                    .padding(horizontal = 10.dp, vertical = 6.dp),
            )
        }
    }
}

/** A copy-to-clipboard icon button that flips to a check for ~1.6s (iOS `CopyButton`). */
@Composable
fun CopyButton(text: String, modifier: Modifier = Modifier) {
    val clipboard = LocalClipboardManager.current
    val colors = CodegTheme.colors
    var copied by remember { mutableStateOf(false) }
    LaunchedEffect(copied) {
        if (copied) {
            delay(1600)
            copied = false
        }
    }
    IconButton(
        onClick = { clipboard.setText(AnnotatedString(text)); copied = true },
        modifier = modifier.size(32.dp),
    ) {
        Crossfade(targetState = copied, label = "copy-icon") { done ->
            Icon(
                imageVector = if (done) Icons.Rounded.Check else Icons.Rounded.ContentCopy,
                contentDescription = stringResource(R.string.code_copy),
                tint = if (done) SuccessGreen else colors.textTertiary,
                modifier = Modifier.size(16.dp),
            )
        }
    }
}
