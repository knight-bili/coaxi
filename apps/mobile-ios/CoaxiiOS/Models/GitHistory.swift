import SwiftUI

/// One commit from `git_log` (Rust `GitLogEntry`). `date` is an ISO8601 string
/// (`git --format=%aI`); `files` is the per-file change summary so a commit's
/// touched files render without a second request. `pushed` is `nil` when the
/// branch has no upstream to compare against.
struct GitLogEntry: Decodable, Identifiable, Hashable, Sendable {
    let hash: String
    let fullHash: String
    let author: String
    let date: String
    let message: String
    let files: [GitLogFileChange]
    let pushed: Bool?

    var id: String { fullHash }

    /// Parsed author date for relative formatting (nil if unparseable).
    var authoredDate: Date? { ISO8601.parse(date) }

    /// First non-empty line of the commit message (the subject).
    var subject: String {
        message.split(whereSeparator: \.isNewline).first.map(String.init)
            ?? message.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    /// Everything after the subject line, trimmed — empty when there's no body.
    var body: String {
        let lines = message.split(separator: "\n", omittingEmptySubsequences: false)
        guard lines.count > 1 else { return "" }
        return lines.dropFirst().joined(separator: "\n")
            .trimmingCharacters(in: .whitespacesAndNewlines)
    }

    var totalAdditions: Int { files.reduce(0) { $0 + $1.additions } }
    var totalDeletions: Int { files.reduce(0) { $0 + $1.deletions } }
}

/// One file touched by a commit (Rust `GitLogFileChange`).
struct GitLogFileChange: Decodable, Hashable, Sendable {
    let path: String
    let status: String
    let additions: Int
    let deletions: Int

    var change: GitChange { GitChange(code: status) }
}

/// Response of `git_log` (Rust `GitLogResult`). `hasUpstream` says whether the
/// branch tracks a remote (so the per-commit `pushed` flag is meaningful).
struct GitLogResult: Decodable, Sendable {
    let entries: [GitLogEntry]
    let hasUpstream: Bool
}

/// Response of `git_list_all_branches` (Rust `GitBranchList`). `local` / `remote`
/// are branch names; `worktreeBranches` are the local branches already checked
/// out in *another* worktree — those can't be checked out in this working tree,
/// so the picker shows them disabled. (snake_case `worktree_branches` is decoded
/// to camelCase by the shared `.convertFromSnakeCase` decoder.)
struct GitBranchList: Decodable, Sendable {
    let local: [String]
    let remote: [String]
    let worktreeBranches: [String]
}

/// Response of `resolve_worktree_folder` (Rust `WorktreeResolution`): where a
/// branch is currently checked out.
/// - `path == nil` → the branch isn't checked out in any worktree (check it out
///   in the repo root).
/// - `path != nil, folderId == nil` → it lives in a worktree directory that
///   isn't a registered folder yet (register it via `open_worktree_folder`).
/// - `path != nil, folderId != nil` → it lives in an already-registered folder
///   (navigate there).
struct WorktreeResolution: Decodable, Sendable {
    let path: String?
    let folderId: Int?
}

/// A configured git remote (`git_list_remotes` / `git_push_info` — Rust
/// `GitRemote`). `git remote -v` lists fetch *and* push entries, so the same
/// `name` can appear twice; dedupe by name when displaying.
struct GitRemote: Decodable, Hashable, Sendable {
    let name: String
    let url: String
}

/// Response of `git_push_info` (Rust `GitPushInfo`): the current branch, the
/// repo's remotes, and which remote the branch tracks (nil = no upstream). Drives
/// the Commits tab's "branch → remote" header and the no-remote disabled state.
struct GitPushInfo: Decodable, Sendable {
    let branch: String
    let remotes: [GitRemote]
    let trackingRemote: String?

    /// Remotes deduped by name (git lists fetch+push rows separately), order
    /// preserved.
    var uniqueRemotes: [GitRemote] {
        var seen = Set<String>()
        return remotes.filter { seen.insert($0.name).inserted }
    }
}

/// Response of `git_commit` (Rust `GitCommitResult`).
struct GitCommitResult: Decodable, Sendable {
    let committedFiles: Int
}

/// Response of `git_push` (Rust `GitPushResult`). `upstreamSet` is true when the
/// push also established the branch's upstream (first push of a new branch).
struct GitPushResult: Decodable, Sendable {
    let pushedCommits: Int
    let upstreamSet: Bool
}

/// Response of `git_pull` (Rust `GitPullResult`). `conflict` is present when the
/// merge produced conflicts the user must resolve.
struct GitPullResult: Decodable, Sendable {
    let updatedFiles: Int
    let conflict: GitConflictInfo?
}

/// Conflict detail attached to a pull/merge result (Rust `GitConflictInfo`).
/// `operation` is the in-progress git operation (e.g. "merge"); `conflictedFiles`
/// are repo-relative paths.
struct GitConflictInfo: Decodable, Sendable {
    let hasConflicts: Bool
    let conflictedFiles: [String]
    let operation: String
    let upstreamCommit: String?
}

/// One working-tree change from `git_status` (Rust `GitStatusEntry`). `status`
/// is the trimmed two-char porcelain code (e.g. `M`, `??`, `A`, `R`); `file` is
/// the path relative to the repo root.
struct GitStatusEntry: Decodable, Identifiable, Hashable, Sendable {
    let status: String
    let file: String

    var id: String { file }
    var change: GitChange { GitChange(code: status) }

    /// The current path. Porcelain rename/copy rows arrive as `old -> new`;
    /// callers want the right-hand (current) path for display, preview, and
    /// `git_diff`. Plain entries return `file` unchanged.
    var path: String {
        if let range = file.range(of: " -> ") {
            return String(file[range.upperBound...])
        }
        return file
    }

    /// The prior path of a rename/copy (left of `->`), or `nil` for a plain
    /// change — lets the row show a "from …" hint.
    var renamedFrom: String? {
        guard let range = file.range(of: " -> ") else { return nil }
        return String(file[..<range.lowerBound])
    }
}

// MARK: - Status interpretation

/// A normalized git change category, derived from porcelain status codes (from
/// `git_status`) or name-status letters (from `git_log`). Gives each a badge
/// letter, a label, and a conventional tint so the Changes/Commits views read
/// at a glance.
enum GitChange: Hashable, Sendable {
    case added, modified, deleted, renamed, copied, untracked, conflicted, typeChanged, other

    /// Interpret a trimmed status code by its most significant character.
    /// Conflict markers win, then untracked, then add/delete/rename/copy/type,
    /// then modified.
    init(code rawCode: String) {
        let code = rawCode.trimmingCharacters(in: .whitespaces)
        if code.isEmpty { self = .other; return }
        if code == "??" { self = .untracked; return }
        if code == "!!" { self = .other; return }
        let chars = Set(code)
        if chars.contains("U") || code == "AA" || code == "DD" { self = .conflicted; return }
        if chars.contains("R") { self = .renamed; return }
        if chars.contains("C") { self = .copied; return }
        if chars.contains("A") { self = .added; return }
        if chars.contains("D") { self = .deleted; return }
        if chars.contains("T") { self = .typeChanged; return }
        if chars.contains("M") { self = .modified; return }
        self = .other
    }

    /// Single-character badge glyph.
    var letter: String {
        switch self {
        case .added: "A"
        case .modified: "M"
        case .deleted: "D"
        case .renamed: "R"
        case .copied: "C"
        case .untracked: "U"
        case .conflicted: "!"
        case .typeChanged: "T"
        case .other: "•"
        }
    }

    var label: LocalizedStringKey {
        switch self {
        case .added: "Added"
        case .modified: "Modified"
        case .deleted: "Deleted"
        case .renamed: "Renamed"
        case .copied: "Copied"
        case .untracked: "Untracked"
        case .conflicted: "Conflicted"
        case .typeChanged: "Type changed"
        case .other: "Changed"
        }
    }

    /// Conventional tint: new = green, modified = amber, deleted = red,
    /// renamed/copied = purple, conflicted = orange.
    var tint: Color {
        switch self {
        case .added, .untracked: Color(red: 0.40, green: 0.80, blue: 0.52)
        case .modified, .typeChanged: Theme.warning
        case .deleted: Color(red: 0.95, green: 0.45, blue: 0.45)
        case .renamed, .copied: Color(red: 0.70, green: 0.56, blue: 0.98)
        case .conflicted: Color(red: 0.98, green: 0.56, blue: 0.30)
        case .other: Color.secondary
        }
    }
}
