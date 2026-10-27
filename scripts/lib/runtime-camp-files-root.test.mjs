import assert from 'node:assert/strict'
import test from 'node:test'
import { createHash } from 'node:crypto'
import { access, chmod, mkdir, mkdtemp, rm, writeFile, symlink } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { basename, dirname, join } from 'node:path'
import {
  coreDataDirectoryArguments,
  removeEphemeralRuntimeCampFilesRoot,
  runtimeCampFilesRootForDataDirectory
} from './runtime-camp-files-root.mjs'

test('derives a full domain-separated instance key for macOS data directories', () => {
  const root = runtimeCampFilesRootForDataDirectory('/tmp/coaxi-user-data', {
    platform: 'darwin',
    homeDirectory: '/tmp/coaxi-home'
  })
  const expectedKey = `v1-${createHash('sha256')
    .update('coaxi-runtime-camp-files-instance-v1\0/tmp/coaxi-user-data')
    .digest('hex')}`
  assert.equal(
    root,
    `/tmp/coaxi-home/.coaxi/instances/${expectedKey}/runtime-files`
  )
})

test('keeps the Windows derived root inside the protected data directory', () => {
  assert.equal(
    runtimeCampFilesRootForDataDirectory('C:\\Coaxi AI\\Core', {
      platform: 'win32',
      homeDirectory: 'C:\\Users\\test'
    }),
    'C:\\Coaxi AI\\Core\\runtime-files'
  )
})

test('emits the two mandatory Core root arguments together', () => {
  const args = coreDataDirectoryArguments('/tmp/coaxi-script-data')
  assert.deepEqual(args.slice(0, 2), ['--data-dir', '/tmp/coaxi-script-data'])
  assert.equal(args[2], '--runtime-camp-files-root')
  assert.match(args[3], /\/\.coaxi\/instances\/v1-[0-9a-f]{64}\/runtime-files$/)
})

test('removes only a marked Runtime Files Root owned by a temporary data directory', {
  skip: process.platform === 'win32'
}, async () => {
  const fixture = await mkdtemp(join(tmpdir(), 'coaxi-runtime-root-cleanup-'))
  const homeDirectory = join(fixture, 'home')
  const temporaryDirectory = join(fixture, 'temporary')
  const dataDirectory = join(temporaryDirectory, 'fixture', 'data')
  await mkdir(dataDirectory, { recursive: true })
  const root = runtimeCampFilesRootForDataDirectory(dataDirectory, { homeDirectory })
  await mkdir(join(root, 'camps', 'camp', 'attachments'), { recursive: true })
  await writeFile(join(root, '.runtime-camp-files-root.json'), JSON.stringify({
    schemaVersion: 2,
    instanceKey: basename(dirname(root)),
    platform: process.platform === 'darwin' ? 'macos' : process.platform
  }))
  const output = join(dirname(root), 'attachments', 'camp')
  await mkdir(output, { recursive: true })
  await writeFile(join(output, 'unpublished.txt'), 'ordinary edited output')
  const external = join(fixture, 'keep.txt')
  await writeFile(external, 'external')
  await symlink(external, join(output, 'external-link'))
  await chmod(join(root, 'camps', 'camp'), 0o100)
  await chmod(join(root, 'camps'), 0o100)

  try {
    assert.equal(await removeEphemeralRuntimeCampFilesRoot(dataDirectory, {
      homeDirectory,
      temporaryDirectory
    }), true)
    await assert.rejects(access(root), { code: 'ENOENT' })
    await assert.rejects(access(output), { code: 'ENOENT' })
    await access(external)

    await mkdir(root, { recursive: true })
    await writeFile(join(root, '.runtime-camp-files-root.json'), JSON.stringify({
      schemaVersion: 3,
      instanceKey: basename(dirname(root)),
      platform: process.platform === 'darwin' ? 'macos' : process.platform
    }))
    await assert.rejects(
      removeEphemeralRuntimeCampFilesRoot(dataDirectory, {
        homeDirectory,
        temporaryDirectory
      }),
      /mismatched ownership marker/
    )
  } finally {
    await rm(fixture, { recursive: true, force: true })
  }
})

test('refuses cleanup for a data directory outside the declared temporary root', {
  skip: process.platform === 'win32'
}, async () => {
  const fixture = await mkdtemp(join(tmpdir(), 'coaxi-runtime-root-persistent-'))
  const dataDirectory = join(fixture, 'persistent', 'data')
  await mkdir(dataDirectory, { recursive: true })
  try {
    await assert.rejects(
      removeEphemeralRuntimeCampFilesRoot(dataDirectory, {
        homeDirectory: join(fixture, 'home'),
        temporaryDirectory: join(fixture, 'different-temporary-root')
      }),
      /non-temporary data directory/
    )
  } finally {
    await rm(fixture, { recursive: true, force: true })
  }
})
