import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import { once } from 'node:events'
import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import test from 'node:test'
import react from '@vitejs/plugin-react'
import electron from 'electron'
import { build } from 'vite'
import { admitElectronIntegrationTest } from './electron-sandbox-capability.mjs'

test('delegated viewer preserves child authority, non-modal stacked drawers and source-row-independent lifetime', { timeout: 90_000 }, t => runFixture(t))
test('taken-over child chat preserves exact owner, input recovery and embedded drawer layout', { timeout: 90_000 }, t => runFixture(t, '--chat'))
async function runFixture(t, mode = '--readonly') {
  if (!admitElectronIntegrationTest(t)) return
  const root = resolve(import.meta.dirname, '../..'), source = join(root, 'scripts/fixtures/delegation-viewer')
  const fixture = await mkdtemp(join(tmpdir(), 'coaxi-delegation-viewer-'))
  let child, closed
  try {
    await build({ configFile: false, root: source, base: './', logLevel: 'error', plugins: [react()],
      resolve: { alias: { '@contracts': join(root, 'packages/contracts/src/index.ts') } }, build: { outDir: join(fixture, 'renderer'), minify: false } })
    const env = { ...process.env, ELECTRON_DISABLE_SECURITY_WARNINGS: 'true' }; delete env.ELECTRON_RUN_AS_NODE
    process.stdout.write(`Automatic verification: ${fixture}; Renderer-only userData/session, no Core/Runtime/SQLite/Skill Library/MCP\n`)
    child = spawn(electron, [join(source, 'main.cjs'), join(fixture, 'renderer/index.html'), join(fixture, 'user-data'), mode, ...(process.platform === 'linux' ? ['--no-sandbox'] : [])], { env, stdio: ['ignore', 'pipe', 'pipe'] })
    closed = once(child, 'close')
    let output = ''
    child.stdout.on('data', data => { output += data.toString() }); child.stderr.on('data', data => { output += data.toString() })
    const timeout = setTimeout(() => child.kill('SIGKILL'), 75_000)
    let code
    try { [code] = await closed } finally { clearTimeout(timeout) }
    assert.equal(code, 0, output)
    const result = output.split('\n').find(line => line.startsWith('{'))
    assert.equal(JSON.parse(result).ok, true); process.stdout.write(result + '\n')
  } finally {
    if (child && child.exitCode === null && child.signalCode === null) { child.kill('SIGKILL'); await closed }
    if (process.env.COAXI_KEEP_DELEGATION_VIEWER_FIXTURE === '1') process.stdout.write(`Viewer screenshots: ${fixture}\n`)
    else await rm(fixture, { recursive: true, force: true })
  }
}
