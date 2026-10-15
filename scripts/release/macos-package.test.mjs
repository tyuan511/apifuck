import assert from 'node:assert/strict'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { test } from 'node:test'
import { createApp, createDmg, runTool } from './macos-package.mjs'

test('native tools build a valid application and mountable DMG without launching the app', { skip: process.platform !== 'darwin' }, t => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'apifuck-package-'))
  t.after(() => fs.rmSync(root, { recursive: true, force: true }))
  fs.mkdirSync(path.join(root, 'assets'))
  fs.copyFileSync(fileURLToPath(new URL('../../assets/apifuck.png', import.meta.url)), path.join(root, 'assets/apifuck.png'))
  fs.writeFileSync(path.join(root, 'Cargo.toml'), '[workspace.package]\nversion = "1.2.3-beta.1"\n')
  const binary = path.join(root, 'fixture')
  const source = path.join(root, 'fixture.c')
  fs.writeFileSync(source, 'int main(void) { return 0; }\n')
  runTool('xcrun', ['clang', '-mmacosx-version-min=11.0', source, '-o', binary])
  const app = createApp({ root, binary, directory: root })
  const info = JSON.parse(runTool('plutil', ['-convert', 'json', '-o', '-', path.join(app, 'Contents/Info.plist')]))
  assert.equal(info.CFBundleIdentifier, 'dev.fastvibe.apifuck')
  assert.equal(info.CFBundleExecutable, 'apifuck-desktop')
  assert.equal(info.CFBundleShortVersionString, '1.2.3')
  assert.equal(info.ApiFuckVersion, '1.2.3-beta.1')
  assert.equal(info.LSMinimumSystemVersion, '11.0')
  assert.ok(fs.statSync(path.join(app, 'Contents/MacOS/apifuck-desktop')).mode & 0o111)
  assert.ok(fs.statSync(path.join(app, 'Contents/Resources/ApiFuck.icns')).size > 0)
  assert.throws(() => createApp({ root, binary, directory: root }), /already exists/)
  const dmg = path.join(root, 'ApiFuck.dmg')
  createDmg({ app, destination: dmg, directory: root })
  const mount = path.join(root, 'mounted')
  fs.mkdirSync(mount)
  runTool('hdiutil', ['attach', '-readonly', '-nobrowse', '-noautoopen', '-mountpoint', mount, dmg])
  try {
    assert.equal(fs.readlinkSync(path.join(mount, 'Applications')), '/Applications')
    assert.deepEqual(fs.readFileSync(path.join(mount, 'ApiFuck.app/Contents/MacOS/apifuck-desktop')), fs.readFileSync(binary))
    assert.ok(fs.existsSync(path.join(mount, 'ApiFuck.app/Contents/Resources/ApiFuck.icns')))
  } finally {
    runTool('hdiutil', ['detach', mount])
  }
})
