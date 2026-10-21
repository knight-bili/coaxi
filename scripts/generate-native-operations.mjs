import { readFile, writeFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import { dirname, resolve } from 'node:path'

// Native clients use only the independently admitted public Host enum.
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const source = await readFile(resolve(root, 'crates/rovai-web/src/operations.rs'), 'utf8')
const declaration = source.slice(source.indexOf('pub enum Operation {'), source.indexOf('impl Operation {'))
const names = [...declaration.matchAll(/#\[serde\(\s*rename\s*=\s*"([^"]+)"/g)].map(match => match[1])
const kotlin = names.map(name => name.replace(/[^a-zA-Z0-9]/g, '_').toUpperCase())
const swift = names.map(name => name.split('.').map((part, index) => index ? part[0].toUpperCase() + part.slice(1) : part).join(''))
if (!names.length || [names, kotlin, swift].some(values => new Set(values).size !== values.length)) throw new Error('Public operation names must be nonempty and unique in each target language')
await writeFile(resolve(root, 'apps/mobile-android/app/src/main/java/app/codeg/android/core/network/RovaiOperation.kt'), `package app.codeg.android.core.network\n\n/** Generated from the explicit public Host enum; private Core methods are not admitted. */\nenum class RovaiOperation(val wire:String) {\n${names.map((name,index)=>`    ${kotlin[index]}("${name}")`).join(',\n')}\n}\n`)
await writeFile(resolve(root, 'apps/mobile-ios/CodegiOS/Networking/RovaiOperation.swift'), `import Foundation\n\n/// Generated from the explicit public Host enum; private Core methods never cross this seam.\nenum RovaiOperation: String, Sendable {\n${names.map((name,index)=>`    case ${swift[index]} = "${name}"`).join('\n')}\n}\n`)
process.stdout.write(`Wrote ${names.length} public operation names to both native clients.\n`)
