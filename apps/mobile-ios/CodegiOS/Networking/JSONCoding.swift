import Foundation

/// Shared JSON coders configured for codeg's split casing convention:
/// requests are camelCase (default encoder, no key conversion) and responses
/// are snake_case (`.convertFromSnakeCase`). Dates are lenient RFC3339.
enum CodegJSON {
    static let decoder: JSONDecoder = {
        let d = JSONDecoder()
        d.keyDecodingStrategy = .convertFromSnakeCase
        d.dateDecodingStrategy = .custom { decoder in
            let container = try decoder.singleValueContainer()
            let raw = try container.decode(String.self)
            if let date = ISO8601.parse(raw) { return date }
            throw DecodingError.dataCorruptedError(
                in: container,
                debugDescription: "Unparseable RFC3339 date: \(raw)"
            )
        }
        return d
    }()

    static let encoder: JSONEncoder = {
        let e = JSONEncoder()
        // Requests use camelCase keys — keep Swift property names verbatim.
        e.dateEncodingStrategy = .iso8601
        return e
    }()
}

/// Tolerant RFC3339 / ISO8601 parsing. chrono on the server may emit fractional
/// seconds with arbitrary precision (or none); we accept all three shapes and
/// drop sub-second precision as a last resort (immaterial for display/sort).
enum ISO8601 {
    private static let withFractional: ISO8601DateFormatter = {
        let f = ISO8601DateFormatter()
        f.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return f
    }()

    private static let plain: ISO8601DateFormatter = {
        let f = ISO8601DateFormatter()
        f.formatOptions = [.withInternetDateTime]
        return f
    }()

    /// Serializes access to the shared formatters. They are decoded from both the
    /// HTTP and WebSocket paths concurrently; ISO8601DateFormatter is not declared
    /// Sendable, so we guard it rather than rely on undocumented thread-safety.
    private static let lock = NSLock()

    static func parse(_ string: String) -> Date? {
        lock.lock()
        defer { lock.unlock() }
        if let d = plain.date(from: string) { return d }
        if let d = withFractional.date(from: string) { return d }
        // Strip any fractional-seconds component and retry (handles 6/9-digit
        // precision that ISO8601DateFormatter rejects).
        let stripped = string.replacingOccurrences(
            of: #"\.\d+"#,
            with: "",
            options: .regularExpression
        )
        return plain.date(from: stripped)
    }
}
