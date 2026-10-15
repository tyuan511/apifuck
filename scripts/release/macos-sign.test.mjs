import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import os from 'node:os'
import { test } from 'node:test'
import { assetName, digest, macosDmg } from './artifacts.mjs'
import { cleanup, developerIdentity, signAndNotarize, validateCredentials } from './macos-sign.mjs'

function fixture(t, outcome = 'Accepted') {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'apifuck-sign-test-'))
  t.after(() => fs.rmSync(root, { recursive: true, force: true }))
  fs.mkdirSync(path.join(root, 'release-assets'))
  fs.mkdirSync(path.join(root, 'assets'))
  fs.writeFileSync(path.join(root, 'assets/apifuck.png'), 'icon')
  fs.writeFileSync(path.join(root, 'Cargo.toml'), '[workspace.package]\nversion = "1.2.3"\n')
  fs.mkdirSync(path.join(root, 'target/release-staging'), { recursive: true })
  const file = path.join(root, 'target/release-staging', assetName('aarch64-apple-darwin'))
  fs.writeFileSync(file, 'unsigned executable')
  fs.writeFileSync(`${file}.sha256`, 'old unsigned hash')
  const env = {
    RUNNER_TEMP: root,
    APPLE_CERT_P12: Buffer.from('test certificate').toString('base64'),
    APPLE_CERT_PASSWORD: 'test-only-password',
    APPLE_API_KEY_P8: '-----BEGIN PRIVATE KEY-----\ntest-only\n-----END PRIVATE KEY-----',
    APPLE_API_KEY_ID: 'TEST12345',
    APPLE_API_ISSUER: '12345678-1234-1234-1234-123456789abc',
  }
  const calls = []
  const run = (command, args) => {
    calls.push([command, ...args])
    if (command === 'security' && args[0] === 'list-keychains' && args.length === 3) return '    "/test/login.keychain-db"\n    "/test/other.keychain-db"'
    if (command === 'security' && args[0] === 'create-keychain') fs.writeFileSync(args.at(-1), '')
    if (command === 'security' && args[0] === 'import' && outcome === 'import-failed') throw new Error('import failed')
    if (command === 'security' && args[0] === 'find-identity') return `1) ${'A'.repeat(40)} "Developer ID Application: Test (TEST123456)"\n1 valid identities found`
    if (command === 'xcrun' && args[0] === 'vtool') return 'platform MACOS\nminos 11.0'
    if (command === 'sips' || command === 'iconutil') fs.writeFileSync(args.at(-1), 'icon')
    if (command === 'codesign' && args.includes('--sign')) {
      const target = args.at(-1)
      fs.appendFileSync(fs.statSync(target).isDirectory() ? path.join(target, 'Contents/MacOS/apifuck-desktop') : target, ':signed')
    }
    if (command === 'ditto') {
      if (args[0] === '-c') fs.writeFileSync(args.at(-1), `archive:${fs.readFileSync(file)}`)
      else fs.cpSync(args[0], args[1], { recursive: true })
    }
    if (command === 'env' && args[1] === 'tar') fs.writeFileSync(args[4], 'bundle archive after stapling')
    if (command === 'hdiutil' && args[0] === 'create') {
      assert.equal(fs.readlinkSync(path.join(args[args.indexOf('-srcfolder') + 1], 'Applications')), '/Applications')
      fs.writeFileSync(args.at(-1), 'disk image')
    }
    if (command === 'xcrun' && args[0] === 'stapler') {
      if (outcome === 'staple-failed') throw new Error('stapling failed')
      if (args[1] === 'staple' && args.at(-1).endsWith('.dmg')) fs.appendFileSync(args.at(-1), ':stapled')
    }
    if (command === 'xcrun' && args[1] === 'submit') return JSON.stringify({ id: 'test-request', status: outcome === 'dmg-invalid' ? (args[2].endsWith('.dmg') ? 'Invalid' : 'Accepted') : outcome === 'staple-failed' ? 'Accepted' : outcome })
    if (command === 'xcrun' && args[1] === 'log') fs.writeFileSync(args.at(-1), '{"status":"Invalid"}')
    return ''
  }
  return { root, file, env, calls, run, platform: 'darwin' }
}

test('missing Apple credentials fail before changing files or keychains', t => {
  const input = fixture(t)
  for (const name of Object.keys(input.env).filter(name => name.startsWith('APPLE_'))) {
    assert.throws(() => signAndNotarize({ ...input, env: { ...input.env, [name]: '' } }), new RegExp(name))
  }
  assert.equal(input.calls.length, 0)
  assert.equal(fs.readFileSync(input.file, 'utf8'), 'unsigned executable')
  assert.throws(() => validateCredentials({ ...input.env, APPLE_API_KEY_P8: '/path/to/key' }), /PEM/)
})

test('accepted notarization hashes signed and stapled app archives and DMG, then restores the keychain', t => {
  t.mock.method(console, 'log', () => {})
  const input = fixture(t)
  signAndNotarize(input)
  for (const file of [path.join(input.root, 'release-assets', macosDmg('1.2.3'))]) assert.equal(fs.readFileSync(`${file}.sha256`, 'utf8'), `${digest(file)}  ${path.basename(file)}\n`)
  assert.deepEqual(fs.readdirSync(path.join(input.root, 'release-assets')).sort(), [macosDmg('1.2.3'), `${macosDmg('1.2.3')}.sha256`].sort())
  const signing = input.calls.find(args => args[0] === 'codesign' && args.includes('--sign'))
  assert.ok(signing.includes('--timestamp'))
  assert.ok(signing.includes('runtime'))
  const submissions = input.calls.filter(args => args[0] === 'xcrun' && args[2] === 'submit')
  assert.equal(submissions.length, 2)
  assert.ok(submissions[1][3].endsWith('.dmg'))
  const stapleApp = input.calls.findIndex(args => args[1] === 'stapler' && args[2] === 'staple' && args[3].endsWith('.app'))
  const dmg = input.calls.findIndex(args => args[0] === 'hdiutil' && args[1] === 'create')
  assert.ok(stapleApp > 0 && dmg > stapleApp)
  assert.ok(fs.readFileSync(path.join(input.root, 'release-assets', macosDmg('1.2.3')), 'utf8').endsWith(':stapled'))
  assert.deepEqual(input.calls.at(-2), ['security', 'list-keychains', '-d', 'user', '-s', '/test/login.keychain-db', '/test/other.keychain-db'])
  assert.equal(input.calls.at(-1)[1], 'delete-keychain')
  assert.equal(fs.existsSync(path.join(input.root, 'apifuck-macos-signing')), false)
  cleanup(input.env, input.run) // Always-run CI cleanup is idempotent.
})

for (const outcome of ['Invalid', 'In Progress', 'import-failed', 'dmg-invalid', 'staple-failed']) {
  test(`${outcome} blocks publication and removes private credentials`, t => {
    const input = fixture(t, outcome)
    assert.throws(() => signAndNotarize(input))
    for (const name of [macosDmg('1.2.3')]) assert.equal(fs.existsSync(path.join(input.root, 'release-assets', `${name}.sha256`)), false)
    assert.equal(fs.existsSync(path.join(input.root, 'apifuck-macos-signing')), false)
    if (!['import-failed', 'staple-failed'].includes(outcome)) assert.ok(fs.existsSync(path.join(input.root, `apifuck-notarization/${outcome === 'dmg-invalid' ? 'dmg' : 'app'}-log.json`)))
  })
}

test('only one Developer ID Application identity can sign the release', () => {
  const identity = `1) ${'A'.repeat(40)} "Developer ID Application: Test (TEAM)"`
  assert.equal(developerIdentity(identity), 'A'.repeat(40))
  assert.throws(() => developerIdentity(identity.replace('Developer ID Application', 'Apple Development')))
  assert.throws(() => developerIdentity(`${identity}\n${identity}`))
})
