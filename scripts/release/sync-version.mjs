import fs from 'node:fs'
import path from 'node:path'
import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { parseVersion } from './version.mjs'

function fail(message) {
  console.error(message)
  process.exit(1)
}

const version = process.argv[2]
try {
  parseVersion(version)
} catch {
  fail('Usage: node scripts/release/sync-version.mjs <semver> [--verify-release-notes]')
}

const rootDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..')
const cargoTomlPath = path.join(rootDir, 'Cargo.toml')
const cargoLockPath = path.join(rootDir, 'Cargo.lock')
const notesPath = path.join(rootDir, 'docs', 'release', `${version}.md`)
if (process.argv.includes('--verify-release-notes') && (!fs.existsSync(notesPath) || !fs.readFileSync(notesPath, 'utf8').trim())) {
  fail(`Missing release notes file: docs/release/${version}.md`)
}

const before = fs.readFileSync(cargoTomlPath, 'utf8')
const lockBefore = fs.existsSync(cargoLockPath) ? fs.readFileSync(cargoLockPath) : null
const versionField = /(\[workspace\.package\][\s\S]*?^version\s*=\s*)"[^"]+"/m
if (!versionField.test(before)) {
  fail('Missing workspace.package.version in Cargo.toml')
}
fs.writeFileSync(cargoTomlPath, before.replace(versionField, `$1"${version}"`))

// Cargo updates every inherited workspace package version in the lockfile.
const result = spawnSync('cargo', ['metadata', '--format-version', '1', '--offline'], {
  cwd: rootDir,
  encoding: 'utf8',
  stdio: ['ignore', 'ignore', 'pipe'],
})
if (result.error || result.status !== 0) {
  fs.writeFileSync(cargoTomlPath, before)
  if (lockBefore) fs.writeFileSync(cargoLockPath, lockBefore)
  else fs.rmSync(cargoLockPath, { force: true })
  fail(result.error?.message || result.stderr || 'Failed to update Cargo.lock')
}
console.log(`Synced workspace version to ${version}`)
