export function formatTokensPrecise(n: number): string {
  if (!Number.isFinite(n) || n < 0) return "0"
  if (n >= 1_000_000_000) return `${(n / 1_000_000_000).toFixed(2)}B`
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(2)}M`
  if (n >= 10_000) return `${(n / 1_000).toFixed(1)}K`
  return Math.round(n).toLocaleString()
}
