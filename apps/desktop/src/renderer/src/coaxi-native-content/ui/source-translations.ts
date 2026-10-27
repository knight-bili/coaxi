import { IntlMessageFormat } from 'intl-messageformat'
import copy from './source-copy.json'
import { supportedInterfaceLanguage } from '../../../../shared/interface-languages'

// Keys and messages come only from the fixed source catalog. No transcript text
// can create cache entries or become a message template.
const compiled = new Map<string, IntlMessageFormat>()
export function formatSourceCopy(language: string, scope: string, key: string, values?: Record<string, unknown>): string {
  const locale = supportedInterfaceLanguage(language) ? language : 'en'
  const catalog: Record<string, unknown> = copy[locale]
  const base = scope.endsWith('.goal') ? scope.slice(0, -5) : scope
  const name = scope.endsWith('.goal') ? 'goal.' + key : key
  let value: unknown = catalog[base]
  for (const part of name.split('.')) value = value && typeof value === 'object' ? (value as Record<string, unknown>)[part] : undefined
  if (typeof value !== 'string') return key
  const cacheKey = locale + ':' + base + ':' + name
  let formatter = compiled.get(cacheKey)
  if (!formatter) { formatter = new IntlMessageFormat(value, locale); compiled.set(cacheKey, formatter) }
  const result = formatter.format(values as Parameters<IntlMessageFormat['format']>[0])
  return Array.isArray(result) ? result.join('') : String(result)
}
