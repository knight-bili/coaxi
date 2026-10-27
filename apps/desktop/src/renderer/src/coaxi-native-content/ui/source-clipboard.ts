// Fixed Codeg aff1e6c; Apache-2.0, integration/licenses/component-a/LICENSE.


/**
 * Legacy clipboard copy: a hidden `<textarea>` + `document.execCommand("copy")`.
 * Works in non-secure contexts (HTTP over LAN) where the async Clipboard API is
 * unavailable. Returns whether the copy succeeded.
 */
function legacyCopyText(text: string): boolean {
  try {
    const textarea = document.createElement("textarea")
    textarea.value = text
    textarea.setAttribute("readonly", "")
    textarea.setAttribute("aria-hidden", "true")
    textarea.style.position = "absolute"
    textarea.style.left = "-9999px"
    textarea.style.top = `${window.pageYOffset || document.documentElement.scrollTop}px`
    textarea.style.opacity = "0"
    textarea.style.pointerEvents = "none"
    const selectionApi = document.getSelection()
    const previousRange =
      selectionApi && selectionApi.rangeCount > 0
        ? selectionApi.getRangeAt(0).cloneRange()
        : null
    document.body.appendChild(textarea)
    textarea.focus({ preventScroll: true })
    textarea.select()
    textarea.setSelectionRange(0, text.length)
    let ok = false
    try {
      ok = document.execCommand("copy")
    } catch {
      ok = false
    }
    textarea.remove()
    if (previousRange) {
      try {
        const selection = document.getSelection()
        selection?.removeAllRanges()
        selection?.addRange(previousRange)
      } catch {
        // restoring the prior page selection is best-effort
      }
    }
    return ok
  } catch {
    return false
  }
}

/**
 * Write text to the clipboard. Falls back to `legacyCopyText` when the async
 * Clipboard API is unavailable (non-secure contexts such as HTTP over LAN).
 */
export async function copyTextToClipboard(text: string): Promise<boolean> {
  if (typeof window === "undefined") return false
  if (navigator.clipboard?.writeText) {
    try {
      await navigator.clipboard.writeText(text)
      return true
    } catch {
      // fall through to legacy path
    }
  }
  return legacyCopyText(text)
}
