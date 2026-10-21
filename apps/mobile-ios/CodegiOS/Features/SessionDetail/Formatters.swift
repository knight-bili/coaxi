import Foundation

/// Compact token-count formatting: 980 → "980", 12_300 → "12.3k", 1_400_000 → "1.4M".
enum TokenFormat {
    static func compact(_ tokens: Int) -> String {
        let n = Double(tokens)
        switch tokens {
        case ..<1_000:
            return "\(tokens)"
        case ..<1_000_000:
            return trim(n / 1_000) + "k"
        default:
            return trim(n / 1_000_000) + "M"
        }
    }

    private static func trim(_ value: Double) -> String {
        if value >= 100 { return String(Int(value.rounded())) }
        return String(format: "%.1f", value)
    }
}

/// Short duration formatting from milliseconds: 850 → "0.8s", 4200 → "4.2s",
/// 95_000 → "1m 35s".
enum DurationFormat {
    static func short(_ ms: Int) -> String {
        let seconds = Double(ms) / 1000
        if seconds < 60 {
            return String(format: "%.1fs", seconds)
        }
        let whole = Int(seconds.rounded())
        let m = whole / 60
        let s = whole % 60
        return s == 0 ? "\(m)m" : "\(m)m \(s)s"
    }
}
