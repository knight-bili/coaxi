package app.codeg.android.feature.projects

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.CheckCircle
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.ErrorOutline
import androidx.compose.material.icons.rounded.WarningAmber
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.codeg.android.R
import app.codeg.android.core.designsystem.theme.CodegTheme

private val SuccessGreen = Color(0xFF4CBC75)
private val WarnAmber = Color(0xFFF0A030)

/**
 * Shown atop the Changes / Commits tabs: a busy row while a git operation runs,
 * otherwise the most recent [GitBanner] (dismissible). Flat tinted surfaces (not
 * glass) so it reads as a colored strip on the light backdrop. Port of iOS
 * `GitStatusStrip`.
 */
@Composable
internal fun GitStatusStrip(
    busy: Boolean,
    busyTitle: String?,
    banner: GitBanner?,
    onDismissBanner: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val colors = CodegTheme.colors
    when {
        busy -> Row(
            modifier.fillMaxWidth().clip(RoundedCornerShape(CodegTheme.dimens.radiusMd)).background(colors.bgElevated.copy(alpha = 0.6f)).padding(horizontal = 14.dp, vertical = 11.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            CircularProgressIndicator(Modifier.size(16.dp), color = colors.accent, strokeWidth = 2.dp)
            Text(busyTitle ?: stringResource(R.string.git_working), fontSize = 12.sp, color = colors.textSecondary)
        }

        banner != null -> {
            val tint = when (banner.kind) {
                GitBannerKind.SUCCESS -> SuccessGreen
                GitBannerKind.WARNING -> WarnAmber
                GitBannerKind.ERROR -> colors.danger
            }
            val icon = when (banner.kind) {
                GitBannerKind.SUCCESS -> Icons.Rounded.CheckCircle
                GitBannerKind.WARNING -> Icons.Rounded.WarningAmber
                GitBannerKind.ERROR -> Icons.Rounded.ErrorOutline
            }
            Row(
                modifier.fillMaxWidth().clip(RoundedCornerShape(CodegTheme.dimens.radiusMd)).background(tint.copy(alpha = 0.12f)).border(1.dp, tint.copy(alpha = 0.35f), RoundedCornerShape(CodegTheme.dimens.radiusMd)).padding(start = 14.dp, top = 8.dp, bottom = 8.dp, end = 6.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                Icon(icon, contentDescription = null, tint = tint, modifier = Modifier.size(16.dp))
                Text(banner.message, fontSize = 12.sp, color = colors.textSecondary, modifier = Modifier.weight(1f), maxLines = 3)
                IconButton(onClick = onDismissBanner, modifier = Modifier.size(28.dp)) {
                    Icon(Icons.Rounded.Close, contentDescription = stringResource(R.string.git_dismiss), tint = colors.textTertiary, modifier = Modifier.size(15.dp))
                }
            }
        }
    }
}
