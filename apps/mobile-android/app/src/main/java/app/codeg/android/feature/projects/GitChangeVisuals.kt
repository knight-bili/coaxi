package app.codeg.android.feature.projects

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.model.GitChange

/** Per-category tint for a working-tree change, shared by the Changes tab + commit sheet. */
internal val GitChangeColors: Map<GitChange, Color> = mapOf(
    GitChange.UNTRACKED to Color(0xFF85D18F),
    GitChange.ADDED to Color(0xFF85D18F),
    GitChange.MODIFIED to Color(0xFFF5BD5C),
    GitChange.TYPE_CHANGED to Color(0xFFF5BD5C),
    GitChange.DELETED to Color(0xFFF57575),
    GitChange.RENAMED to Color(0xFFC28CFF),
    GitChange.COPIED to Color(0xFFC28CFF),
    GitChange.CONFLICTED to Color(0xFFFF9E4D),
)

/** The colored letter badge (M / A / U / …) for a change category. */
@Composable
internal fun GitChangeBadge(change: GitChange, modifier: Modifier = Modifier) {
    val tint = GitChangeColors[change] ?: CodegTheme.colors.textTertiary
    Box(
        modifier.size(20.dp).clip(RoundedCornerShape(5.dp)).background(tint.copy(alpha = 0.16f)),
        contentAlignment = Alignment.Center,
    ) {
        Text(change.badge, fontSize = 10.sp, fontWeight = FontWeight.Bold, color = tint)
    }
}
