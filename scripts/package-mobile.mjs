import {spawn,execFileSync} from 'node:child_process'
import {cp,mkdir,readFile,writeFile,stat} from 'node:fs/promises'
import {resolve,dirname} from 'node:path'
import {fileURLToPath} from 'node:url'
import {createHash} from 'node:crypto'

// 显式打包命令，只在后续集中构建阶段调用；本文件不启动模拟器、模型或远端工作流。
const root=resolve(dirname(fileURLToPath(import.meta.url)),'..'),platform=process.argv[2]
if(!['--android','--ios'].includes(platform)||process.argv.length!==3)throw new Error('Usage: node scripts/package-mobile.mjs --android|--ios')
async function run(command,args,cwd=root){await new Promise((done,reject)=>{const child=spawn(command,args,{cwd,stdio:'inherit'});child.once('error',reject);child.once('exit',code=>code===0?done():reject(new Error(`${command} failed: ${code}`)))})}
const output=resolve(root,'out/mobile-packages');await mkdir(output,{recursive:true})
let input,packagePath
if(platform==='--android'){
  const folder=resolve(root,'apps/mobile-android')
  if(process.platform==='win32')await run('cmd.exe',['/d','/c','gradlew.bat','--no-daemon',':app:assembleDebug'],folder)
  else await run('bash',['gradlew','--no-daemon',':app:assembleDebug'],folder)
  input=resolve(folder,'app/build/outputs/apk/debug/app-debug.apk');packagePath=resolve(output,'Coaxi-V1.0.0-android-debug.apk');await cp(input,packagePath)
}else{
  if(process.platform!=='darwin')throw new Error('iOS packaging requires a macOS build host')
  await run(process.execPath,[resolve(root,'scripts/build-mobile-cards.mjs'),'--ios'])
  const folder=resolve(root,'apps/mobile-ios'),derived=resolve(root,'out/mobile-ios')
  await run('xcodegen',['generate'],folder)
  await run('xcodebuild',['-project','CodegiOS.xcodeproj','-scheme','CodegiOS','-configuration','Debug','-sdk','iphonesimulator','-destination','generic/platform=iOS Simulator','-derivedDataPath',derived,'CODE_SIGNING_ALLOWED=NO','build'],folder)
  input=resolve(derived,'Build/Products/Debug-iphonesimulator/Coaxi.app');await stat(input)
  packagePath=resolve(output,'Coaxi-V1.0.0-ios-simulator.app.zip');await run('ditto',['-c','-k','--sequesterRsrc','--keepParent',input,packagePath])
}
const bytes=await readFile(packagePath),revision=execFileSync('git',['rev-parse','HEAD'],{cwd:root,encoding:'utf8'}).trim(),dirty=Boolean(execFileSync('git',['status','--porcelain'],{cwd:root,encoding:'utf8'}).trim())
await writeFile(packagePath+'.json',JSON.stringify({schemaVersion:1,productId:'codeg-rovai-workbench',targetVersion:'V1.0.0',sourceRevision:revision,dirty,platform:platform.slice(2),kind:platform==='--ios'?'unsigned_simulator':'debug_apk',sha256:createHash('sha256').update(bytes).digest('hex'),bytes:bytes.length,builtAt:new Date().toISOString(),runtimeStarted:false,modelStarted:false,realDeviceVerified:false},null,2)+'\n')
