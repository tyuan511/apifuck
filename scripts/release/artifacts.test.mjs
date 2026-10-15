import assert from 'node:assert/strict'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { test } from 'node:test'
import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { digest, releaseAssetName, releaseAssetNames, macosDmg, linuxAppImage, verifyArtifacts, verifyUploadedAssets } from './artifacts.mjs'
import { assetNames } from '../../website/src/releases.mjs'

test('release tooling and website share the versioned platform naming contract', () => {
  const fixtures = JSON.parse(fs.readFileSync(new URL('./asset-name-fixtures.json', import.meta.url)))
  for (const { version, ...expected } of fixtures) {
    assert.deepEqual(assetNames(version), expected)
    assert.deepEqual(releaseAssetNames(version), [expected.mac, expected.windows, expected.linux])
  }
  assert.throws(() => releaseAssetName('x86_64-pc-windows-msvc', '../1.2.3'))
  assert.throws(() => releaseAssetName('unsupported', '1.2.3'))
})

test('Windows staging reads the workspace version for both the EXE and checksum', t => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'apifuck-stage-'))
  t.after(() => fs.rmSync(root, { recursive: true, force: true }))
  fs.writeFileSync(path.join(root, 'Cargo.toml'), '[workspace.package]\nversion = "4.5.6"\n')
  const binaryDirectory = path.join(root, 'target/x86_64-pc-windows-msvc/release')
  fs.mkdirSync(binaryDirectory, { recursive: true })
  fs.writeFileSync(path.join(binaryDirectory, 'apifuck-desktop.exe'), 'compiled program')
  const result = spawnSync(process.execPath, [fileURLToPath(new URL('./artifacts.mjs', import.meta.url)), 'stage', 'x86_64-pc-windows-msvc'], { cwd: root, encoding: 'utf8' })
  assert.equal(result.status, 0, result.stderr)
  const expected = 'ApiFuck-4.5.6.exe'
  assert.deepEqual(fs.readdirSync(path.join(root, 'release-assets')).sort(), [expected, `${expected}.sha256`])
  assert.equal(fs.readFileSync(path.join(root, 'release-assets', `${expected}.sha256`), 'utf8'), `${digest(path.join(root, 'release-assets', expected))}  ${expected}\n`)
})

test('release cannot publish with missing, corrupt or incomplete platform assets', t => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'apifuck-assets-'))
  t.after(() => fs.rmSync(root, { recursive: true, force: true }))
  assert.throws(() => verifyArtifacts(root, '1.2.3'))
  for (const name of releaseAssetNames('1.2.3')) {
    const file = path.join(root, name)
    fs.writeFileSync(file, `executable-${name}`)
    fs.writeFileSync(`${file}.sha256`, `${digest(file)}  ${name}\n`)
  }
  const assets = verifyArtifacts(root, '1.2.3')
  assert.throws(() => verifyArtifacts(root, '1.2.4'))
  assert.deepEqual(releaseAssetNames('1.2.3'), ['ApiFuck-1.2.3-arm64.dmg', 'ApiFuck-1.2.3.exe', 'ApiFuck-1.2.3.AppImage'])
  for (const name of [macosDmg('1.2.3'), linuxAppImage('1.2.3')]) {
    const file = path.join(root, name)
    const bytes = fs.readFileSync(file)
    fs.rmSync(file)
    assert.throws(() => verifyArtifacts(root, '1.2.3'))
    fs.writeFileSync(file, bytes)
    fs.appendFileSync(file, 'corrupt')
    assert.throws(() => verifyArtifacts(root, '1.2.3'), /Checksum mismatch/)
    fs.writeFileSync(file, bytes)
  }
  assert.equal(fs.readFileSync(path.join(root, 'SHA256SUMS'), 'utf8').trim().split('\n').length, releaseAssetNames('1.2.3').length)
  const release = { assets: assets.map(a => ({ name: a.name, size: a.size, state: 'uploaded', digest: `sha256:${a.sha256}` })) }
  const sums = path.join(root, 'SHA256SUMS')
  release.assets.push({ name: 'SHA256SUMS', size: fs.statSync(sums).size, state: 'uploaded', digest: `sha256:${digest(sums)}` })
  verifyUploadedAssets(release, assets)
  release.assets.push({ name: 'obsolete-macos.zip' })
  assert.throws(() => verifyUploadedAssets(release, assets), /Unexpected/)
  release.assets.pop()
  release.assets[0].digest = null
  assert.throws(() => verifyUploadedAssets(release, assets))
  fs.appendFileSync(path.join(root, assets[0].name), 'tampered')
  assert.throws(() => verifyArtifacts(root, '1.2.3'), /Checksum mismatch/)
})
