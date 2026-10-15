import fs from 'node:fs'
import path from 'node:path'
import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { workspaceVersion, parseVersion } from './version.mjs'

export const bundleName = 'ApiFuck.app'
export const bundleIdentifier = 'dev.fastvibe.apifuck'

export function runTool(command, args) {
  const result = spawnSync(command, args, { encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 })
  if (result.error || result.status !== 0) throw new Error(`${command} ${args[0]} failed: ${result.stderr || result.error || result.status}`)
  return result.stdout.trim()
}

// Only command-line tools: packaging must never open Finder or run UI scripts.
export function createApp({ root, binary, directory, run = runTool }) {
  const version = workspaceVersion(root)
  const numericVersion = parseVersion(version).parts.join('.')
  const build = run('xcrun', ['vtool', '-show-build', binary])
  const minimum = build.match(/\bminos\s+(\d+(?:\.\d+){1,2})\b/)?.[1]
  if (!minimum) throw new Error('Cannot determine the binary minimum macOS version')
  const app = path.join(directory, bundleName)
  const contents = path.join(app, 'Contents')
  if (fs.existsSync(app)) throw new Error(`Bundle already exists: ${app}`)
  fs.mkdirSync(path.join(contents, 'MacOS'), { recursive: true })
  fs.mkdirSync(path.join(contents, 'Resources'))
  fs.copyFileSync(binary, path.join(contents, 'MacOS/apifuck-desktop'))
  fs.chmodSync(path.join(contents, 'MacOS/apifuck-desktop'), 0o755)
  const values = {
    CFBundleInfoDictionaryVersion: '6.0', CFBundlePackageType: 'APPL',
    CFBundleIdentifier: bundleIdentifier, CFBundleName: 'ApiFuck', CFBundleDisplayName: 'ApiFuck',
    CFBundleExecutable: 'apifuck-desktop', CFBundleIconFile: 'ApiFuck.icns',
    CFBundleShortVersionString: numericVersion, CFBundleVersion: numericVersion,
    ApiFuckVersion: version, LSMinimumSystemVersion: minimum,
    NSPrincipalClass: 'NSApplication', LSApplicationCategoryType: 'public.app-category.developer-tools',
  }
  const entries = Object.entries(values).map(([key, value]) => `  <key>${key}</key><string>${value}</string>`).join('\n')
  fs.writeFileSync(path.join(contents, 'Info.plist'), `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
${entries}
  <key>NSHighResolutionCapable</key><true/>
</dict></plist>\n`)
  run('plutil', ['-lint', path.join(contents, 'Info.plist')])
  const iconset = fs.mkdtempSync(path.join(directory, 'icons-')) + '.iconset'
  fs.mkdirSync(iconset)
  try {
    for (const size of [16, 32, 128, 256, 512]) {
      for (const scale of [1, 2]) {
        run('sips', ['-z', String(size * scale), String(size * scale), path.join(root, 'assets/apifuck.png'), '--out', path.join(iconset, `icon_${size}x${size}${scale === 2 ? '@2x' : ''}.png`)])
      }
    }
    run('iconutil', ['-c', 'icns', iconset, '-o', path.join(contents, 'Resources/ApiFuck.icns')])
  } finally {
    fs.rmSync(iconset, { recursive: true, force: true })
    fs.rmSync(iconset.slice(0, -'.iconset'.length), { recursive: true, force: true })
  }
  return app
}

export function createDmg({ app, destination, directory, run = runTool }) {
  const source = fs.mkdtempSync(path.join(directory, 'dmg-'))
  try {
    run('ditto', [app, path.join(source, bundleName)])
    fs.symlinkSync('/Applications', path.join(source, 'Applications'))
    run('hdiutil', ['create', '-volname', 'ApiFuck', '-srcfolder', source, '-format', 'UDZO', '-fs', 'HFS+', destination])
    run('hdiutil', ['verify', destination])
  } finally {
    fs.rmSync(source, { recursive: true, force: true })
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    if (process.platform !== 'darwin') throw new Error('macOS packaging requires macOS')
    const [binary, output] = process.argv.slice(2)
    if (!binary || !output) throw new Error('Usage: node scripts/release/macos-package.mjs <binary> <new-output-directory>')
    const directory = path.resolve(output)
    fs.mkdirSync(directory) // Do not overwrite an existing build.
    const app = createApp({ root: process.cwd(), binary: path.resolve(binary), directory })
    // Local structural validation only; production signing is handled by macos-sign.mjs.
    createDmg({ app, destination: path.join(directory, 'ApiFuck.dmg'), directory })
    console.log(`Unsigned local bundle and DMG: ${directory}`)
  } catch (error) {
    console.error(error.message)
    process.exitCode = 1
  }
}
