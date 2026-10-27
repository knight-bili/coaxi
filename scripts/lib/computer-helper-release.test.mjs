import assert from 'node:assert/strict'
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import test from 'node:test'
import { recordWorkbenchPackage } from './workbench-package.cjs'

const read = file => readFileSync(new URL(file, import.meta.url), 'utf8')

test('Windows computer executor ships its helper beside the Core that locates it', () => {
  const metadata = JSON.parse(read('../../package.json'))
  assert.ok(metadata.build.win.extraResources.some(resource =>
    resource.from === 'resources/bin/windows-x64/coaxi-computer-helper.exe'
      && resource.to === 'bin/coaxi-computer-helper.exe'))
  const build = read('../build-core.mjs')
  assert.match(build, /'--package', 'coaxi-computer'/u)
  assert.match(build, /for \(const binary of \[[^\]]*'coaxi-computer-helper'/u)
  assert.match(read('../after-pack.cjs'), /stampWindowsExecutable\(join\(bundledBin, 'coaxi-computer-helper\.exe'\)/u)
})

test('Windows release verification rejects a missing helper and probes the installed binary', () => {
  const verifier = read('../verify-windows-release.mjs')
  assert.match(verifier, /verifyBinary\('coaxi-computer-helper', computerHelperExecutable\)/u)
  assert.match(verifier, /run\(computerHelperExecutable, \['--request-permission', 'accessibility'\]/u)
  assert.match(verifier, /helperPermission\.prompted !== false/u)
})

test('package admission fails on a missing helper and records it only after it exists', t => {
  const directory = mkdtempSync(join(tmpdir(), 'coaxi-helper-package-'))
  t.after(() => rmSync(directory, { recursive: true, force: true }))
  const resources = join(directory, 'resources')
  const put = (relative, content = 'fixture') => {
    const file = join(resources, relative)
    mkdirSync(dirname(file), { recursive: true })
    writeFileSync(file, content)
  }
  put('workbench/product-release.json', JSON.stringify({ productId: 'coaxi', repository: 'https://github.com/knight-bili/coaxi', targetVersion: '1.0.0' }))
  put('workbench/sources.lock.json', JSON.stringify({ repository: 'knight-bili/coaxi' }))
  for (const name of ['coaxi-core', 'coaxi-host', 'coaxi', 'coaxi-native-history']) put(`bin/${name}.exe`)
  for (const name of ['web-ui/index.html', 'skills/cli-operations/SKILL.md', 'licenses/coaxi-LICENSE', 'licenses/THIRD_PARTY_NOTICES.md', 'licenses/integration/component-a/LICENSE', 'licenses/integration/component-b/LICENSE']) put(name)
  const context = { electronPlatformName: 'win32', appOutDir: directory, arch: 1, packager: { projectDir: fileURLToPath(new URL('../../', import.meta.url)), appInfo: { version: '0.4.8' } } }
  assert.throws(() => recordWorkbenchPackage(context), /coaxi-computer-helper\.exe/u)
  put('bin/coaxi-computer-helper.exe')
  recordWorkbenchPackage(context)
  assert.ok(JSON.parse(readFileSync(join(resources, 'workbench/package.json'), 'utf8')).nativeExecutables.includes('bin/coaxi-computer-helper.exe'))
})
