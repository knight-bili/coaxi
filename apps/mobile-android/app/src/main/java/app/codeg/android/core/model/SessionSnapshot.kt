package app.codeg.android.core.model

import kotlinx.serialization.Serializable

/**
 * A live session's available slash command (`SessionSnapshot.availableCommands`).
 * Slash commands are not a dedicated endpoint — they come from the session
 * snapshot. Mirrors iOS `AvailableCommandInfo`.
 */
@Serializable
data class AvailableCommandInfo(
    val name: String,
    val description: String? = null,
    val inputHint: String? = null,
)

/**
 * Authoritative live session state (`acp_get_session_snapshot_by_conversation` /
 * `acp_get_session_snapshot`). Modeled with the fields the compose-insert sheet
 * and the agent-options sheet need; unknown fields are ignored. Mirrors iOS
 * `SessionSnapshot` (a subset).
 */
@Serializable
data class SessionSnapshot(
    val selectorsReady: Boolean = false,
    val availableCommands: List<AvailableCommandInfo> = emptyList(),
    val modes: SessionModeState? = null,
    val configOptions: List<SessionConfigOption> = emptyList(),
)

/** The current + available session modes. */
@Serializable
data class SessionModeState(
    val currentModeId: String? = null,
    val availableModes: List<SessionModeInfo> = emptyList(),
)

@Serializable
data class SessionModeInfo(
    val id: String,
    val name: String = "",
    val description: String? = null,
)

/** A configurable session option (e.g. reasoning effort), a `select` of values. */
@Serializable
data class SessionConfigOption(
    val id: String,
    val name: String = "",
    val description: String? = null,
    val category: String? = null,
    val kind: SessionConfigKind = SessionConfigKind(),
)

@Serializable
data class SessionConfigKind(
    val type: String = "select",
    val currentValue: String? = null,
    val options: List<SessionConfigSelectOption> = emptyList(),
    val groups: List<SessionConfigSelectGroup> = emptyList(),
)

@Serializable
data class SessionConfigSelectOption(
    val value: String,
    val name: String = "",
    val description: String? = null,
)

@Serializable
data class SessionConfigSelectGroup(
    val group: String = "",
    val name: String = "",
    val options: List<SessionConfigSelectOption> = emptyList(),
)

/** `acp_describe_agent_options` result (the per-agent catalog of modes + config). */
@Serializable
data class AgentOptionsSnapshot(
    val modes: SessionModeState? = null,
    val configOptions: List<SessionConfigOption> = emptyList(),
)
