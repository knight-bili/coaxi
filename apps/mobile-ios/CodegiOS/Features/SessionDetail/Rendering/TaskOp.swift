import Foundation

// Pure (view-free) parsing for the harness task-management tools — `TaskCreate`,
// `TaskUpdate`, `TaskList`, `TaskGet` — so a run of them renders as one evolving
// to-do checklist (Codex-mobile style) instead of raw JSON / log dumps. The
// outputs are PLAIN TEXT in a stable format (not JSON), so parsing keys off both
// the JSON input args and these text shapes:
//
//   TaskCreate  in {subject,description,activeForm}   out "Task #1 created successfully: <subject>"
//   TaskUpdate  in {taskId,status,subject?,...}       out "Updated task #1 status"
//   TaskList    in {}                                 out "#1 [completed] <subject>\n#3 [pending] …"
//   TaskGet     in {taskId}                           out "Task #3: <subject>\nStatus: in_progress\nDescription: …"
//
// `TaskListBuild` flattens a group of ops into a deduped, in-order checklist
// (last write wins per task id) and resolves an update's missing title from a
// nearby create/list/get in the same group.

// MARK: - Status

/// A task's lifecycle state, normalized from the assorted strings the tools emit.
enum TaskItemStatus: Equatable {
    case pending
    case inProgress
    case completed
    case deleted
    case blocked

    /// Map a free-form status string (`"in_progress"`, `"In Progress"`, `"done"`,
    /// …) to a case. Unknown / empty → `.pending`.
    init(parsing raw: String) {
        let s = raw.lowercased().trimmingCharacters(in: .whitespacesAndNewlines)
            .replacingOccurrences(of: "_", with: " ")
        switch s {
        case "completed", "complete", "done", "finished":
            self = .completed
        case "in progress", "inprogress", "active", "running", "started":
            self = .inProgress
        case "deleted", "removed", "cancelled", "canceled", "dropped":
            self = .deleted
        case "blocked", "waiting", "on hold":
            self = .blocked
        default:
            self = .pending
        }
    }
}

// MARK: - One parsed operation

/// One parsed task-management call.
struct TaskOp: Equatable {
    enum Kind: Equatable { case create, update, get, list }

    let kind: Kind
    /// The affected task id (digits only, without the leading `#`). `nil` for a
    /// `list`, or a `create` whose output hasn't streamed an id yet (live).
    let id: String?
    let subject: String?
    let description: String?
    /// The resulting / target status. `create` is always `.pending`; an `update`
    /// with no `status` arg (e.g. a pure `addBlockedBy`) carries `nil`.
    let status: TaskItemStatus?
    /// Populated only for `.list`: one entry per listed task.
    let listRows: [TaskRowData]
}

/// A single task as seen in a `TaskList` output line.
struct TaskRowData: Equatable {
    let id: String?
    let status: TaskItemStatus
    let subject: String?
}

// MARK: - Parse a tool call → TaskOp

enum TaskOpParse {

    /// Parse one task-management tool call into a `TaskOp`, or `nil` if the name
    /// isn't a recognized task tool. Tolerant of missing output (live, still
    /// running) — it parses whatever is present.
    static func parse(name: String, input: String?, output: String?) -> TaskOp? {
        guard let kind = kind(of: name) else { return nil }
        let args = ToolDerive.effectiveArgs(ToolDerive.parseJSON(input))
        let out = output?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""

        switch kind {
        case .create:
            let subject = str(args, ["subject", "title", "content"]) ?? createdSubject(from: out)
            return TaskOp(kind: .create,
                          id: createdID(from: out),
                          subject: subject,
                          description: str(args, ["description", "details"]),
                          status: .pending,
                          listRows: [])

        case .update:
            let statusStr = str(args, ["status", "state"])
            return TaskOp(kind: .update,
                          id: idStr(args, ["taskId", "task_id", "id"]) ?? updatedID(from: out),
                          subject: str(args, ["subject", "title"]),
                          description: str(args, ["description", "details"]),
                          status: statusStr.map(TaskItemStatus.init(parsing:)),
                          listRows: [])

        case .get:
            let detail = parseGet(out)
            return TaskOp(kind: .get,
                          id: detail.id ?? idStr(args, ["taskId", "task_id", "id"]),
                          subject: detail.subject,
                          description: detail.description,
                          status: detail.status,
                          listRows: [])

        case .list:
            return TaskOp(kind: .list, id: nil, subject: nil, description: nil,
                          status: nil, listRows: parseList(out))
        }
    }

    // MARK: name → kind

    static func kind(of name: String) -> TaskOp.Kind? {
        var n = name.lowercased()
        if let r = n.range(of: "__", options: .backwards) { n = String(n[r.upperBound...]) }
        n = n.replacingOccurrences(of: "_", with: "")
        switch n {
        case "taskcreate", "createtask", "addtask": return .create
        case "taskupdate", "updatetask": return .update
        case "tasklist", "listtasks": return .list
        case "taskget", "gettask": return .get
        default: return nil
        }
    }

    // MARK: text shapes

    /// `Task #1 created successfully: …` → "1".
    private static func createdID(from output: String) -> String? {
        Rx.capture(output, #"[Tt]ask\s+#(\d+)\s+created"#, groups: 1)?.first.flatMap(nonEmpty)
    }

    /// `Task #1 created successfully: <subject>` → "<subject>" (output fallback
    /// when the input lacked a subject).
    private static func createdSubject(from output: String) -> String? {
        Rx.capture(output, #"[Tt]ask\s+#\d+\s+created[^:]*:\s*(.+)"#, groups: 1)?
            .first.flatMap(nonEmpty)
    }

    /// `Updated task #1 status` → "1".
    private static func updatedID(from output: String) -> String? {
        Rx.capture(output, #"[Tt]ask\s+#(\d+)"#, groups: 1)?.first.flatMap(nonEmpty)
    }

    private struct GetDetail { var id: String?; var subject: String?; var status: TaskItemStatus?; var description: String? }

    /// Parse a `TaskGet` body: `Task #N: <subject>` / `Status: <st>` /
    /// `Description: <rest…>` (description may span the remaining lines).
    private static func parseGet(_ output: String) -> GetDetail {
        var d = GetDetail()
        if let caps = Rx.capture(output, #"[Tt]ask\s+#(\d+):\s*(.*)"#, groups: 2) {
            d.id = nonEmpty(caps[0])
            d.subject = nonEmpty(caps[1])
        }
        if let st = Rx.capture(output, #"(?m)^Status:\s*(.+)$"#, groups: 1)?.first.flatMap(nonEmpty) {
            d.status = TaskItemStatus(parsing: st)
        }
        // Everything after "Description:" to the end (so a multi-line description
        // is kept whole).
        if let desc = Rx.capture(output, #"Description:\s*([\s\S]+)"#, groups: 1)?.first.flatMap(nonEmpty) {
            d.description = desc.trimmingCharacters(in: .whitespacesAndNewlines)
        }
        return d
    }

    /// Parse `TaskList` lines `#N [status] subject` (lenient: skips lines that
    /// don't match the shape, e.g. a header or a blank line).
    private static func parseList(_ output: String) -> [TaskRowData] {
        output.split(separator: "\n", omittingEmptySubsequences: true).compactMap { line in
            let s = line.trimmingCharacters(in: .whitespaces)
            guard let caps = Rx.capture(s, #"^#(\d+)\s*\[([^\]]*)\]\s*(.*)$"#, groups: 3) else { return nil }
            return TaskRowData(id: nonEmpty(caps[0]),
                               status: TaskItemStatus(parsing: caps[1]),
                               subject: nonEmpty(caps[2]))
        }
    }

    // MARK: helpers

    private static func str(_ args: [String: Any]?, _ keys: [String]) -> String? {
        for k in keys { if let v = args?[k] as? String, !v.trimmingCharacters(in: .whitespaces).isEmpty { return v } }
        return nil
    }

    /// A task id from either a JSON string (`"1"`) or a JSON number (`1`) — a
    /// numbered-task API may serialize `taskId` either way, and on the live path the
    /// id is the only handle for same-group title resolution before output arrives.
    private static func idStr(_ args: [String: Any]?, _ keys: [String]) -> String? {
        for k in keys {
            guard let v = args?[k] else { continue }
            if let s = v as? String, !s.trimmingCharacters(in: .whitespaces).isEmpty { return s }
            if let n = v as? NSNumber, !ToolJSONFormat.isBoolean(n) { return n.stringValue }
        }
        return nil
    }

    private static func nonEmpty(_ s: String) -> String? {
        let t = s.trimmingCharacters(in: .whitespacesAndNewlines)
        return t.isEmpty ? nil : t
    }
}

// MARK: - Flatten a group of ops → checklist

/// A view-ready checklist row: the *current* state of one task after replaying
/// the group's ops in order.
struct TaskChecklistRow: Identifiable, Equatable {
    let id: String          // task id, or a synthetic key for an id-less op
    let displayID: String?  // the `#id` badge text (nil → no badge)
    let title: String
    let description: String?
    let status: TaskItemStatus
}

/// Which overall action a group of ops represents — drives the card's summary.
/// Carries no count: the header counts the *deduped rows* (distinct tasks), not
/// the raw op count, so two updates to one task read "Updated 1 task".
enum TaskGroupAction: Equatable {
    case added
    case updated
    case deleted
    case listed
    case detail
    case mixed
}

enum TaskListBuild {

    /// Replay a group's ops into a deduped, in-order checklist (last write wins
    /// per task id), resolving an update's missing title from any create / list /
    /// get for the same id seen anywhere in the group.
    static func rows(from ops: [TaskOp]) -> [TaskChecklistRow] {
        // Accumulate by id, preserving first-seen order. id-less ops get a
        // synthetic per-op key so two un-numbered creates don't collapse together.
        struct Acc { var displayID: String?; var subject: String?; var description: String?; var status: TaskItemStatus }
        var order: [String] = []
        var acc: [String: Acc] = [:]
        var synthetic = 0

        func upsert(key: String, displayID: String?, subject: String?, description: String?, status: TaskItemStatus) {
            if var existing = acc[key] {
                if let displayID { existing.displayID = displayID }
                if let subject { existing.subject = subject }        // never clobber a known title with nil
                if let description { existing.description = description }
                existing.status = status                              // last write wins
                acc[key] = existing
            } else {
                order.append(key)
                acc[key] = Acc(displayID: displayID, subject: subject, description: description, status: status)
            }
        }

        for op in ops {
            switch op.kind {
            case .create, .get, .update:
                let key: String
                if let id = op.id { key = id }
                else { synthetic += 1; key = "·new\(synthetic)" }
                let status = op.status ?? acc[key]?.status ?? .pending
                upsert(key: key, displayID: op.id, subject: op.subject, description: op.description, status: status)
            case .list:
                for r in op.listRows {
                    let key = r.id ?? { synthetic += 1; return "·new\(synthetic)" }()
                    upsert(key: key, displayID: r.id, subject: r.subject, description: nil, status: r.status)
                }
            }
        }

        return order.map { key in
            let a = acc[key]!
            // With a known subject, the `#id` rides along as a small badge. Without
            // one, the id *becomes* the title — so don't also render the badge, or
            // the row reads "#9  #9".
            if let subject = a.subject {
                return TaskChecklistRow(id: key, displayID: a.displayID, title: subject,
                                        description: a.description, status: a.status)
            }
            let title = a.displayID.map { "#\($0)" } ?? String(localized: "Task")
            return TaskChecklistRow(id: key, displayID: nil, title: title,
                                    description: a.description, status: a.status)
        }
    }

    /// Classify the group for its header summary (the count is supplied separately
    /// from the deduped row total).
    static func action(of ops: [TaskOp]) -> TaskGroupAction {
        guard !ops.isEmpty else { return .mixed }
        let kinds = Set(ops.map(\.kind))
        if kinds == [.list] { return .listed }
        if kinds == [.get] { return .detail }
        if kinds == [.create] { return .added }
        if kinds == [.update] {
            return ops.allSatisfy { $0.status == .deleted } ? .deleted : .updated
        }
        return .mixed
    }
}
