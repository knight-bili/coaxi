package app.codeg.android.core.designsystem.component

import androidx.annotation.DrawableRes
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.size
import androidx.compose.material3.Icon
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.unit.Dp
import app.codeg.android.R
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.model.AgentType

/**
 * Per-agent brand visuals, mirroring the web client's `agent-icon.tsx` and iOS
 * `AgentType.iconAsset`/`accent`. The brand marks live in `res/drawable` as
 * vector drawables ported from the same SVGs the web and iOS clients ship.
 */
object AgentVisuals {

    /** The agent's brand mark as a vector drawable. */
    @DrawableRes
    fun icon(agent: AgentType): Int = when (agent) {
        AgentType.CLAUDE_CODE -> R.drawable.ic_agent_claude_code
        AgentType.CODEX -> R.drawable.ic_agent_codex
        AgentType.OPEN_CODE -> R.drawable.ic_agent_open_code
        AgentType.GEMINI -> R.drawable.ic_agent_gemini
        AgentType.OPEN_CLAW -> R.drawable.ic_agent_open_claw
        AgentType.CLINE -> R.drawable.ic_agent_cline
        AgentType.HERMES -> R.drawable.ic_agent_hermes
        AgentType.CODE_BUDDY -> R.drawable.ic_agent_code_buddy
        AgentType.KIMI_CODE -> R.drawable.ic_agent_kimi_code
        AgentType.PI -> R.drawable.ic_agent_pi
        AgentType.GROK -> R.drawable.ic_agent_grok
        AgentType.CURSOR -> R.drawable.ic_agent_cursor
    }

    /**
     * Whether the brand mark is a monochrome glyph the caller should tint (the
     * web's `MONO_ICONS` / iOS `iconIsTemplate`); the others carry their own
     * brand colors/gradients and render as-is.
     */
    fun iconIsTemplate(agent: AgentType): Boolean = when (agent) {
        AgentType.OPEN_CODE, AgentType.CLINE, AgentType.HERMES,
        AgentType.CODE_BUDDY, AgentType.GROK, AgentType.CURSOR,
        -> true
        AgentType.CLAUDE_CODE, AgentType.CODEX, AgentType.GEMINI,
        AgentType.OPEN_CLAW, AgentType.KIMI_CODE, AgentType.PI,
        -> false
    }

    /**
     * Accent colour for badges/avatars and the default tint of monochrome
     * brand marks (iOS `AgentType.accent`). Grok's and Cursor's marks are
     * monochrome brands, so their accent follows the theme — near-black in
     * light, near-white in dark — to stay legible on either surface.
     */
    fun accent(agent: AgentType, isDark: Boolean): Color = when (agent) {
        AgentType.CLAUDE_CODE -> Color(0xFFD98557) // claude clay
        AgentType.CODEX -> Color(0xFF73C7A8) // teal
        AgentType.OPEN_CODE -> Color(0xFF8C9EF2) // indigo
        AgentType.GEMINI -> Color(0xFF80B3FA) // blue
        AgentType.OPEN_CLAW -> Color(0xFFEB9E6B) // amber
        AgentType.CLINE -> Color(0xFF9EC780) // green
        AgentType.HERMES -> Color(0xFF9980D9) // violet
        AgentType.CODE_BUDDY -> Color(0xFF3378F5) // tencent blue
        AgentType.KIMI_CODE -> Color(0xFF1782FF) // moonshot blue
        AgentType.PI -> Color(0xFF383842) // pi slate
        AgentType.GROK, AgentType.CURSOR ->
            if (isDark) Color(0xFFEBEBEB) else Color(0xFF1F1F1F)
    }

    /** [accent] resolved against the current theme. */
    @Composable
    fun accent(agent: AgentType): Color = accent(agent, CodegTheme.colors.isDark)
}

/**
 * The per-agent brand icon (web `AgentIcon` / iOS `AgentIcon`): color agents
 * render their own colors/gradients from the vector asset; monochrome agents
 * are tinted with [tint] (the agent accent by default). The surrounding UI
 * usually supplies the visible agent name; the icon keeps its own description
 * for standalone uses.
 */
@Composable
fun AgentIcon(
    agent: AgentType,
    size: Dp,
    modifier: Modifier = Modifier,
    tint: Color? = null,
) {
    val painter = painterResource(AgentVisuals.icon(agent))
    val decorated = modifier
        .size(size)
        .clearAndSetSemantics { contentDescription = agent.displayName }
    if (AgentVisuals.iconIsTemplate(agent)) {
        Icon(
            painter = painter,
            contentDescription = null,
            tint = tint ?: AgentVisuals.accent(agent),
            modifier = decorated,
        )
    } else {
        Image(
            painter = painter,
            contentDescription = null,
            modifier = decorated,
        )
    }
}
