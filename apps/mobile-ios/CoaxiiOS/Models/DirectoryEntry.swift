import Foundation

/// One subdirectory returned by `list_directory_entries` for the server-side
/// directory browser. The server lists directories only; `hasChildren` says
/// whether drilling in will reveal further subdirectories (drives the chevron).
struct DirectoryEntry: Decodable, Identifiable, Hashable, Sendable {
    let name: String
    let path: String
    let hasChildren: Bool

    var id: String { path }
}

/// One entry returned by `list_directory_with_files` — like ``DirectoryEntry``
/// but includes files too (Rust `DirectoryItem`, serialized camelCase). Powers
/// the folder file browser: directories drill in, files open a preview. `path`
/// is the entry's **absolute** server path; `size` is bytes (files only).
struct DirectoryItem: Decodable, Identifiable, Hashable, Sendable {
    let name: String
    let path: String
    let isDir: Bool
    /// Only meaningful for directories — whether drilling in reveals children.
    let hasChildren: Bool
    /// File size in bytes; `nil` for directories.
    let size: Int?

    var id: String { path }
}

/// A file's text content (Rust `FilePreviewContent`), returned by
/// `read_file_preview`. `path` echoes the request's relative path.
struct FilePreviewContent: Decodable, Sendable {
    let path: String
    let content: String
}
