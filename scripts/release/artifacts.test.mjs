import assert from 'node:assert/strict'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { test } from 'node:test'
import { digest, releaseAssetNames, macosDmg, linuxAppImage, verifyArtifacts, verifyUploadedAssets } from './artifacts.mjs'

test('release cannot publish with missing, corrupt or incomplete platform assets', t => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'apifuck-assets-'))
  t.after(() => fs.rmSync(root, { recursive: true, force: true }))
  assert.throws(() => verifyArtifacts(root))
  for (const name of releaseAssetNames) {
    const file = path.join(root, name)
    fs.writeFileSync(file, `executable-${name}`)
    fs.writeFileSync(`${file}.sha256`, `${digest(file)}  ${name}\n`)
  }
  const assets = verifyArtifacts(root)
  assert.deepEqual(releaseAssetNames, [macosDmg, 'apifuck-desktop-x86_64-pc-windows-msvc.exe', linuxAppImage])
  for (const name of [macosDmg, linuxAppImage]) {
    const file = path.join(root, name)
    const bytes = fs.readFileSync(file)
    fs.rmSync(file)
    assert.throws(() => verifyArtifacts(root))
    fs.writeFileSync(file, bytes)
    fs.appendFileSync(file, 'corrupt')
    assert.throws(() => verifyArtifacts(root), /Checksum mismatch/)
    fs.writeFileSync(file, bytes)
  }
  assert.equal(fs.readFileSync(path.join(root, 'SHA256SUMS'), 'utf8').trim().split('\n').length, releaseAssetNames.length)
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
  assert.throws(() => verifyArtifacts(root), /Checksum mismatch/)
})
