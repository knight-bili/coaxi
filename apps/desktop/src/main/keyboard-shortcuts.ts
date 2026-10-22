import { promises as fs } from 'node:fs'
import { join } from 'node:path'
import { randomUUID } from 'node:crypto'
import { DEFAULT_SHORTCUTS,SHORTCUT_DEFINITIONS,canShareShortcut,normalizeShortcut,normalizeShortcutSettings,shortcutsConflict } from '../shared/codeg-keyboard-shortcuts'
import { writePrivateJson } from './general-preferences'
import type { KeyboardShortcutsSnapshot } from '@contracts'

export class KeyboardShortcutsStore {
  private saved:KeyboardShortcutsSnapshot|null=null
  private tail:Promise<unknown>=Promise.resolve()
  constructor(private readonly preferencesRoot:()=>string){}
  private file():string{return join(this.preferencesRoot(),'keyboard-shortcuts-v1.json')}
  private validate(value:KeyboardShortcutsSnapshot):void{
    if(!value||typeof value.revision!=='string'||!value.bindings||typeof value.bindings!=='object'||Array.isArray(value.bindings)
      ||Object.keys(value.bindings).sort().join(',')!==SHORTCUT_DEFINITIONS.map(def=>def.id).sort().join(','))throw new Error('shortcuts.invalid_settings')
    for(const {id}of SHORTCUT_DEFINITIONS){const raw=value.bindings[id];if(typeof raw!=='string'||raw.length>128||(raw!==''&&normalizeShortcut(raw)!==raw))throw new Error('shortcuts.invalid_binding')}
    for(let index=0;index<SHORTCUT_DEFINITIONS.length;index++)for(const other of SHORTCUT_DEFINITIONS.slice(index+1)){const id=SHORTCUT_DEFINITIONS[index].id;if(!canShareShortcut(id,other.id)&&shortcutsConflict(value.bindings[id],value.bindings[other.id]))throw new Error('shortcuts.conflicting_binding')}
  }
  async get():Promise<KeyboardShortcutsSnapshot>{
    if(this.saved)return structuredClone(this.saved)
    try{const file=this.file(),stat=await fs.lstat(file);if(!stat.isFile()||stat.isSymbolicLink()||stat.size>32768)throw new Error('shortcuts.invalid_file');const value=JSON.parse(await fs.readFile(file,'utf8'))as KeyboardShortcutsSnapshot;this.validate(value);this.saved=value}
    catch(error){if((error as NodeJS.ErrnoException).code!=='ENOENT')throw error;const value={revision:randomUUID(),bindings:{...DEFAULT_SHORTCUTS}};await writePrivateJson(this.file(),value);this.saved=value}
    return structuredClone(this.saved)
  }
  save(value:KeyboardShortcutsSnapshot):Promise<KeyboardShortcutsSnapshot>{
    const work=this.tail.catch(()=>undefined).then(async()=>{this.validate(value);const current=await this.get();if(current.revision!==value.revision)throw new Error('shortcuts.settings_changed');const next={revision:randomUUID(),bindings:normalizeShortcutSettings(value.bindings)};await writePrivateJson(this.file(),next);this.saved=next;return structuredClone(next)})
    this.tail=work;return work
  }
}
