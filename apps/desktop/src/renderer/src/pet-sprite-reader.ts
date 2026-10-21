import { sha256 } from '@noble/hashes/sha2.js'
type Request=<T>(input:{operation:'asset';id:string;offset:number})=>Promise<T>
export async function readPetSprite(request:Request,id:string,signal:AbortSignal):Promise<string>{
  const hash=sha256.create(),chunks:Uint8Array[]=[];let offset=0,total:number|null=null,digest:string|null=null,mime:string|null=null
  for(;;){
    if(signal.aborted)throw new DOMException('closed','AbortError')
    const page=await request<{mime:string;digest:string;offset:number;totalBytes:number;next:number|null;bytes:string}>({operation:'asset',id,offset})
    if(signal.aborted)throw new DOMException('closed','AbortError')
    if(!Number.isSafeInteger(page.totalBytes)||page.totalBytes>16*1024*1024||page.totalBytes<1||page.offset!==offset||total!==null&&total!==page.totalBytes||digest!==null&&digest!==page.digest||mime!==null&&mime!==page.mime||!['image/png','image/webp'].includes(page.mime))throw new Error('pets.asset_changed')
    total=page.totalBytes;digest=page.digest;mime=page.mime
    const bytes=Uint8Array.from(atob(page.bytes),character=>character.charCodeAt(0));chunks.push(bytes);hash.update(bytes);offset+=bytes.length
    if(page.next===null){if(offset!==total)throw new Error('pets.incomplete_asset');break}
    if(page.next!==offset||!bytes.length)throw new Error('pets.invalid_page')
  }
  if(Array.from(hash.digest(),byte=>byte.toString(16).padStart(2,'0')).join('')!==digest)throw new Error('pets.asset_digest_changed')
  const bytes=new Uint8Array(offset);let position=0;for(const chunk of chunks){bytes.set(chunk,position);position+=chunk.length}
  return URL.createObjectURL(new Blob([bytes],{type:mime!}))
}
