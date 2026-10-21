import {spawnSync} from 'node:child_process'
import {copyFileSync,chmodSync,existsSync,mkdirSync} from 'node:fs'
import {dirname,join,resolve} from 'node:path'
import {serverTarget} from './sidecar-targets.mjs'

/** The parser helper has an isolated Cargo workspace so the donor's read-only
 * SQLite/SeaORM dependencies cannot change Core's SQLite link identity. */
export function buildNativeHistoryHelper(repository,targetKey,{release=false,destination}={}){
  const target=serverTarget(targetKey)
  const targetDirectory=resolve(repository,process.env.ROVAI_HISTORY_TARGET_DIR??'helpers/native-history/target')
  const args=['build','--manifest-path',join(repository,'helpers/native-history/Cargo.toml'),'--target',target.rustTarget,...(release?['--release']:[])]
  const result=spawnSync('cargo',args,{cwd:repository,stdio:'inherit',env:{...process.env,CARGO_TARGET_DIR:targetDirectory}})
  if(result.status!==0)throw new Error('Native history parser helper build failed')
  const name=`rovai-native-history${target.executableSuffix}`
  const source=join(targetDirectory,target.rustTarget,release?'release':'debug',name)
  if(!existsSync(source))throw new Error('Native history helper executable is missing')
  if(destination){const output=join(destination,name);mkdirSync(dirname(output),{recursive:true});copyFileSync(source,output);if(target.platform!=='win32')chmodSync(output,0o755);return output}
  return source
}
