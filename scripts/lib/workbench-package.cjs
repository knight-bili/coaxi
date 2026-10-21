const {execFileSync}=require('node:child_process')
const {createHash}=require('node:crypto')
const {lstatSync,readFileSync,writeFileSync}=require('node:fs')
const {join}=require('node:path')
/** Build-time package admission; no App, Runtime, model or external account is started. */
function recordWorkbenchPackage(context){
  const repository=context.packager.projectDir,platform=context.electronPlatformName
  const resources=platform==='darwin'?join(context.appOutDir,`${context.packager.appInfo.productFilename}.app`,'Contents/Resources'):join(context.appOutDir,'resources')
  const identity=JSON.parse(readFileSync(join(resources,'workbench/product-release.json'),'utf8'))
  if(identity.productId!=='codeg-rovai-workbench'||identity.repository!=='https://github.com/knight-bili/codeg-rovai-workbench')throw new Error('Fusion package identity is missing or belongs to another product')
  const source=JSON.parse(readFileSync(join(resources,'workbench/sources.lock.json'),'utf8'))
  if(source.repository!=='knight-bili/codeg-rovai-workbench')throw new Error('Fusion source lock mismatch')
  const suffix=platform==='win32'?'.exe':''
  const binaries=['rovai-core','rovai-host','rovai','rovai-native-history'].map(name=>`bin/${name}${suffix}`)
  const documents=['licenses/rovai-LICENSE','licenses/THIRD_PARTY_NOTICES.md','licenses/integration/codeg/LICENSE','licenses/integration/rovai-ai/LICENSE','workbench/sources.lock.json','workbench/product-release.json']
  const resourcesToHash=['web-ui/index.html','skills/cli-operations/SKILL.md',...documents]
  const files={}
  for(const relative of [...binaries,...resourcesToHash]){const path=join(resources,relative),metadata=lstatSync(path);if(!metadata.isFile()||metadata.isSymbolicLink()||metadata.size===0)throw new Error(`Required package input is missing: ${relative}`);if(resourcesToHash.includes(relative))files[relative]={bytes:metadata.size,sha256:createHash('sha256').update(readFileSync(path)).digest('hex')}}
  const revision=execFileSync('git',['rev-parse','HEAD'],{cwd:repository,encoding:'utf8'}).trim()
  if(!/^[a-f0-9]{40}$/.test(revision))throw new Error('Package source revision unavailable')
  const dirty=execFileSync('git',['status','--porcelain'],{cwd:repository,encoding:'utf8'}).trim().length>0
  // Native signing runs after afterPack. Record executable presence here; their
  // final signed digests belong to the final release archive/installer manifest.
  writeFileSync(join(resources,'workbench/package.json'),JSON.stringify({schemaVersion:1,productId:identity.productId,targetVersion:identity.targetVersion,version:context.packager.appInfo.version,sourceRevision:revision,dirty,platform,architecture:context.arch,stage:'before_native_signing',nativeExecutables:binaries,resources:files},null,2)+'\n')
}
module.exports={recordWorkbenchPackage}
