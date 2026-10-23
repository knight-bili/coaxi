import {useEffect,useState} from 'react'
import {useThreadClient} from './camp-client'
import {useUiText} from './interface-language'

type Result={success:boolean;output:string;error:string}
const LARGE_FILE_CHARS=256_000

export function WorkspaceGitFileAt({threadId,commit,path,deleted,binary}:{threadId:string;commit:string;path:string;deleted:boolean;binary:boolean}):React.JSX.Element{
  const client=useThreadClient(),t=useUiText(),[result,setResult]=useState<Result|null>(null),[error,setError]=useState<string|null>(null),[reading,setReading]=useState(false)
  useEffect(()=>{
    let active=true
    setResult(null);setError(null)
    if(deleted||binary){setReading(false);return()=>{active=false}}
    setReading(true)
    void client.request<Result>('workbench.git.read',{scope:{threadId},action:'file_at',target:commit,paths:[path]}).then(value=>{if(active)setResult(value)}).catch(reason=>{if(active)setError(String(reason))}).finally(()=>{if(active)setReading(false)})
    return()=>{active=false}
  },[client,threadId,commit,path,deleted,binary])

  if(deleted)return <p>{t('此提交已删除该文件，无法显示该版本内容。')}</p>
  if(binary)return <p>{t('这是二进制文件，当前不提供文本版本预览。')}</p>
  if(reading)return <p>{t('正在读取历史文件…')}</p>
  if(error)return <p role="alert">{error}</p>
  if(!result?.success)return <p role="alert">{result?.error||t('历史文件读取失败。')}</p>
  if(result.output.length>LARGE_FILE_CHARS)return <details><summary>{t('文件较大，展开查看完整内容')}（{result.output.length.toLocaleString()} {t('字符')}）</summary><pre className="workbench-git-output">{result.output}</pre></details>
  return <pre className="workbench-git-output">{result.output}</pre>
}
