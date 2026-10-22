// 依据固定 Codeg computer/stop_shortcut.rs 的闭集和强度要求。
const punctuation:Record<string,string>={Minus:'-',Equal:'=',BracketLeft:'[',BracketRight:']',Backslash:'\\',Semicolon:';',Quote:"'",Backquote:'`',Comma:',',Period:'.',Slash:'/'}
export const defaultInteractionStopShortcut=(platform:string):string=>platform==='darwin'?'Control+Command+Escape':'Control+Alt+Escape'
export function interactionStopAccelerator(spelling:string,platform:string):string|null{
  if(spelling==='')return null
  const parts=spelling.split('+'),code=parts.pop()??'',allowed=['Control','Alt','Shift','Command']
  if(new Set(parts).size!==parts.length||parts.some(part=>!allowed.includes(part))||parts.length<2||!parts.some(part=>part==='Control'||part==='Command')||platform!=='darwin'&&parts.includes('Command'))throw new Error('computer.invalid_stop_shortcut')
  const key=/^Key[A-Z]$/.test(code)?code.slice(3):/^Digit[0-9]$/.test(code)?code.slice(5):/^F([1-9]|1[0-2])$/.test(code)||code==='Escape'?code:punctuation[code]
  if(!key)throw new Error('computer.invalid_stop_shortcut')
  return [...allowed.filter(part=>parts.includes(part)),key].join('+')
}
