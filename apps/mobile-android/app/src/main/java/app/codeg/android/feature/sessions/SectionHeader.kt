package app.codeg.android.feature.sessions

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.ExpandMore
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import app.codeg.android.R
import app.codeg.android.core.designsystem.component.CountBadge
import app.codeg.android.core.designsystem.component.SectionBadgeIcon
import app.codeg.android.core.designsystem.theme.CodegTheme

/**
 * A grouped-list section header shared by the Chats and Activity tabs: a tinted
 * circular [SectionBadgeIcon] + a **prominent** group title (`titleLarge`, larger
 * than the row titles so the grouping reads at a glance) + a trailing [CountBadge],
 * tappable to collapse/expand its rows with a chevron that flips ▾↔▴.
 *
 * The whole row is the toggle target (the native accordion idiom). When [collapsible]
 * is false (an empty group), it renders statically — no chevron, no tap.
 */
@Composable
fun CollapsibleSectionHeader(
    icon: ImageVector,
    tint: Color,
    label: String,
    count: Int,
    collapsed: Boolean,
    onToggle: () -> Unit,
    modifier: Modifier = Modifier,
    collapsible: Boolean = true,
) {
    val colors = CodegTheme.colors
    val rotation by animateFloatAsState(
        targetValue = if (collapsed) 0f else 180f,
        label = "section-chevron",
    )
    val toggleLabel = stringResource(
        if (collapsed) R.string.sessions_expand else R.string.sessions_collapse,
    )

    Row(
        modifier = modifier
            .fillMaxWidth()
            .padding(top = 10.dp) // inter-section gap, kept outside the ripple
            .clip(RoundedCornerShape(12.dp))
            .then(
                if (collapsible) {
                    Modifier.clickable(onClickLabel = toggleLabel, role = Role.Button, onClick = onToggle)
                } else {
                    Modifier
                },
            )
            .padding(horizontal = 8.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(11.dp),
    ) {
        // Same 26dp diameter as the row AgentAvatars (SessionRow), so the group icon
        // matches the agent icons and the big title lines up with the row titles.
        SectionBadgeIcon(icon = icon, tint = tint, size = 26.dp)
        Text(
            text = label,
            style = MaterialTheme.typography.titleLarge,
            color = colors.textPrimary,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f),
        )
        CountBadge(count)
        if (collapsible) {
            Icon(
                Icons.Rounded.ExpandMore,
                contentDescription = null,
                tint = colors.textTertiary,
                modifier = Modifier.size(22.dp).rotate(rotation),
            )
        }
    }
}
