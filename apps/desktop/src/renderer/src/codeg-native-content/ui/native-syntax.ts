import type { HighlightedSourceSegment } from '../../source-preview'
import type { ResolvedTheme } from '@contracts'

// Retain every source character, including CRLF, empty and trailing lines.
// If a grammar normalizes input, show raw source instead of changed content.
export function sourceCodeLines(code: string, highlighted: HighlightedSourceSegment[]): Array<Array<{ content: string; className?: string }>> {
  const segments = highlighted.map(item => item.text).join('') === code ? highlighted : [{ text: code }]
  const lines: Array<Array<{ content: string; className?: string }>> = [[]]
  for (const segment of segments) {
    const pieces = segment.text.split('\n')
    pieces.forEach((content, index) => {
      if (index > 0) lines.push([])
      if (content) lines.at(-1)!.push({ content, ...(segment.className ? { className: segment.className } : {}) })
    })
  }
  return lines
}

const listeners = new Set<() => void>()
let observer: MutationObserver | null = null
export function sourceTheme(): ResolvedTheme { return typeof document !== 'undefined' && document.documentElement.dataset.theme === 'day' ? 'day' : 'night' }
export function subscribeSourceTheme(listener: () => void): () => void {
  listeners.add(listener)
  if (!observer && typeof document !== 'undefined') {
    observer = new MutationObserver(() => listeners.forEach(notify => notify()))
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] })
  }
  return () => { listeners.delete(listener); if (!listeners.size) { observer?.disconnect(); observer = null } }
}
