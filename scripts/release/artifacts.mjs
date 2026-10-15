import fs from 'node:fs'
import path from 'node:path'
import { createHash } from 'node:crypto'
import { fileURLToPath } from 'node:url'

export const targets = ['aarch64-apple-darwin', 'x86_64-unknown-linux-gnu', 'x86_64-pc-windows-msvc']
export function assetName(target) {
  if (!targets.includes(target)) throw new Error(`Unsupported release target: ${target}`)
  return `apifuck-desktop-${target}${target.includes('windows') ? '.exe' : ''}`
}
export const macosArchive = `${assetName('aarch64-apple-darwin')}.zip`
export const releaseAssetNames = [...targets.map(assetName), macosArchive]
export function digest(file) {
  return createHash('sha256').update(fs.readFileSync(file)).digest('hex')
}
export function verifyArtifacts(directory) {
  const assets = releaseAssetNames.map(name => {
    const file = path.join(directory, name)
    const size = fs.statSync(file).size
    if (!size || size > 512 * 1024 * 1024) throw new Error(`Invalid asset size: ${name}`)
    const sha256 = digest(file)
    if (fs.readFileSync(`${file}.sha256`, 'utf8').trim() !== `${sha256}  ${name}`) throw new Error(`Checksum mismatch: ${name}`)
    return { name, sha256, size }
  })
  fs.writeFileSync(path.join(directory, 'SHA256SUMS'), assets.map(a => `${a.sha256}  ${a.name}\n`).join(''))
  return assets
}
export function verifyUploadedAssets(release, assets) {
  for (const expected of assets) {
    const matches = release.assets?.filter(asset => asset.name === expected.name) ?? []
    if (matches.length !== 1 || matches[0].state !== 'uploaded' || matches[0].size !== expected.size || matches[0].digest !== `sha256:${expected.sha256}`) {
      throw new Error(`Uploaded asset missing or invalid: ${expected.name}`)
    }
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const [mode, argument] = process.argv.slice(2)
    if (mode === 'stage') {
      const name = assetName(argument)
      const source = `target/${argument}/release/apifuck-desktop${argument.includes('windows') ? '.exe' : ''}`
      fs.mkdirSync('release-assets', { recursive: true })
      const file = path.join('release-assets', name)
      fs.copyFileSync(source, file)
      fs.writeFileSync(`${file}.sha256`, `${digest(file)}  ${name}\n`)
    } else if (mode === 'verify') {
      verifyArtifacts(argument)
    } else {
      throw new Error('Usage: artifacts.mjs stage <target> | verify <directory>')
    }
  } catch (error) {
    console.error(error.message)
    process.exitCode = 1
  }
}
