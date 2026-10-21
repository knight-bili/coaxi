package app.codeg.android.core.designsystem.theme

import androidx.compose.runtime.Immutable
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/** Corner radii and spacing tokens, ported from iOS `Theme.Radius` / `Theme.Layout`. */
@Immutable
data class CodegDimens(
    val radiusXl: Dp = 26.dp,
    val radiusLg: Dp = 20.dp,
    val radiusMd: Dp = 14.dp,
    val radiusSm: Dp = 10.dp,
    val screenHMargin: Dp = 16.dp,
    val blockSpacing: Dp = 12.dp,
    val hairlineWidth: Dp = 0.75.dp,
)
