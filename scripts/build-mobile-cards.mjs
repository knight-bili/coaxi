import {spawn} from 'node:child_process'
import {cp,mkdir,writeFile} from 'node:fs/promises'
import {resolve,dirname} from 'node:path'
import {fileURLToPath} from 'node:url'

// 编译时复用同一原 Coaxi Renderer；此文件不启动 App、Host 或 Runtime。
const root=resolve(dirname(fileURLToPath(import.meta.url)),'..')
const flags=process.argv.slice(2);if(flags.some(value=>!['--android','--ios'].includes(value)))throw new Error('Only --android / --ios are accepted')
await new Promise((resolvePromise,reject)=>{const child=spawn(process.execPath,[resolve(root,'node_modules/vite/bin/vite.js'),'build','--config','mobile-cards.vite.config.ts'],{cwd:root,stdio:'inherit'});child.once('error',reject);child.once('exit',code=>code===0?resolvePromise():reject(new Error(`Mobile card bundle build failed: ${code}`)))})
const output=resolve(root,'out/mobile-cards')
await cp(resolve(root,'THIRD_PARTY_NOTICES.md'),resolve(output,'THIRD_PARTY_NOTICES.md'))
await cp(resolve(root,'LICENSE'),resolve(output,'coaxi-LICENSE'))
await cp(resolve(root,'integration/licenses'),resolve(output,'integration-licenses'),{recursive:true,force:true})
await cp(resolve(root,'legal/licenses'),resolve(output,'dependency-licenses'),{recursive:true,force:true})
const csp="default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src data:; font-src 'self' data:; connect-src 'none'; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'"
await writeFile(resolve(output,'index.html'),`<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1,viewport-fit=cover"><meta http-equiv="Content-Security-Policy" content="${csp}"><link rel="stylesheet" href="mobile-card.css"><title>交互卡片</title></head><body><div id="root"></div><script src="mobile-card.js"></script></body></html>`)
const targets=[]
if(!flags.length||flags.includes('--android'))targets.push(resolve(root,'apps/mobile-android/app/src/main/assets/card-preview'))
if(!flags.length||flags.includes('--ios'))targets.push(resolve(root,'apps/mobile-ios/CoaxiiOS/Resources/CardPreview.bundle'))
for(const target of targets){await mkdir(target,{recursive:true});await cp(output,target,{recursive:true,force:true});if(target.endsWith('.bundle'))await writeFile(resolve(target,'Info.plist'),'<?xml version="1.0" encoding="UTF-8"?><!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd"><plist version="1.0"><dict><key>CFBundleIdentifier</key><string>app.coaxi.workbench.card-preview</string><key>CFBundleName</key><string>CardPreview</string><key>CFBundlePackageType</key><string>BNDL</string></dict></plist>')}
