// Fixed Codeg aff1e6c; Apache-2.0, integration/licenses/component-a/LICENSE.
export function localImagePath(source: string): string | null {
  let path = source.trim()
  if (!path || path.startsWith("//") || path.startsWith("#")) return null
  if (/^[\\/]{2}[?.][\\/]/.test(path)) return null
  if (/^file:/i.test(path)) {
    try {
      const url = new URL(path)
      path = url.host ? `//${url.host}${url.pathname}` : url.pathname
    } catch {
      return null
    }
  } else {
    // A drive letter is a path, not a URI scheme. Drive-relative C:foo is
    // deliberately excluded because it depends on process-global state.
    if (/^[a-z][a-z\d+.-]*:/i.test(path) && !/^[a-z]:[\\/]/i.test(path)) {
      return null
    }
    path = path.split(/[?#]/, 1)[0]
  }
  try {
    path = decodeURIComponent(path)
  } catch {
    return null
  }
  if (/[\u0000-\u001f\u007f]/.test(path)) return null
  path = path.replace(/\\/g, "/")
  // Reject device paths and NTFS alternate data streams as well as schemes
  // hidden behind percent encoding. Only the drive-designator colon is valid.
  if (/^\/\/[?.]\//.test(path) || /:/.test(path.replace(/^\/?[a-z]:\//i, ""))) {
    return null
  }
  return path
}
