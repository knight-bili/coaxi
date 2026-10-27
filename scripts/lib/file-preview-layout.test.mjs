import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import { once } from 'node:events'
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import test from 'node:test'
import react from '@vitejs/plugin-react'
import electron from 'electron'
import { build } from 'vite'
import { admitElectronIntegrationTest } from './electron-sandbox-capability.mjs'

const root = resolve(import.meta.dirname, '../..')
const fixtureSource = join(root, 'scripts/fixtures/file-preview-layout')

test('production file preview keeps split geometry, reading state and stable preferences through native input', { timeout: 180_000 }, async (t) => {
  if (!admitElectronIntegrationTest(t)) return
  const fixture = await mkdtemp(join(tmpdir(), 'coaxi-file-preview-layout-test-'))
  let child
  let closed
  try {
    const shortcutModule = join(fixture, 'shortcut/close-tab-shortcut.cjs')
    const shortcutPreload = join(fixture, 'close-tab-preload.cjs')
    await build({ configFile: false, logLevel: 'error', build: { ssr: join(root, 'apps/desktop/src/shared/close-tab-shortcut.ts'), outDir: join(fixture, 'shortcut'), minify: false, rollupOptions: { output: { format: 'cjs', entryFileNames: 'close-tab-shortcut.cjs' } } } })
    const shortcutCode = await readFile(shortcutModule, 'utf8')
    await build({ configFile: false, logLevel: 'error', ssr: { noExternal: ['parse5', 'entities'] }, build: { ssr: join(root, 'packages/html-preview/src/site.ts'), outDir: join(fixture, 'site'), minify: false, rollupOptions: { output: { format: 'cjs', entryFileNames: 'site.cjs' } } } })
    await build({ configFile: false, logLevel: 'error', build: { ssr: join(root, 'packages/html-preview/src/file-source.ts'), outDir: join(fixture, 'source'), minify: false, rollupOptions: { output: { format: 'cjs', entryFileNames: 'source.cjs' } } } })
    await writeFile(shortcutPreload, `${shortcutCode}\nconst { contextBridge, ipcRenderer } = require('electron')
contextBridge.exposeInMainWorld('previewHtmlFixture', { prepare: () => ipcRenderer.invoke('html-fixture-prepare'), release: () => ipcRenderer.invoke('html-fixture-release') })
contextBridge.exposeInMainWorld('previewWindowControls', {
  onCloseTabRequested: exports.createCloseTabShortcutHandler(ipcRenderer)
})`)
    await build({
      configFile: false, root: fixtureSource, base: './', logLevel: 'error', plugins: [react()],
      resolve: { alias: { '@contracts': join(root, 'packages/contracts/src/index.ts') } },
      build: { outDir: join(fixture, 'renderer'), minify: false }
    })
    const environment = { ...process.env, ELECTRON_DISABLE_SECURITY_WARNINGS: 'true' }
    delete environment.ELECTRON_RUN_AS_NODE
    process.stdout.write(`Isolated file preview userData: ${join(fixture, 'user-data')} (no Core/Runtime)\n`)
    child = spawn(electron, [
      join(fixtureSource, 'main.cjs'), join(fixture, 'renderer/index.html'), join(fixture, 'user-data'),
      shortcutModule, shortcutPreload, join(fixture, 'site/site.cjs'), join(fixture, 'source/source.cjs'),
      ...(process.platform === 'linux' ? ['--no-sandbox'] : [])
    ], { env: environment, stdio: ['ignore', 'pipe', 'pipe'] })
    closed = once(child, 'close')
    let output = ''
    child.stdout.on('data', chunk => { output += chunk.toString() })
    child.stderr.on('data', chunk => { output += chunk.toString() })
    const timeout = setTimeout(() => child.kill('SIGKILL'), 150_000)
    let code
    try { [code] = await closed } finally { clearTimeout(timeout) }
    assert.equal(code, 0, `File preview interaction regression failed:\n${output}`)
    const report = JSON.parse(output.split('\n').find(line => line.startsWith('{')))
    assert.equal(report.ok, true)
    await writeFile(join(fixture,'results.json'),JSON.stringify(report,null,2))
  } finally {
    if (child && child.exitCode === null && child.signalCode === null) {
      child.kill('SIGKILL')
      await closed
    }
    if (process.env.COAXI_KEEP_FILE_PREVIEW_FIXTURE === '1') {
      process.stdout.write(`File preview fixture: ${fixture}\n`)
    } else {
      await rm(fixture, { recursive: true, force: true })
    }
  }
})
