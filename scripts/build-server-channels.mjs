import {build} from 'esbuild'
import {mkdir,readFile,writeFile,copyFile} from 'node:fs/promises'
import path from 'node:path'
import {fileURLToPath} from 'node:url'
import {createHash} from 'node:crypto'

const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..')
const output=path.join(root,'build/server-channels')
await mkdir(path.join(output,'assets'),{recursive:true})
const copied=new Map()
const platform=path.join(root,'apps/server-channels/src/node-platform.ts')
await build({entryPoints:[path.join(root,'apps/server-channels/src/index.ts')],outfile:path.join(output,'channels.mjs'),bundle:true,platform:'node',format:'esm',target:'node24',packages:'bundle',metafile:true,banner:{js:"import {createRequire as __createRequire} from 'node:module';const require=__createRequire(import.meta.url);"},plugins:[{
  name:'standalone-channel-platform',setup(builder){
    builder.onResolve({filter:/^electron$/},()=>({path:platform}))
    builder.onResolve({filter:/^@contracts$/},()=>({path:path.join(root,'packages/contracts/src/index.ts')}))
    builder.onResolve({filter:/\?asset$/},args=>({path:path.resolve(args.resolveDir,args.path.slice(0,-6)),namespace:'channel-asset'}))
    builder.onLoad({filter:/.*/,namespace:'channel-asset'},async args=>{
      const bytes=await readFile(args.path);const digest=createHash('sha256').update(bytes).digest('hex');const name=`${digest}${path.extname(args.path)}`
      if(!copied.has(name)){await copyFile(args.path,path.join(output,'assets',name));copied.set(name,{path:`assets/${name}`,sha256:digest,bytes:bytes.length})}
      return {loader:'js',contents:`import {fileURLToPath} from 'node:url';export default fileURLToPath(new URL(${JSON.stringify(`./assets/${name}`)},import.meta.url));`}
    })
  }
}]}).then(async result=>{
  const bundle=await readFile(path.join(output,'channels.mjs'))
  await writeFile(path.join(output,'manifest.json'),JSON.stringify({schemaVersion:1,protocol:1,nodeMajor:24,bundle:{path:'channels.mjs',sha256:createHash('sha256').update(bundle).digest('hex'),bytes:bundle.length},assets:[...copied.values()],providers:['feishu','lark','dingtalk'],sharedCore:true,nativeWindowFallback:'requires_desktop_interaction'},null,2)+'\n')
  await writeFile(path.join(output,'inputs.json'),JSON.stringify(result.metafile,null,2)+'\n')
})
