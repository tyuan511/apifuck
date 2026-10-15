import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { test } from 'node:test'
import { fileURLToPath } from 'node:url'

const repository = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..')

function run(program, args, cwd) {
  return spawnSync(program, args, { cwd, encoding: 'utf8', timeout: 30000 })
}

function fixture(t) {
  const root = mkdtempSync(path.join(tmpdir(), 'apifuck-release-test-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  mkdirSync(path.join(root, 'scripts/release'), { recursive: true })
  return root
}

function workspace(t) {
  const root = fixture(t)
  copyFileSync(path.join(repository, 'scripts/release/sync-version.mjs'), path.join(root, 'scripts/release/sync-version.mjs'))
  copyFileSync(path.join(repository, 'scripts/release/version.mjs'), path.join(root, 'scripts/release/version.mjs'))
  copyFileSync(path.join(repository, 'scripts/release/validate-release.mjs'), path.join(root, 'scripts/release/validate-release.mjs'))
  copyFileSync(path.join(repository, 'rust-toolchain.toml'), path.join(root, 'rust-toolchain.toml'))
  writeFileSync(path.join(root, 'Cargo.toml'), '[workspace]\nmembers = ["crates/core", "crates/desktop"]\nresolver = "2"\n\n[workspace.package]\nversion = "0.1.0"\nedition = "2024"\n')
  for (const name of ['core', 'desktop']) {
    const dir = path.join(root, 'crates', name)
    mkdirSync(path.join(dir, 'src'), { recursive: true })
    writeFileSync(path.join(dir, 'Cargo.toml'), `[package]\nname = "fixture-${name}"\nversion.workspace = true\nedition.workspace = true\n`)
    writeFileSync(path.join(dir, 'src/lib.rs'), '')
  }
  const initial = run('cargo', ['metadata', '--format-version', '1', '--offline'], root)
  assert.equal(initial.status, 0, initial.stderr)
  return root
}

test('version sync updates inherited package versions and the committed lockfile', (t) => {
  const root = workspace(t)
  const result = run(process.execPath, ['scripts/release/sync-version.mjs', '1.2.3'], root)
  assert.equal(result.status, 0, result.stderr)
  const metadata = run('cargo', ['metadata', '--locked', '--offline', '--format-version', '1', '--no-deps'], root)
  assert.equal(metadata.status, 0, metadata.stderr)
  assert.deepEqual(JSON.parse(metadata.stdout).packages.map(pkg => pkg.version), ['1.2.3', '1.2.3'])
  const lock = readFileSync(path.join(root, 'Cargo.lock'), 'utf8')
  assert.match(lock, /name = "fixture-core"\nversion = "1\.2\.3"/)
  assert.match(lock, /name = "fixture-desktop"\nversion = "1\.2\.3"/)
})

test('missing required release notes leave version and lockfile untouched', (t) => {
  const root = workspace(t)
  const before = ['Cargo.toml', 'Cargo.lock'].map(name => readFileSync(path.join(root, name), 'utf8'))
  const result = run(process.execPath, ['scripts/release/sync-version.mjs', '1.2.3', '--verify-release-notes'], root)
  assert.notEqual(result.status, 0)
  assert.match(result.stderr, /Missing release notes/)
  assert.deepEqual(['Cargo.toml', 'Cargo.lock'].map(name => readFileSync(path.join(root, name), 'utf8')), before)
})

test('Cargo version validation failure restores the original workspace and lockfile', (t) => {
  const root = workspace(t)
  const before = ['Cargo.toml', 'Cargo.lock'].map(name => readFileSync(path.join(root, name), 'utf8'))
  const result = run(process.execPath, ['scripts/release/sync-version.mjs', '1.2.3-01'], root)
  assert.notEqual(result.status, 0)
  assert.deepEqual(['Cargo.toml', 'Cargo.lock'].map(name => readFileSync(path.join(root, name), 'utf8')), before)
})

test('release gate requires matching versions and non-empty notes', (t) => {
  const root = workspace(t)
  const check = tag => run(process.execPath, ['scripts/release/validate-release.mjs', tag], root)
  assert.notEqual(check('v1.0.0').status, 0)
  assert.notEqual(check('v0.1.0').status, 0)
  mkdirSync(path.join(root, 'docs/release'), { recursive: true })
  writeFileSync(path.join(root, 'docs/release/0.1.0.md'), '   \n')
  assert.notEqual(check('v0.1.0').status, 0)
  writeFileSync(path.join(root, 'docs/release/0.1.0.md'), '# 0.1.0\n\nInitial native release.\n')
  const result = check('v0.1.0')
  assert.equal(result.status, 0, result.stderr)
})

test('download sync selects desktop binaries over unrelated assets and old installers', (t) => {
  const root = fixture(t)
  writeFileSync(path.join(root, 'README.md'), 'Before\n<!-- release-downloads:start -->\nOld\n<!-- release-downloads:end -->\nAfter\n')
  const names = [
    'ApiFuck_0.0.8_aarch64.dmg', 'ApiFuck_0.0.8_x64-setup.exe', 'ApiFuck_0.0.8_amd64.AppImage',
    'unrelated-tool-aarch64-apple-darwin', 'unrelated-tool-x86_64-pc-windows-msvc.exe', 'unrelated-tool-x86_64-unknown-linux-gnu',
    'apifuck-desktop-aarch64-apple-darwin.zip', 'apifuck-desktop-x86_64-pc-windows-msvc.exe', 'apifuck-desktop-x86_64-unknown-linux-gnu',
  ]
  writeFileSync(path.join(root, 'mock-fetch.mjs'), `globalThis.fetch = async () => new Response(JSON.stringify(${JSON.stringify({ tag_name: 'v1.2.3', assets: names.map(name => ({ name })) })}), {status: 200});\n`)
  const result = run(process.execPath, ['--import', './mock-fetch.mjs', path.join(repository, 'scripts/release/update-readme-downloads.mjs'), 'v1.2.3', '--repo', 'example/apifuck'], root)
  assert.equal(result.status, 0, result.stderr)
  const readme = readFileSync(path.join(root, 'README.md'), 'utf8')
  for (const name of names.slice(-3)) {
    assert.ok(readme.includes(`https://github.com/example/apifuck/releases/download/v1.2.3/${name}`))
  }
  assert.doesNotMatch(readme, /unrelated-tool|\.dmg|setup\.exe|AppImage/)
  assert.ok(readme.startsWith('Before\n'))
  assert.ok(readme.endsWith('\nAfter\n'))
})
