package app.codeg.android.feature.sessiondetail

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.ArrowUpward
import androidx.compose.material.icons.rounded.Image
import androidx.compose.material.icons.rounded.Stop
import androidx.compose.material3.FilledIconButton
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import app.codeg.android.R
import app.codeg.android.core.designsystem.theme.CodegTheme

/**
 * The prompt composer pinned to the bottom of the session detail screen: a
 * multiline text field flanked by an insert ("+"), attach, and send / stop
 * action. The send button morphs between Send and Stop with a Material
 * scale-and-fade transition and an accent↔danger colour tween. All actions are
 * Material [FilledIconButton] / [FilledTonalIconButton]s, so they carry native
 * ripples and 40dp touch targets.
 */
@Composable
fun ComposeBar(
    text: String,
    onTextChange: (String) -> Unit,
    onSend: () -> Unit,
    isInFlight: Boolean,
    onStop: () -> Unit,
    modifier: Modifier = Modifier,
    onPlus: (() -> Unit)? = null,
    onAttach: (() -> Unit)? = null,
    canSendOverride: Boolean = false,
) {
    val colors = CodegTheme.colors
    val canSend = (text.isNotBlank() || canSendOverride) && !isInFlight
    val sendContainer by animateColorAsState(
        targetValue = if (isInFlight) colors.danger else colors.accent,
        animationSpec = tween(220),
        label = "send-container",
    )

    Column(modifier = modifier.fillMaxWidth()) {
        // A hairline setting the composer off from the timeline above it — the same
        // separator the bottom navigation bar uses, so every bottom-chrome surface is
        // delineated from its content the same way.
        HorizontalDivider(thickness = Dp.Hairline, color = colors.surfaceStroke)
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .background(colors.bgElevated.copy(alpha = 0.92f))
                .padding(horizontal = 10.dp, vertical = 8.dp),
            verticalAlignment = Alignment.Bottom,
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            if (onPlus != null) {
                FilledTonalIconButton(
                    onClick = onPlus,
                    colors = IconButtonDefaults.filledTonalIconButtonColors(
                        containerColor = colors.textPrimary.copy(alpha = 0.06f),
                        contentColor = colors.textSecondary,
                    ),
                ) {
                    Icon(Icons.Rounded.Add, contentDescription = stringResource(R.string.compose_insert), modifier = Modifier.size(22.dp))
                }
            }
            if (onAttach != null) {
                FilledTonalIconButton(
                    onClick = onAttach,
                    colors = IconButtonDefaults.filledTonalIconButtonColors(
                        containerColor = colors.textPrimary.copy(alpha = 0.06f),
                        contentColor = colors.textSecondary,
                    ),
                ) {
                    Icon(Icons.Rounded.Image, contentDescription = stringResource(R.string.compose_attach_image), modifier = Modifier.size(20.dp))
                }
            }
            Box(
                Modifier
                    .weight(1f)
                    .heightIn(min = 40.dp)
                    .background(colors.codeSurface, RoundedCornerShape(20.dp))
                    .border(0.5.dp, colors.surfaceStroke, RoundedCornerShape(20.dp))
                    .padding(horizontal = 14.dp, vertical = 9.dp),
                contentAlignment = Alignment.CenterStart,
            ) {
                if (text.isEmpty()) {
                    Text(
                        stringResource(R.string.compose_message_placeholder),
                        color = colors.textTertiary,
                        style = MaterialTheme.typography.bodyLarge,
                    )
                }
                BasicTextField(
                    value = text,
                    onValueChange = onTextChange,
                    textStyle = MaterialTheme.typography.bodyLarge.copy(color = colors.textPrimary),
                    cursorBrush = SolidColor(colors.accent),
                    maxLines = 6,
                    modifier = Modifier.fillMaxWidth(),
                )
            }

            FilledIconButton(
                onClick = { if (isInFlight) onStop() else onSend() },
                enabled = canSend || isInFlight,
                colors = IconButtonDefaults.filledIconButtonColors(
                    containerColor = sendContainer,
                    contentColor = colors.onAccent,
                    disabledContainerColor = colors.accent.copy(alpha = 0.4f),
                    disabledContentColor = colors.onAccent.copy(alpha = 0.7f),
                ),
            ) {
                AnimatedContent(
                    targetState = isInFlight,
                    transitionSpec = {
                        (fadeIn(tween(150)) + scaleIn(tween(150), initialScale = 0.6f)) togetherWith
                            (fadeOut(tween(120)) + scaleOut(tween(120), targetScale = 0.6f))
                    },
                    label = "send-stop-icon",
                ) { inFlight ->
                    Icon(
                        imageVector = if (inFlight) Icons.Rounded.Stop else Icons.Rounded.ArrowUpward,
                        contentDescription = if (inFlight) stringResource(R.string.compose_stop) else stringResource(R.string.compose_send),
                        modifier = Modifier.size(20.dp),
                    )
                }
            }
        }
    }
}
