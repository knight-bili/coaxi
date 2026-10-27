import {createHash} from 'node:crypto'
import {chmodSync,copyFileSync,cpSync,existsSync,lstatSync,mkdirSync,readFileSync,writeFileSync} from 'node:fs'
import {dirname,join,resolve} from 'node:path'

/** Package inputs only. This function neither launches the Host nor installs a
 * driver, performs login, initializes Core data or changes an existing server. */
export function packageServerChannels(repository,destination,target){
  if(Number(process.versions.node.split('.')[0])!==24)throw new Error('Server channel packaging requires Node 24')
  if(process.versions.electron)throw new Error('Server channels require a standalone Node runtime')
  const built=join(repository,'build/server-channels')
  const manifest=JSON.parse(readFileSync(join(built,'manifest.json'),'utf8'))
  if(manifest.schemaVersion!==1||manifest.protocol!==1||manifest.nodeMajor!==24)throw new Error('Server channel bundle protocol mismatch')
  for(const entry of [manifest.bundle,...manifest.assets]){
    const path=resolve(built,entry.path)
    if(!entry.path||entry.path.split(/[\\/]/).some(part=>!part||part==='.'||part==='..')||/^[A-Za-z]:|^[\\/]/.test(entry.path))throw new Error('Server channel asset path rejected')
    const info=lstatSync(path);if(!info.isFile()||info.isSymbolicLink())throw new Error('Server channel asset must be a regular file')
    if(createHash('sha256').update(readFileSync(path)).digest('hex')!==entry.sha256)throw new Error('Server channel bundle digest mismatch')
  }
  const runtime=join(destination,'channel-runtime');mkdirSync(runtime,{recursive:true})
  const name=target.platform==='win32'?'node.exe':'node'
  const input=process.execPath
  if(!lstatSync(input).isFile()||lstatSync(input).isSymbolicLink())throw new Error('Resolve Node to a regular native executable before packaging')
  const binary=join(runtime,name);copyFileSync(input,binary);if(target.platform!=='win32')chmodSync(binary,0o755)
  const licenseCandidates=[process.env.COAXI_NODE_LICENSE,join(dirname(input),'LICENSE'),join(dirname(input),'LICENSE.txt'),join(dirname(input),'../share/doc/node/LICENSE')].filter(Boolean)
  const license=licenseCandidates.find(path=>existsSync(path)&&lstatSync(path).isFile()&&!lstatSync(path).isSymbolicLink())
  if(!license)throw new Error('Node distribution LICENSE is required; set COAXI_NODE_LICENSE to its exact source file')
  copyFileSync(license,join(runtime,'LICENSE'))
  cpSync(built,join(destination,'server-channels'),{recursive:true,filter:path=>!path.endsWith('inputs.json')})
  manifest.runtime={path:`channel-runtime/${name}`,version:process.versions.node,sha256:createHash('sha256').update(readFileSync(binary)).digest('hex')}
  writeFileSync(join(destination,'server-channels/manifest.json'),JSON.stringify(manifest,null,2)+'\n')
  // Preserve the donor source notices and the bundled SDK/Node license chain.
  cpSync(join(repository,'integration/licenses'),join(destination,'licenses/integration'),{recursive:true})
  cpSync(join(repository,'legal/licenses'),join(destination,'licenses/dependencies'),{recursive:true})
  return {protocol:manifest.protocol,nodeVersion:manifest.runtime.version,providers:manifest.providers,runtimeSha256:manifest.runtime.sha256}
}
