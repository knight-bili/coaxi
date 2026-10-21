import SwiftUI

/// Renders a string as inline Markdown, gracefully degrading to plain text when
/// the string is not valid Markdown. Used for user bubbles and assistant text
/// blocks. Whitespace is preserved between paragraphs (`.inlineOnlyPreservingWhitespace`
/// keeps newlines that the default parser would otherwise collapse).
struct MarkdownText: View {
    let raw: String
    var color: Color = Theme.textPrimary
    var font: Font = Theme.Typography.messageBody
    /// When true, render the raw string verbatim and skip Markdown parsing. Used
    /// for live-streaming text so each token delta doesn't re-parse the entire
    /// accumulated reply (O(n²) main-actor work on long replies); the turn
    /// re-renders once with full Markdown when it finalizes.
    var plain: Bool = false

    var body: some View {
        // Live-streaming text (`plain`) renders verbatim and is skipped by the
        // cache — each token delta is a distinct string, so caching would only
        // churn. Finalized turns go through the cache so a `List` row recycled
        // back into view re-uses its parse instead of re-running it.
        Text(plain ? AttributedString(raw) : Self.cachedAttributed(raw))
            .font(font)
            .lineSpacing(Theme.Typography.messageLineSpacing)
            .foregroundStyle(color)
            .textSelection(.enabled)
            .tint(Theme.accent)
            .fixedSize(horizontal: false, vertical: true)
    }

    /// Parse Markdown into an `AttributedString`, falling back to a verbatim
    /// string when parsing fails. Newlines are preserved so streamed multi-line
    /// replies keep their shape.
    static func attributed(from raw: String) -> AttributedString {
        var options = AttributedString.MarkdownParsingOptions()
        options.interpretedSyntax = .inlineOnlyPreservingWhitespace
        options.failurePolicy = .returnPartiallyParsedIfPossible
        if let parsed = try? AttributedString(markdown: raw, options: options) {
            return parsed
        }
        return AttributedString(raw)
    }

    // MARK: - Parse cache

    /// Markdown parsing is the per-turn rendering cost that dominates a long
    /// transcript. With a recycling `List`, a row that scrolls off and back on
    /// would otherwise re-parse every time. This bounded, main-thread-only cache
    /// keeps each distinct string's parse around so re-display is free.
    ///
    /// Accessed exclusively from `body` (SwiftUI rendering is on the main
    /// thread), so a plain static store is safe without extra synchronization.
    private static var cache: [String: AttributedString] = [:]
    private static var cacheOrder: [String] = []
    private static let cacheLimit = 500

    private static func cachedAttributed(_ raw: String) -> AttributedString {
        if let hit = cache[raw] { return hit }
        let parsed = attributed(from: raw)
        cache[raw] = parsed
        cacheOrder.append(raw)
        if cacheOrder.count > cacheLimit {
            let evicted = cacheOrder.removeFirst()
            cache.removeValue(forKey: evicted)
        }
        return parsed
    }
}
