import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import os from 'node:os'
import { test } from 'node:test'
import { assetName, digest, macosArchive } from './artifacts.mjs'
import { cleanup, developerIdentity, signAndNotarize, validateCredentials } from './macos-sign.mjs'

function fixture(t, outcome = 'Accepted') {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'apifuck-sign-test-'))
  t.after(() => fs.rmSync(root, { recursive: true, force: true }))
  fs.mkdirSync(path.join(root, 'release-assets'))
  const file = path.join(root, 'release-assets', assetName('aarch64-apple-darwin'))
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
    if (command === 'codesign' && args.includes('--sign')) fs.appendFileSync(file, ':signed')
    if (command === 'ditto') fs.writeFileSync(args.at(-1), `archive:${fs.readFileSync(file)}`)
    if (command === 'xcrun' && args[1] === 'submit') return JSON.stringify({ id: 'test-request', status: outcome })
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

test('accepted notarization hashes signed bytes and archive, then restores the keychain', t => {
  t.mock.method(console, 'log', () => {})
  const input = fixture(t)
  signAndNotarize(input)
  const archive = path.join(input.root, 'release-assets', macosArchive)
  for (const file of [input.file, archive]) assert.equal(fs.readFileSync(`${file}.sha256`, 'utf8'), `${digest(file)}  ${path.basename(file)}\n`)
  assert.ok(fs.readFileSync(archive, 'utf8').endsWith(':signed'))
  const signing = input.calls.find(args => args[0] === 'codesign' && args.includes('--sign'))
  assert.ok(signing.includes('--timestamp'))
  assert.ok(signing.includes('runtime'))
  assert.deepEqual(input.calls.at(-2), ['security', 'list-keychains', '-d', 'user', '-s', '/test/login.keychain-db', '/test/other.keychain-db'])
  assert.equal(input.calls.at(-1)[1], 'delete-keychain')
  assert.equal(fs.existsSync(path.join(input.root, 'apifuck-macos-signing')), false)
  cleanup(input.env, input.run) // Always-run CI cleanup is idempotent.
})

for (const outcome of ['Invalid', 'In Progress', 'import-failed']) {
  test(`${outcome} blocks publication and removes private credentials`, t => {
    const input = fixture(t, outcome)
    assert.throws(() => signAndNotarize(input))
    assert.equal(fs.existsSync(`${input.file}.sha256`), false)
    assert.equal(fs.existsSync(path.join(input.root, 'release-assets', `${macosArchive}.sha256`)), false)
    assert.equal(fs.existsSync(path.join(input.root, 'apifuck-macos-signing')), false)
    if (outcome !== 'import-failed') assert.ok(fs.existsSync(path.join(input.root, 'apifuck-notarization/log.json')))
  })
}

test('only one Developer ID Application identity can sign the release', () => {
  const identity = `1) ${'A'.repeat(40)} "Developer ID Application: Test (TEAM)"`
  assert.equal(developerIdentity(identity), 'A'.repeat(40))
  assert.throws(() => developerIdentity(identity.replace('Developer ID Application', 'Apple Development')))
  assert.throws(() => developerIdentity(`${identity}\n${identity}`))
})
