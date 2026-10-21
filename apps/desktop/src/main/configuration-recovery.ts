import { spawn } from 'node:child_process'
import { resolveCoreBinary } from './core-client'

/** Runs before appearance/navigation stores read the files being restored. */
export async function recoverLocalDataBeforeStores(dataRoot: string): Promise<void> {
  await new Promise<void>((resolve, reject) => {
    const env: NodeJS.ProcessEnv = { LANG: 'C.UTF-8', LC_ALL: 'C.UTF-8' }
    for (const key of ['PATH', 'SystemRoot', 'WINDIR', 'HOME', 'USERPROFILE', 'TEMP', 'TMP', 'LOCALAPPDATA']) if (process.env[key]) env[key] = process.env[key]
    const child = spawn(resolveCoreBinary(), ['recover-local-data', '--data-dir', dataRoot], { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'], env })
    let output = '', errors = ''
    child.stdout.setEncoding('utf8'); child.stderr.setEncoding('utf8')
    child.stdout.on('data', (chunk: string) => { output += chunk; if (output.length > 65536) child.kill() })
    child.stderr.on('data', (chunk: string) => { errors = (errors + chunk).slice(-16000) })
    child.once('error', reject)
    child.once('close', code => { if (code !== 0) { reject(new Error(errors || '本机数据恢复未能完成，安全副本仍保留。')); return }; try { const value = JSON.parse(output); if (typeof value.configurationRecovered !== 'boolean' || typeof value.backupRestored !== 'boolean') throw Error('invalid'); resolve() } catch { reject(new Error('本机数据恢复的结果无法确认。')) } })
  })
}
