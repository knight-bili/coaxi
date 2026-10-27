import type { InterfaceLanguage } from '@contracts'
/** Coaxi aff1e6c src/lib/i18n.ts 的原十种语言，沿用标准 Intl 标签。 */
export const INTERFACE_LANGUAGE_OPTIONS:ReadonlyArray<{id:InterfaceLanguage;name:string}>=[
  {id:'en',name:'English'},{id:'zh-CN',name:'简体中文'},{id:'zh-TW',name:'繁體中文'},
  {id:'ja',name:'日本語'},{id:'ko',name:'한국어'},{id:'es',name:'Español'},
  {id:'de',name:'Deutsch'},{id:'fr',name:'Français'},{id:'pt',name:'Português'},{id:'ar',name:'العربية'},
]
export function supportedInterfaceLanguage(value:unknown):value is InterfaceLanguage{return typeof value==='string'&&INTERFACE_LANGUAGE_OPTIONS.some(option=>option.id===value)}
export function systemInterfaceLanguage(candidates:readonly string[]):InterfaceLanguage{
  for(const candidate of candidates){const normalized=candidate.trim().toLowerCase().replaceAll('_','-');if(normalized==='zh'||normalized==='zh-cn'||normalized==='zh-sg'||normalized.startsWith('zh-hans'))return 'zh-CN';if(normalized==='zh-tw'||normalized==='zh-hk'||normalized==='zh-mo'||normalized.startsWith('zh-hant'))return 'zh-TW';const base=normalized.split('-')[0];if(supportedInterfaceLanguage(base))return base}
  return 'en'
}
