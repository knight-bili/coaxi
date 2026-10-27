/** 地址栏接受网址或搜索词；底层浏览器接口仍只接受 HTTP(S) URL。 */
export function resolveBrowserAddress(input: string): string | null {
  const value = input.trim()
  if (!value) return null
  const explicitScheme = /^[a-z][a-z\d+.-]*:/i.test(value) && !/^(localhost|[\w.-]+\.\w+):\d+(?:[/?#]|$)/i.test(value)
  const authority = value.split(/[/?#]/, 1)[0]
  const addressLike = !/\s/.test(value) && (/^(localhost|\[[\da-f:]+\]|(?:\d{1,3}\.){3}\d{1,3})(?::\d+)?$/i.test(authority) || /^(?:[^\s.@:]+\.)+[^\s.@:]{2,}(?::\d+)?$/u.test(authority))
  if (!explicitScheme && !addressLike) return `https://www.baidu.com/s?wd=${encodeURIComponent(value)}`
  const url = new URL(explicitScheme ? value : `${/^localhost(?::|\/|$)|^127\.|^\[::1\]/i.test(value) ? 'http' : 'https'}://${value}`)
  if (!['http:', 'https:'].includes(url.protocol) || url.username || url.password) throw new Error('browser.invalid_address')
  return url.href
}
