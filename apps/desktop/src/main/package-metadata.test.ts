import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'
import { PRODUCT_APPLICATION_ID, PRODUCT_NAME } from '../shared/product-identity'

const packageMetadata = JSON.parse(readFileSync(
  new URL('../../../../package.json', import.meta.url),
  'utf8'
))

describe('desktop package metadata', () => {
  it('keeps the unified product identity aligned with the Electron bundle metadata', () => {
    expect(PRODUCT_NAME).toBe('Coaxi')
    expect(PRODUCT_APPLICATION_ID).toBe('app.coaxi.desktop')
    expect(packageMetadata.productName).toBe(PRODUCT_NAME)
    expect(packageMetadata.version).toBe('0.4.8')
    expect(packageMetadata.build.appId).toBe(PRODUCT_APPLICATION_ID)
    expect(packageMetadata.build.productName).toBe(PRODUCT_NAME)
    expect(packageMetadata.build.mac.executableName).toBeUndefined()
    expect(packageMetadata.build.win.executableName).toBe('Coaxi')
    expect(packageMetadata.build.mac.artifactName).toBe('Coaxi-${version}-${arch}.${ext}')
    expect(packageMetadata.build.win.artifactName).toBe('Coaxi-${version}-${arch}.${ext}')
    expect(packageMetadata.build.mac.extendInfo.CFBundleDisplayName).toBe(PRODUCT_NAME)
    expect(packageMetadata.build.mac.extendInfo.CFBundleName).toBeUndefined()
  })

  it('packages GitHub update metadata and a macOS zip beside the install DMG', () => {
    expect(packageMetadata.build.publish).toEqual([{
      provider: 'github',
      owner: 'knight-bili',
      repo: 'coaxi',
      releaseType: 'draft'
    }])
    expect(packageMetadata.build.mac.target).toEqual(['dir', 'dmg', 'zip'])
    expect(packageMetadata.scripts['dist:mac:release:arm64']).toContain('--mac dmg zip')
    expect(packageMetadata.scripts['dist:mac:release:x64']).toContain('--mac dmg zip')
  })

  it('keeps local ad-hoc installation separate from Developer ID releases', () => {
    const localDailyCommand = packageMetadata.scripts['package:mac:daily']
    expect(localDailyCommand).toContain('CSC_IDENTITY_AUTO_DISCOVERY=false')
    expect(localDailyCommand).toContain('identity=-')
    expect(localDailyCommand).not.toContain('Coaxi Release Signing')
    expect(localDailyCommand).not.toContain('forceCodeSigning')
    expect(localDailyCommand).toContain('scripts/verify-macos-app.mjs arm64')

    const developerIdReleaseCommands = [
      packageMetadata.scripts['dist:mac:release:arm64'],
      packageMetadata.scripts['dist:mac:release:x64']
    ]
    for (const command of developerIdReleaseCommands) {
      expect(command).toContain('-c.forceCodeSigning=true')
      expect(command).toContain('-c.mac.notarize=true')
      expect(command).not.toContain('Coaxi Release Signing')
      expect(command).not.toContain('identity=-')
    }
    expect(packageMetadata.scripts['install:mac:daily']).toContain(
      'scripts/install-macos-daily.mjs'
    )
    expect(packageMetadata.scripts['package:mac:unsigned']).toContain('identity=-')
    expect(packageMetadata.scripts['package:mac']).toContain('identity=-')
  })

  it('does not build or package a native open-panel prewarmer', () => {
    expect(packageMetadata.scripts).not.toHaveProperty('native:build:macos')
    expect(packageMetadata.scripts.dev).not.toContain('native:build:macos')
    expect(packageMetadata.scripts['build:desktop']).not.toContain('native:build:macos')
    const packagedResources = [
      ...packageMetadata.build.mac.extraResources,
      ...packageMetadata.build.win.extraResources
    ]
    expect(JSON.stringify(packagedResources)).not.toMatch(/open.?panel|prewarm/iu)
  })

  it('packages target-specific sidecars and required release assets', () => {
    expect(packageMetadata.build.mac.extraResources).toEqual([
      {
        from: 'resources/bin/macos-${arch}/coaxi-core',
        to: 'bin/coaxi-core'
      },
      { from: 'resources/bin/macos-${arch}/coaxi-host', to: 'bin/coaxi-host' },
      {
        from: 'resources/bin/macos-${arch}/coaxi',
        to: 'bin/coaxi'
      },
      { from: 'out/web', to: 'web-ui' },
      { from: 'skills', to: 'skills' },
      {
        from: 'resources/bin/macos-${arch}/coaxi-native-history',
        to: 'bin/coaxi-native-history'
      },
      { from: 'helpers/native-history/LICENSE', to: 'licenses/native-history-LICENSE' },
      { from: 'LICENSE', to: 'licenses/coaxi-LICENSE' },
      { from: 'THIRD_PARTY_NOTICES.md', to: 'licenses/THIRD_PARTY_NOTICES.md' },
      { from: 'integration/licenses', to: 'licenses/integration' },
      { from: 'legal/licenses', to: 'licenses/dependencies' },
      { from: 'integration/sources.lock.json', to: 'workbench/sources.lock.json' },
      { from: 'integration/product-release.json', to: 'workbench/product-release.json' }
    ])
    expect(packageMetadata.build.win.extraResources).toEqual([
      {
        from: 'resources/bin/windows-x64/coaxi-core.exe',
        to: 'bin/coaxi-core.exe'
      },
      { from: 'resources/bin/windows-x64/coaxi-host.exe', to: 'bin/coaxi-host.exe' },
      {
        from: 'resources/bin/windows-x64/coaxi.exe',
        to: 'bin/coaxi.exe'
      },
      {
        from: 'resources/bin/windows-x64/coaxi-computer-helper.exe',
        to: 'bin/coaxi-computer-helper.exe'
      },
      { from: 'out/web', to: 'web-ui' },
      { from: 'skills', to: 'skills' },
      {
        from: 'resources/bin/windows-x64/coaxi-native-history.exe',
        to: 'bin/coaxi-native-history.exe'
      },
      { from: 'helpers/native-history/LICENSE', to: 'licenses/native-history-LICENSE' },
      { from: 'LICENSE', to: 'licenses/coaxi-LICENSE' },
      { from: 'THIRD_PARTY_NOTICES.md', to: 'licenses/THIRD_PARTY_NOTICES.md' },
      { from: 'integration/licenses', to: 'licenses/integration' },
      { from: 'legal/licenses', to: 'licenses/dependencies' },
      { from: 'integration/sources.lock.json', to: 'workbench/sources.lock.json' },
      { from: 'integration/product-release.json', to: 'workbench/product-release.json' }
    ])
    expect(packageMetadata.build).not.toHaveProperty('extraResources')
    expect(packageMetadata.build.mac.binaries).toEqual([
      'Contents/Resources/bin/coaxi-core',
      'Contents/Resources/bin/coaxi-host',
      'Contents/Resources/bin/coaxi',
      'Contents/Resources/bin/coaxi-native-history'
    ])
    expect(JSON.stringify(packageMetadata.build)).not.toMatch(/dws/iu)
    expect(Object.keys(packageMetadata.scripts).some((name) => name.startsWith('dws:'))).toBe(false)
    expect(packageMetadata.scripts.dev).not.toContain('dws')
    expect(packageMetadata.scripts.build).not.toContain('dws')
    expect(Object.keys(packageMetadata.scripts).some((name) => name.startsWith('legal:'))).toBe(false)
    expect(packageMetadata.scripts.test).not.toContain('legal:')
    expect(packageMetadata.scripts['package:mac:unsigned']).not.toContain('legal:')
    expect(packageMetadata.scripts['package:mac']).not.toContain('legal:')
    expect(packageMetadata.scripts['dist:mac']).not.toContain('legal:')
    expect(packageMetadata.scripts['dist:mac:release:arm64']).toContain(
      'pnpm build:macos:arm64'
    )
    expect(packageMetadata.scripts['build:macos:arm64']).not.toContain('dws')
    expect(packageMetadata.scripts['dist:mac:release:arm64']).not.toContain('legal:')
    expect(packageMetadata.scripts['dist:mac:release:x64']).toContain(
      'pnpm build:macos:x64'
    )
    expect(packageMetadata.scripts['build:macos:x64']).not.toContain('dws')
    expect(packageMetadata.scripts['dist:mac:release:x64']).not.toContain('legal:')
    expect(packageMetadata.scripts['package:windows:x64']).toContain('pnpm build:windows:x64')
    expect(packageMetadata.scripts['build:windows:x64']).not.toContain('dws')
    expect(packageMetadata.scripts['dist:windows:x64']).toContain('scripts/package-windows.mjs nsis')
    expect(packageMetadata.scripts['dist:windows:release:x64']).toContain('--require-signed')
    expect(packageMetadata.scripts['package:windows:x64']).not.toContain('legal:')
    expect(packageMetadata.scripts['dist:windows:x64']).not.toContain('legal:')
    expect(packageMetadata.scripts['dist:windows:release:x64']).not.toContain('legal:')
  })
})
