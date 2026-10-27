import { BrowserWindow, screen } from 'electron'
import { randomBytes } from 'node:crypto'
import type { InteractionComputer } from './interaction-computer'

const actions:Record<string,string>={click:'点击',drag:'拖动',scroll:'滚动',type:'输入',key:'按键','hold-key':'按住按键','set-value':'设置值',restore:'还原窗口',menu:'选择菜单','set-frame':'调整窗口',launch:'启动应用','clipboard-write':'写入剪贴板'}
const escape=(value:string):string=>value.replace(/[&<>"']/g,char=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[char]!))

/** Coaxi computer-indicator 的独立窄提示条、四秒动作提示与直接停止。 */
export class InteractionComputerIndicator {
  private window:BrowserWindow|null=null
  private readonly stopUrl=`coaxi-interaction://stop/${randomBytes(32).toString('hex')}`
  private readonly timer:ReturnType<typeof setInterval>
  private disposed=false
  private busy=false
  private mark=''
  private stopping=false
  constructor(private readonly computer:InteractionComputer,private readonly owner:()=>BrowserWindow|null,private readonly stop:()=>Promise<void>){
    this.timer=setInterval(()=>{void this.refresh().catch(()=>undefined)},500);this.timer.unref()
  }
  private create():BrowserWindow{
    const window=new BrowserWindow({width:680,height:46,show:false,frame:false,transparent:true,resizable:false,minimizable:false,maximizable:false,skipTaskbar:true,focusable:false,alwaysOnTop:true,hasShadow:false,webPreferences:{sandbox:true,contextIsolation:true,nodeIntegration:false,webSecurity:true,devTools:false}})
    window.setAlwaysOnTop(true,'floating');window.setVisibleOnAllWorkspaces(true,{visibleOnFullScreen:true})
    window.webContents.setWindowOpenHandler(()=>({action:'deny'}))
    window.webContents.on('will-navigate',(event,url)=>{event.preventDefault();if(url!==this.stopUrl||this.stopping||this.disposed)return;this.stopping=true;void this.stop().finally(()=>{this.stopping=false;this.mark='';void this.refresh()}).catch(()=>undefined)})
    window.on('closed',()=>{if(this.window===window)this.window=null;this.mark=''})
    this.window=window;return window
  }
  private async refresh():Promise<void>{
    if(this.disposed||this.busy)return;this.busy=true
    try{
      const snapshot=await this.computer.monitor()
      if(this.disposed)return
      if(!snapshot.enabled||!snapshot.grants.length){this.window?.hide();this.mark='';return}
      const control=snapshot.grants.some(grant=>grant.level==='control'),desktop=snapshot.grants.some(grant=>grant.targetKind==='computer_desktop'),count=new Set(snapshot.grants.map(grant=>`${grant.targetKind}:${grant.targetId}`)).size
      const summary=desktop?(control?'队员可操作整个桌面':'队员可查看整个桌面'):`队员可${control?'操作':'查看'} ${count} 个共享资源`
      const latest=snapshot.activity.find(item=>item.outcome==='done'&&actions[item.action]),recent=latest&&Date.now()-latest.at<4000?`${actions[latest.action]}${latest.app?` · ${latest.app}`:''}`:''
      const content=JSON.stringify({summary,recent,shortcut:snapshot.stopShortcut,stopping:this.stopping,error:snapshot.lastError})
      if(content===this.mark)return
      this.mark=content
      const window=this.window??this.create(),owner=this.owner(),display=owner&&!owner.isDestroyed()?screen.getDisplayMatching(owner.getBounds()):screen.getPrimaryDisplay()
      window.setPosition(Math.round(display.workArea.x+(display.workArea.width-680)/2),display.workArea.y+10,false)
      const document=`<!doctype html><html lang="zh-CN"><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'"><style>html,body{background:transparent!important;margin:0;font:12px 'Segoe UI',sans-serif}body{display:flex;justify-content:center;overflow:hidden}.strip{box-sizing:border-box;display:flex;width:max-content;max-width:668px;align-items:center;gap:8px;margin:6px;border:1px solid #333;border-radius:999px;background:#171717;color:#ededed;padding:4px 4px 4px 12px;box-shadow:0 2px 6px #0006;white-space:nowrap}.dot{width:8px;height:8px;border-radius:50%;flex:none;background:${control?'#ef4444':'#8b5cf6'}}.summary,.recent{min-width:0;overflow:hidden;text-overflow:ellipsis}.recent{color:#a3a3a3}a{color:#fee2e2;background:#7f1d1d;border-radius:999px;text-decoration:none;padding:4px 9px;flex:none}kbd{font:10px 'Segoe UI',sans-serif;opacity:.8}</style><div class="strip"><span class="dot"></span><span class="summary">${escape(summary)}</span>${recent?`<span class="recent">${escape(recent)}</span>`:''}<a href="${this.stopUrl}" title="${escape(snapshot.lastError??'停止并收回全部电脑共享')}">${this.stopping?'正在停止':'■ 停止'}${snapshot.stopShortcut?` <kbd>${escape(snapshot.stopShortcut)}</kbd>`:''}</a></div></html>`
      await window.loadURL(`data:text/html;charset=utf-8,${encodeURIComponent(document)}`)
      if(!this.disposed&&!window.isDestroyed())window.showInactive()
    }finally{this.busy=false}
  }
  dispose():void{this.disposed=true;clearInterval(this.timer);this.window?.destroy();this.window=null}
}
