import { spawnSync } from 'node:child_process'
import { lstatSync, readFileSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { inspectElfBinary } from './lib/linux-server-abi.mjs'

// Runs only on explicit packaging. This script neither installs a package nor publishes a release.
if (process.platform !== 'linux' || process.arch !== 'x64') throw new Error('Linux Desktop packaging requires an x86_64 Linux host')
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const kind = process.argv[2] ?? 'portable'
const targets = { dir: ['dir'], portable: ['AppImage', 'tar.gz'], appimage: ['AppImage'], tar: ['tar.gz'], deb: ['deb'], all: ['AppImage', 'tar.gz', 'deb'] }[kind]
if (!targets) throw new Error('Expected dir, portable, appimage, tar, deb or all')
const metadata = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8'))
if (metadata.build.appId !== 'app.coaxi.desktop' || metadata.name !== 'coaxi') throw new Error('Linux package must use the fusion installation identity')
for (const name of ['coaxi-core', 'coaxi-host', 'coaxi', 'coaxi-native-history']) {
  const binary = join(root, 'resources/bin/linux-x64', name)
  const info = lstatSync(binary)
  if (!info.isFile() || info.isSymbolicLink() || !(info.mode & 0o111)) throw new Error(`Missing executable Linux sidecar: ${name}`)
  inspectElfBinary(binary)
}
const args = ['exec', 'electron-builder', '--linux', ...targets, '--x64', '--publish', 'never']
if (targets.includes('deb')) {
  const maintainer = process.env.WORKBENCH_PACKAGE_MAINTAINER
  if (!maintainer || !/^[^<>\r\n]+ <[^<>\s@]+@[^<>\s@]+>$/.test(maintainer)) throw new Error('Debian packaging requires WORKBENCH_PACKAGE_MAINTAINER in Name <email> form')
  args.push(`-c.deb.maintainer=${maintainer}`)
}
const result = spawnSync('pnpm', args, { cwd: root, stdio: 'inherit', env: process.env })
if (result.error) throw result.error
process.exitCode = result.status ?? 1
