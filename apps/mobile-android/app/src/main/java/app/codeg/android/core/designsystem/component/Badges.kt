package app.codeg.android.core.designsystem.component

import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.tween
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.material3.MaterialTheme
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Folder
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.scale
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.model.AgentType
import app.codeg.android.core.model.ConversationStatus

private val Capsule = RoundedCornerShape(percent = 50)

/** Conversation-status tint (iOS `ConversationStatus.tint`). */
@Composable
fun statusColor(status: ConversationStatus): Color = when (status) {
    ConversationStatus.IN_PROGRESS -> Color(0xFF6BC7F2)
    ConversationStatus.PENDING_REVIEW -> Color(0xFFF5BD5C)
    ConversationStatus.COMPLETED -> Color(0xFF85D18F)
    ConversationStatus.CANCELLED, ConversationStatus.OTHER -> CodegTheme.colors.textTertiary
}

/** Short status label (iOS `ConversationStatus.label`). */
fun statusLabel(status: ConversationStatus): String = when (status) {
    ConversationStatus.IN_PROGRESS -> "Running"
    ConversationStatus.PENDING_REVIEW -> "Review"
    ConversationStatus.COMPLETED -> "Done"
    ConversationStatus.CANCELLED -> "Cancelled"
    ConversationStatus.OTHER -> "—"
}

/** Circular agent avatar (iOS `AgentAvatar`): accent wash + thin accent ring. */
@Composable
fun AgentAvatar(
    agent: AgentType,
    modifier: Modifier = Modifier,
    size: Dp = 36.dp,
) {
    val accent = AgentVisuals.accent(agent)
    Box(
        modifier
            .size(size)
            .clip(CircleShape)
            .background(accent.copy(alpha = 0.16f))
            .border(1.dp, accent.copy(alpha = 0.32f), CircleShape),
        contentAlignment = Alignment.Center,
    ) {
        AgentIcon(agent, size = size * 0.56f)
    }
}

/** Agent type pill: neutral monogram + short name in the agent accent. */
@Composable
fun AgentBadge(agent: AgentType, modifier: Modifier = Modifier) {
    val accent = AgentVisuals.accent(agent)
    Row(
        modifier
            .clip(Capsule)
            .background(accent.copy(alpha = 0.14f))
            .padding(horizontal = 8.dp, vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        AgentIcon(agent, size = 11.dp)
        Text(
            text = "  ${agent.shortName}",
            color = accent,
            fontSize = 11.sp,
            fontWeight = FontWeight.SemiBold,
        )
    }
}

/** Status pill (iOS `StatusBadge`): coloured dot + label. */
@Composable
fun StatusBadge(status: ConversationStatus, modifier: Modifier = Modifier) {
    val tint = statusColor(status)
    Row(
        modifier
            .clip(Capsule)
            .background(tint.copy(alpha = 0.14f))
            .padding(horizontal = 8.dp, vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.size(6.dp).clip(CircleShape).background(tint))
        Text(
            text = "  ${statusLabel(status)}",
            color = tint,
            fontSize = 11.sp,
            fontWeight = FontWeight.SemiBold,
        )
    }
}

/**
 * Circular, tinted section icon (iOS `SectionBadgeIcon`): a glyph centred on a
 * tinted circle that mirrors [AgentAvatar]'s shape, so a group header visually
 * rhymes with the agent avatars in its rows and — at the same 26dp diameter —
 * lines the header title up with the row titles beneath it.
 */
@Composable
fun SectionBadgeIcon(
    icon: ImageVector,
    tint: Color,
    modifier: Modifier = Modifier,
    size: Dp = 26.dp,
) {
    Box(
        modifier
            .size(size)
            .clip(CircleShape)
            .background(tint.copy(alpha = 0.16f))
            .border(1.dp, tint.copy(alpha = 0.32f), CircleShape),
        contentAlignment = Alignment.Center,
    ) {
        Icon(icon, contentDescription = null, tint = tint, modifier = Modifier.size(size * 0.5f))
    }
}

/**
 * iOS-Settings-style icon badge (`SettingsIconBadge`): a glyph in [onAccent]
 * centred on an accent-filled rounded square. Used as the leading affordance on
 * grouped Settings rows so they read as a polished, tappable list.
 */
@Composable
fun SettingsIconBadge(
    icon: ImageVector,
    modifier: Modifier = Modifier,
    tint: Color = CodegTheme.colors.accent,
    size: Dp = 28.dp,
) {
    val onTint = CodegTheme.colors.onAccent
    Box(
        modifier
            .size(size)
            .clip(RoundedCornerShape(7.dp))
            .background(tint),
        contentAlignment = Alignment.Center,
    ) {
        Icon(icon, contentDescription = null, tint = onTint, modifier = Modifier.size(size * 0.58f))
    }
}

/** Square folder badge (iOS `FolderBadge`). */
@Composable
fun FolderBadge(
    color: Color,
    modifier: Modifier = Modifier,
    size: Dp = 40.dp,
) {
    Box(
        modifier
            .size(size)
            .clip(RoundedCornerShape(size * 0.3f))
            .background(color.copy(alpha = 0.18f))
            .border(1.dp, color.copy(alpha = 0.40f), RoundedCornerShape(size * 0.3f)),
        contentAlignment = Alignment.Center,
    ) {
        Icon(
            imageVector = Icons.Rounded.Folder,
            contentDescription = null,
            tint = color,
            modifier = Modifier.size(size * 0.45f),
        )
    }
}

/** Small count pill (iOS `CountBadge`). */
@Composable
fun CountBadge(count: Int, modifier: Modifier = Modifier) {
    Text(
        text = count.toString(),
        color = CodegTheme.colors.textTertiary,
        fontSize = 11.sp,
        fontWeight = FontWeight.Bold,
        modifier = modifier
            .clip(Capsule)
            .background(MaterialTheme.colorScheme.onSurface.copy(alpha = 0.08f))
            .padding(horizontal = 6.dp, vertical = 1.dp),
    )
}

/** A pulsing "live" indicator (iOS `LivePulse`): accent dot + expanding ring. */
@Composable
fun LivePulse(modifier: Modifier = Modifier, dotSize: Dp = 8.dp) {
    val accent = CodegTheme.colors.accent
    val transition = rememberInfiniteTransition(label = "live-pulse")
    val progress by transition.animateFloat(
        initialValue = 0f,
        targetValue = 1f,
        animationSpec = infiniteRepeatable(
            animation = tween(durationMillis = 1100),
            repeatMode = RepeatMode.Restart,
        ),
        label = "pulse-progress",
    )
    Box(modifier.size(dotSize), contentAlignment = Alignment.Center) {
        Box(
            Modifier
                .size(dotSize)
                .scale(1f + progress * 1.2f)
                .drawBehind {
                    drawCircle(color = accent.copy(alpha = (1f - progress) * 0.8f))
                },
        )
        Box(Modifier.size(dotSize).clip(CircleShape).background(accent))
    }
}
