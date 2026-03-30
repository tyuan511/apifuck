import fs from 'node:fs'
import path from 'node:path'

function fail(message) {
  console.error(message)
  process.exit(1)
}

const version = process.argv[2]
const shouldVerifyReleaseNotes = process.argv.includes('--verify-release-notes')

if (!version) {
  fail('Usage: node scripts/release/sync-version.mjs <version> [--verify-release-notes]')
}

const rootDir = process.cwd()
const packageJsonPath = path.join(rootDir, 'package.json')
const cargoTomlPath = path.join(rootDir, 'src-tauri', 'Cargo.toml')
const tauriConfigPath = path.join(rootDir, 'src-tauri', 'tauri.conf.json')
const releaseNotesPath = path.join(rootDir, 'docs', 'release', `${version}.md`)

const packageJson = JSON.parse(fs.readFileSync(packageJsonPath, 'utf8'))
packageJson.version = version
fs.writeFileSync(packageJsonPath, `${JSON.stringify(packageJson, null, 2)}\n`)

const cargoToml = fs.readFileSync(cargoTomlPath, 'utf8').replace(
  /^version = "[^"]+"$/m,
  `version = "${version}"`,
)

if (!/^version = "[^"]+"$/m.test(cargoToml)) {
  fail('Failed to update version in src-tauri/Cargo.toml')
}

fs.writeFileSync(cargoTomlPath, cargoToml)

const tauriConfig = JSON.parse(fs.readFileSync(tauriConfigPath, 'utf8'))
tauriConfig.version = version
fs.writeFileSync(tauriConfigPath, `${JSON.stringify(tauriConfig, null, 2)}\n`)

if (shouldVerifyReleaseNotes && !fs.existsSync(releaseNotesPath)) {
  fail(`Missing release notes file: docs/release/${version}.md`)
}

console.log(`Synced release version to ${version}`)
