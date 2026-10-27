import Foundation

/// Per-agent skill files (CLAUDE.md / SKILL.md), at global or project scope.
/// Mirrors the server's agent-skill types. Scope/layout are string enum *values*
/// (unaffected by the snake_case key decoder), so raw values match directly.

enum AgentSkillScope: String, Codable, Hashable, Sendable {
    case global
    case project
}

enum AgentSkillLayout: String, Codable, Hashable, Sendable {
    case markdownFile = "markdown_file"
    case skillDirectory = "skill_directory"
}

struct AgentSkillItem: Decodable, Identifiable, Hashable, Sendable {
    let id: String
    let name: String
    let scope: AgentSkillScope
    let layout: AgentSkillLayout
    let path: String
    let description: String?
    let readOnly: Bool
}

struct AgentSkillLocation: Decodable, Hashable, Sendable {
    let scope: AgentSkillScope
    let path: String
    let exists: Bool
}

struct AgentSkillsListResult: Decodable, Sendable {
    let supported: Bool
    let message: String?
    let locations: [AgentSkillLocation]
    let skills: [AgentSkillItem]
}

struct AgentSkillContent: Decodable, Sendable {
    let skill: AgentSkillItem
    let content: String
}
