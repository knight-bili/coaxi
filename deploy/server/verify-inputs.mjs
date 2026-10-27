import {createHash} from 'node:crypto'
import {lstatSync,readFileSync} from 'node:fs'
import {isAbsolute,join,resolve} from 'node:path'

// Image assembly admission only. Never launches Core, a Runtime or a model.
const root=resolve(process.argv[2]??'')
if(process.argv.length!==3||process.platform!=='linux'||process.arch!=='x64')throw new Error('A native Linux x64 Server package is required')
const manifest=JSON.parse(readFileSync(join(root,'manifest.json'),'utf8'))
if(manifest.schemaVersion!==1||manifest.productId!=='coaxi'||manifest.target!=='linux-x64'||manifest.profile!=='release'||!/^[a-f0-9]{40}$/.test(manifest.commit))throw new Error('Server package identity mismatch')
if(!manifest.files||typeof manifest.files!=='object'||Array.isArray(manifest.files))throw new Error('Server file manifest is missing')
for(const name of ['coaxi-server','coaxi-host','coaxi','coaxi-native-history','web-ui/index.html','workbench/product-release.json','workbench/sources.lock.json','server-channels/manifest.json','channel-runtime/node','licenses/integration/coaxi/LICENSE','licenses/integration/coaxi/LICENSE']){
  if(typeof manifest.files[name]!=='string')throw new Error(`Missing Server package member: ${name}`)
}
for(const [name,digest] of Object.entries(manifest.files)){
  const parts=name.split('/')
  if(isAbsolute(name)||name.includes('\\')||parts.some(part=>!part||part==='.'||part==='..')||typeof digest!=='string'||!/^[a-f0-9]{64}$/.test(digest))throw new Error('Invalid Server manifest entry')
  let path=root
  for(const part of parts){path=join(path,part);if(lstatSync(path).isSymbolicLink())throw new Error('Linked Server package members are not allowed')}
  if(!lstatSync(path).isFile()||createHash('sha256').update(readFileSync(path)).digest('hex')!==digest)throw new Error(`Server package content mismatch: ${name}`)
}
for(const name of ['coaxi-server','coaxi-host','coaxi','coaxi-native-history','channel-runtime/node']){
  if(!(lstatSync(join(root,name)).mode&0o111))throw new Error(`Server executable permission missing: ${name}`)
}
const identity=JSON.parse(readFileSync(join(root,'workbench/product-release.json'),'utf8'))
if(identity.productId!==manifest.productId||identity.repository!=='https://github.com/knight-bili/codeg-rovai-workbench')throw new Error('Server product source mismatch')
