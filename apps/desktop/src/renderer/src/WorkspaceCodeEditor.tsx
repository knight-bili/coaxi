import { useEffect, useMemo, useState } from 'react'
import CodeMirror from '@uiw/react-codemirror'
import { EditorView } from '@codemirror/view'
import { useOptionalFilePreview } from './FilePreviewContext'
import { loadSourceLanguageForFilename, sourceCodeMirrorTheme } from './source-preview'
import { useUiText } from './interface-language'

/** Editable counterpart of Coaxi's existing ReadonlyCodeViewer. */
export function WorkspaceCodeEditor({ path, text, onChange, onSave, readonly = false, visible = true,line }: {
  path: string; text: string; onChange?(text: string): void; onSave?(): void; readonly?: boolean; visible?: boolean;line?:number
}): React.JSX.Element {
  const t = useUiText(), preview = useOptionalFilePreview()
  const [language, setLanguage] = useState<Awaited<ReturnType<typeof loadSourceLanguageForFilename>>>(null)
  const [view, setView] = useState<EditorView | null>(null)
  useEffect(() => { let current = true; void loadSourceLanguageForFilename(path).then(value => { if(current)setLanguage(value) }).catch(() => {if(current)setLanguage(null)});return () => {current=false} }, [path])
  useEffect(() => { if(visible)view?.requestMeasure() }, [visible, view])
  useEffect(()=>{if(!visible||!view||!line)return;const target=view.state.doc.line(Math.min(Math.max(1,line),view.state.doc.lines));view.dispatch({selection:{anchor:target.from},effects:EditorView.scrollIntoView(target.from,{y:'center'})})},[view,visible,line])
  const extensions = useMemo(() => [EditorView.theme({ '&':{height:'100%',background:'transparent'}, '.cm-scroller':{overflow:'auto',fontFamily:'ui-monospace,SFMono-Regular,Consolas,monospace'}, '.cm-content':{minHeight:'100%'} }),
    EditorView.domEventHandlers({ keydown(event) { if((event.ctrlKey || event.metaKey) && event.key.toLowerCase()==='s' && onSave){event.preventDefault();onSave();return true};return false} }), ...(language ? [language] : [])], [language, onSave])
  return <CodeMirror className="workbench-code-editor" dir="ltr" value={text} height="100%" theme={sourceCodeMirrorTheme(preview?.resolvedTheme ?? 'day')}
    extensions={extensions} readOnly={readonly} editable={!readonly} onCreateEditor={setView} onChange={onChange}
    aria-label={t('文件内容：{0}', path)} basicSetup={{lineNumbers:true,foldGutter:true,highlightActiveLine:!readonly,history:true,searchKeymap:true,autocompletion:!readonly}} />
}
