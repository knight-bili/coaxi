import {createRoot} from 'react-dom/client'
import {ProjectLauncher} from './ProjectLauncher'
import {applyRendererPlatform} from './renderer-platform'
import {readErrorMessage} from './error-message'
import {applyAppearanceSnapshot} from './theme'
import {acceptPersistedInterfaceLanguage} from './interface-language'
import './styles.css'
const api=window.coaxiLauncher
const presentation=(value:Awaited<ReturnType<typeof api.bootstrap>>):void=>{applyRendererPlatform(document.documentElement,value.platform);if(value.appearance)applyAppearanceSnapshot(document.documentElement,value.appearance);if(value.language)acceptPersistedInterfaceLanguage({interfaceLanguage:value.language})}
void api.bootstrap().then(value=>{presentation(value);createRoot(document.getElementById('root')!).render(<ProjectLauncher initialDirectory={value.directory} request={input=>api.request(input)} chooseDirectory={()=>api.chooseDirectory()} onEnter={commandId=>api.enter(commandId)}/>)}).catch(error=>{document.getElementById('root')!.textContent=readErrorMessage(error)})
window.addEventListener('focus',()=>{void api.bootstrap().then(presentation).catch(()=>{})})
