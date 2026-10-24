import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const read = file => readFileSync(new URL(file, import.meta.url), 'utf8')

test('Windows computer executor ships its helper beside the Core that locates it', () => {
  const metadata = JSON.parse(read('../../package.json'))
  assert.ok(metadata.build.win.extraResources.some(resource =>
    resource.from === 'resources/bin/windows-x64/rovai-computer-helper.exe'
      && resource.to === 'bin/rovai-computer-helper.exe'))
  const build = read('../build-core.mjs')
  assert.match(build, /'--package', 'rovai-computer'/u)
  assert.match(build, /for \(const binary of \[[^\]]*'rovai-computer-helper'/u)
  assert.match(read('../after-pack.cjs'), /stampWindowsExecutable\(join\(bundledBin, 'rovai-computer-helper\.exe'\)/u)
})

test('Windows release verification rejects a missing helper and probes the installed binary', () => {
  const verifier = read('../verify-windows-release.mjs')
  assert.match(verifier, /verifyBinary\('rovai-computer-helper', computerHelperExecutable\)/u)
  assert.match(verifier, /run\(computerHelperExecutable, \['--request-permission', 'accessibility'\]/u)
  assert.match(verifier, /helperPermission\.prompted !== false/u)
})
