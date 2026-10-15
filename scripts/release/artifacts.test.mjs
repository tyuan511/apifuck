import assert from 'node:assert/strict'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { test } from 'node:test'
import { digest, releaseAssetNames, verifyArtifacts, verifyUploadedAssets } from './artifacts.mjs'

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
  const archive = path.join(root, 'apifuck-desktop-aarch64-apple-darwin.zip')
  assert.ok(assets.some(asset => asset.name === path.basename(archive)))
  const archiveBytes = fs.readFileSync(archive)
  fs.rmSync(archive)
  assert.throws(() => verifyArtifacts(root))
  fs.writeFileSync(archive, archiveBytes)
  assert.equal(fs.readFileSync(path.join(root, 'SHA256SUMS'), 'utf8').trim().split('\n').length, releaseAssetNames.length)
  const release = { assets: assets.map(a => ({ name: a.name, size: a.size, state: 'uploaded', digest: `sha256:${a.sha256}` })) }
  verifyUploadedAssets(release, assets)
  release.assets[0].digest = null
  assert.throws(() => verifyUploadedAssets(release, assets))
  fs.appendFileSync(path.join(root, assets[0].name), 'tampered')
  assert.throws(() => verifyArtifacts(root), /Checksum mismatch/)
})
