import Foundation

/// Context-compaction tool-call detection (port of the web `lib/context-compaction.ts`).
///
/// A compaction lifecycle arrives as an ACP `tool_call` tagged with
/// `_meta.contextCompaction == true` — codex-acp emits it natively (1.1.3+), and
/// codeg's Grok bridge synthesizes the same shape for `auto_compact_completed`,
/// both on the live stream and when re-reading history. It is addressed by that
/// meta flag, NOT by tool name, so it works for every host that adopts the tag.
///
/// It renders as a chrome-less centered divider ("context was compacted here"),
/// not as a tool card, and must never fold into a "N tools" group.
enum ContextCompaction {
    /// Whether this tool call's `_meta` marks it as a compaction.
    static func matches(_ meta: AnyJSON?) -> Bool {
        meta?["contextCompaction"]?.bool == true
    }

    /// The token counts Grok stamps on its compaction card (`tokensBefore` /
    /// `tokensAfter`). codex sends none, so both are frequently nil — the divider
    /// falls back to a plain label then.
    static func tokens(_ meta: AnyJSON?) -> (before: Int?, after: Int?) {
        (count(meta, "tokensBefore"), count(meta, "tokensAfter"))
    }

    /// A non-negative, finite integer field off the opaque meta pass-through.
    private static func count(_ meta: AnyJSON?, _ key: String) -> Int? {
        guard let value = meta?[key]?.double, value.isFinite, value >= 0 else { return nil }
        return Int(value)
    }
}
