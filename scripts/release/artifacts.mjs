import fs from 'node:fs'
import path from 'node:path'
import { createHash } from 'node:crypto'
import { fileURLToPath } from 'node:url'
import { parseVersion, workspaceVersion } from './version.mjs'

export const targets = ['aarch64-apple-darwin', 'x86_64-unknown-linux-gnu', 'x86_64-pc-windows-msvc']
export function assetName(target) {
  if (!targets.includes(target)) throw new Error(`Unsupported release target: ${target}`)
  return `apifuck-desktop-${target}${target.includes('windows') ? '.exe' : ''}`
}
// electron-builder defaults: non-default arm64 suffix on DMG, default x64 omitted.
// Windows is portable; use the GitHub-safe form of "ApiFuck <version>.exe".
export function releaseAssetName(target, version) {
  parseVersion(version)
  switch (target) {
    case 'aarch64-apple-darwin': return `ApiFuck-${version}-arm64.dmg`
    case 'x86_64-pc-windows-msvc': return `ApiFuck-${version}.exe`
    case 'x86_64-unknown-linux-gnu': return `ApiFuck-${version}.AppImage`
    default: throw new Error(`Unsupported release target: ${target}`)
  }
}
export const macosDmg = version => releaseAssetName('aarch64-apple-darwin', version)
export const linuxAppImage = version => releaseAssetName('x86_64-unknown-linux-gnu', version)
export const releaseAssetNames = version => [macosDmg(version), releaseAssetName('x86_64-pc-windows-msvc', version), linuxAppImage(version)]
export function digest(file) {
  return createHash('sha256').update(fs.readFileSync(file)).digest('hex')
}
export function verifyArtifacts(directory, version) {
  const assets = releaseAssetNames(version).map(name => {
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
  const sums = Buffer.from(assets.map(asset => `${asset.sha256}  ${asset.name}\n`).join(''))
  const expectedAssets = [...assets, { name: 'SHA256SUMS', size: sums.length, sha256: createHash('sha256').update(sums).digest('hex') }]
  if (release.assets?.length !== expectedAssets.length) throw new Error('Unexpected or missing release assets')
  for (const expected of expectedAssets) {
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
      const name = argument.includes('windows') ? releaseAssetName(argument, workspaceVersion(process.cwd())) : assetName(argument)
      const source = `target/${argument}/release/apifuck-desktop${argument.includes('windows') ? '.exe' : ''}`
      fs.mkdirSync('release-assets', { recursive: true })
      const directory = argument.includes('windows') ? 'release-assets' : 'target/release-staging'
      fs.mkdirSync(directory, { recursive: true })
      const file = path.join(directory, name)
      fs.copyFileSync(source, file)
      fs.writeFileSync(`${file}.sha256`, `${digest(file)}  ${name}\n`)
    } else if (mode === 'verify') {
      verifyArtifacts(argument, workspaceVersion(process.cwd()))
    } else {
      throw new Error('Usage: artifacts.mjs stage <target> | verify <directory>')
    }
  } catch (error) {
    console.error(error.message)
    process.exitCode = 1
  }
}
