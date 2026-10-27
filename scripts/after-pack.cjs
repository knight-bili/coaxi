const { execFileSync } = require('node:child_process')
const { chmodSync } = require('node:fs')
const { join } = require('node:path')
const { recordWorkbenchPackage } = require('./lib/workbench-package.cjs')

module.exports = async function afterPack(context) {
  if (context.electronPlatformName === 'win32') {
    const { stampWindowsExecutable } = await import('./lib/windows-pe-resources.mjs')
    const appFilename = `${context.packager.appInfo.productFilename}.exe`
    const bundledBin = join(context.appOutDir, 'resources', 'bin')
    const iconPath = await context.packager.getIconPath()
    const version = context.packager.appInfo.version
    await stampWindowsExecutable(join(context.appOutDir, appFilename), {
      version,
      description: 'Coaxi Desktop',
      originalFilename: appFilename,
      iconPath
    })
    await stampWindowsExecutable(join(bundledBin, 'coaxi-core.exe'), {
      version,
      description: 'Coaxi Core',
      originalFilename: 'coaxi-core.exe',
      iconPath
    })
    await stampWindowsExecutable(join(bundledBin, 'coaxi-host.exe'), {
      version,
      description: 'Coaxi Host',
      originalFilename: 'coaxi-host.exe',
      iconPath
    })
    await stampWindowsExecutable(join(bundledBin, 'coaxi.exe'), {
      version,
      description: 'Coaxi CLI',
      originalFilename: 'coaxi.exe',
      iconPath
    })
    await stampWindowsExecutable(join(bundledBin, 'coaxi-computer-helper.exe'), {
      version,
      description: 'Coaxi Computer Helper',
      originalFilename: 'coaxi-computer-helper.exe',
      iconPath
    })
    recordWorkbenchPackage(context)
    return
  }
  if (context.electronPlatformName === 'linux') {
    for (const name of ['coaxi-core', 'coaxi-host', 'coaxi', 'coaxi-native-history']) chmodSync(join(context.appOutDir, 'resources', 'bin', name), 0o755)
    recordWorkbenchPackage(context)
    return
  }
  if (context.electronPlatformName !== 'darwin') return

  const appName = `${context.packager.appInfo.productFilename}.app`
  const infoPlist = join(context.appOutDir, appName, 'Contents', 'Info.plist')
  const bundledBin = join(context.appOutDir, appName, 'Contents', 'Resources', 'bin')
  chmodSync(join(bundledBin, 'coaxi-core'), 0o755)
  chmodSync(join(bundledBin, 'coaxi-host'), 0o755)
  chmodSync(join(bundledBin, 'coaxi'), 0o755)
  chmodSync(join(bundledBin, 'coaxi-native-history'), 0o755)
  execFileSync('/usr/libexec/PlistBuddy', [
    '-c',
    'Set :NSAppTransportSecurity:NSAllowsArbitraryLoads false',
    infoPlist
  ])
  recordWorkbenchPackage(context)
}
