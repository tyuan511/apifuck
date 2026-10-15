import fs from 'node:fs'
import path from 'node:path'
import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { assetName, linuxAppImage, digest } from './artifacts.mjs'
import { workspaceVersion } from './version.mjs'

// Pin the published tool bytes, not the moving continuous release.
const toolUrl = 'https://github.com/linuxdeploy/linuxdeploy/releases/download/1-alpha-20251107-1/linuxdeploy-x86_64.AppImage'
const toolSha256 = 'c20cd71e3a4e3b80c3483cef793cda3f4e990aca14014d23c544ca3ce1270b4d'
function run(command, args, options = {}) {
  const result = spawnSync(command, args, { encoding: 'utf8', maxBuffer: 16 * 1024 * 1024, ...options })
  if (result.error || result.status !== 0) throw new Error(`${path.basename(command)} failed: ${result.stderr || result.stdout || result.error}`)
  return result.stdout
}

export function prepareAppDir({ binary, icon, appdir }) {
  fs.mkdirSync(path.join(appdir, 'usr/bin'), { recursive: true })
  fs.copyFileSync(binary, path.join(appdir, 'usr/bin/apifuck-desktop'))
  fs.chmodSync(path.join(appdir, 'usr/bin/apifuck-desktop'), 0o755)
  fs.copyFileSync(icon, path.join(appdir, 'apifuck.png'))
  fs.writeFileSync(path.join(appdir, 'apifuck.desktop'), `[Desktop Entry]
Type=Application
Name=ApiFuck
Exec=apifuck-desktop
Icon=apifuck
Categories=Development;Network;
Terminal=false
Comment=Native API workbench
`)
  fs.writeFileSync(path.join(appdir, 'AppRun'), `#!/bin/sh
APPDIR="\${APPDIR:-$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)}"
export APPDIR
export LD_LIBRARY_PATH="$APPDIR/usr/lib\${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
exec "$APPDIR/usr/bin/apifuck-desktop" "$@"
`, { mode: 0o755 })
}

export function verifyAppImage(file) {
  const fd = fs.openSync(file, 'r')
  const header = Buffer.alloc(64)
  try { if (fs.readSync(fd, header, 0, 64, 0) !== 64) throw new Error('Incomplete AppImage') }
  finally { fs.closeSync(fd) }
  if (!header.subarray(0, 4).equals(Buffer.from([127, 69, 76, 70])) || !header.subarray(8, 11).equals(Buffer.from([65, 73, 2])) || header[4] !== 2 || header[5] !== 1 || header.readUInt16LE(18) !== 62) throw new Error('Expected a Linux x86_64 type-2 AppImage')
}

export async function packageLinux(root = process.cwd()) {
  if (process.platform !== 'linux' || process.arch !== 'x64') throw new Error('AppImage packaging requires Linux x86_64')
  const cache = path.join(root, 'target/packaging-tools')
  fs.mkdirSync(cache, { recursive: true })
  const tool = path.join(cache, 'linuxdeploy-x86_64.AppImage')
  if (!fs.existsSync(tool)) {
    const response = await fetch(toolUrl)
    if (!response.ok) throw new Error(`linuxdeploy download failed: ${response.status}`)
    fs.writeFileSync(tool, Buffer.from(await response.arrayBuffer()))
  }
  if (digest(tool) !== toolSha256) throw new Error('linuxdeploy checksum mismatch')
  fs.chmodSync(tool, 0o755)
  const work = fs.mkdtempSync(path.join(root, 'target/appimage-'))
  const appdir = path.join(work, 'ApiFuck.AppDir')
  const output = path.join(root, 'release-assets', linuxAppImage)
  fs.mkdirSync(path.dirname(output), { recursive: true })
  const binary = path.join(work, 'apifuck-desktop')
  try {
    fs.copyFileSync(path.join(root, 'target/release-staging', assetName('x86_64-unknown-linux-gnu')), binary)
    fs.chmodSync(binary, 0o755)
    prepareAppDir({ binary, icon: path.join(root, 'assets/apifuck.png'), appdir })
    run(tool, ['--appimage-extract-and-run', '--appdir', appdir, '--executable', binary, '--desktop-file', path.join(appdir, 'apifuck.desktop'), '--icon-file', path.join(appdir, 'apifuck.png'), '--output', 'appimage'], {
      cwd: work,
      env: { ...process.env, APPIMAGE_EXTRACT_AND_RUN: '1', LDAI_OUTPUT: output, LDAI_VERSION: workspaceVersion(root), LDAI_NO_APPSTREAM: '1', NO_STRIP: '1' },
    })
    verifyAppImage(output)
    fs.chmodSync(output, 0o755)
    // Exercise the AppImage runtime and inspect the installed tree, never start the GUI.
    run(output, ['--appimage-extract'], { cwd: work })
    const extracted = path.join(work, 'squashfs-root')
    for (const name of ['AppRun', 'apifuck.desktop', 'apifuck.png', 'usr/bin/apifuck-desktop']) {
      if (!fs.existsSync(path.join(extracted, name))) throw new Error(`AppImage is missing ${name}`)
    }
    const dependencies = run('ldd', [path.join(extracted, 'usr/bin/apifuck-desktop')], { env: { ...process.env, LD_LIBRARY_PATH: path.join(extracted, 'usr/lib') } })
    if (dependencies.includes('not found')) throw new Error(`AppImage has unresolved libraries: ${dependencies}`)
    fs.writeFileSync(`${output}.sha256`, `${digest(output)}  ${linuxAppImage}\n`)
    console.log(`Verified AppImage structure and shared libraries: ${output}`)
  } finally {
    fs.rmSync(work, { recursive: true, force: true })
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  packageLinux().catch(error => { console.error(error.message); process.exitCode = 1 })
}
