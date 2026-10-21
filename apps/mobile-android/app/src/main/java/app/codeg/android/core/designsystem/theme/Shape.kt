package app.codeg.android.core.designsystem.theme

import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Shapes
import androidx.compose.ui.unit.dp

/**
 * Material3 shape scale mapped onto the iOS radii (sm 10 / md 14 / lg 20 / xl 26).
 * iOS uses `.continuous` (squircle) corners; `RoundedCornerShape` is the closest
 * Compose equivalent.
 */
val CodegShapes = Shapes(
    extraSmall = RoundedCornerShape(10.dp),
    small = RoundedCornerShape(10.dp),
    medium = RoundedCornerShape(14.dp),
    large = RoundedCornerShape(20.dp),
    extraLarge = RoundedCornerShape(26.dp),
)
