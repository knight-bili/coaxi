// Fixed Codeg aff1e6c; Apache-2.0, integration/licenses/codeg/LICENSE.
// Host seams are recorded in integration/inventory/native-session-viewer-ui-reference.md.
export function formatTokenCount(n: number): string {
  if (n >= 1_000_000) {
    return `${(n / 1_000_000).toFixed(1).replace(/\.0$/, "")}M`
  }
  if (n >= 1_000) {
    return `${(n / 1_000).toFixed(1).replace(/\.0$/, "")}K`
  }
  return n.toLocaleString()
}
