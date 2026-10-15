import fs from 'node:fs'
import path from 'node:path'
import { randomBytes } from 'node:crypto'
import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { assetName, digest, macosArchive } from './artifacts.mjs'

const requiredSecrets = ['APPLE_CERT_P12', 'APPLE_CERT_PASSWORD', 'APPLE_API_KEY_P8', 'APPLE_API_KEY_ID', 'APPLE_API_ISSUER']

export function validateCredentials(env) {
  for (const name of requiredSecrets) {
    if (!env[name]?.trim()) throw new Error(`Missing GitHub Secret: ${name}`)
  }
  if (!env.RUNNER_TEMP) throw new Error('RUNNER_TEMP is required')
  if (!/^[A-Za-z0-9]+$/.test(env.APPLE_API_KEY_ID)) throw new Error('Invalid APPLE_API_KEY_ID')
  if (!/^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/i.test(env.APPLE_API_ISSUER)) throw new Error('Invalid APPLE_API_ISSUER')
  if (!env.APPLE_API_KEY_P8.includes('-----BEGIN PRIVATE KEY-----')) throw new Error('APPLE_API_KEY_P8 must contain the original PEM text')
}

function runTool(command, args) {
  const result = spawnSync(command, args, { encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 })
  // Some notarytool failures still return the request ID and status as JSON.
  // Preserve that response so rejection/timeout logs can be downloaded.
  if (!result.error && result.status !== 0 && command === 'xcrun' && args[0] === 'notarytool' && args[1] === 'submit') {
    try {
      const response = JSON.parse(result.stdout)
      if (response.id && response.status !== 'Accepted') return result.stdout.trim()
    } catch { /* An unstructured tool failure is handled below. */ }
  }
  // Never include argv: security commands carry certificate/keychain passwords.
  if (result.error || result.status !== 0) throw new Error(`${path.basename(command)} ${args[0]} failed (${result.status ?? 'spawn error'}). Check runner or notarization diagnostics.`)
  return result.stdout.trim()
}

export function developerIdentity(output) {
  const identities = [...output.matchAll(/\b([A-Fa-f0-9]{40}) "Developer ID Application:[^"\n]+"/g)]
  if (identities.length !== 1) throw new Error('The P12 must contain exactly one valid Developer ID Application signing identity and its private key')
  return identities[0][1]
}

function locations(env) {
  if (!env.RUNNER_TEMP) throw new Error('RUNNER_TEMP is required')
  const directory = path.join(env.RUNNER_TEMP, 'apifuck-macos-signing')
  return { directory, keychain: path.join(directory, 'signing.keychain-db'), savedList: path.join(directory, 'keychains.json') }
}

export function cleanup(env = process.env, run = runTool) {
  const { directory, keychain, savedList } = locations(env)
  if (!fs.existsSync(directory)) return
  const errors = []
  if (fs.existsSync(savedList)) {
    try { run('security', ['list-keychains', '-d', 'user', '-s', ...JSON.parse(fs.readFileSync(savedList, 'utf8'))]) }
    catch (error) { errors.push(error) }
  }
  if (fs.existsSync(keychain)) {
    try { run('security', ['delete-keychain', keychain]) }
    catch (error) { errors.push(error) }
  }
  fs.rmSync(directory, { recursive: true, force: true })
  if (errors.length) throw new Error('Failed to restore or remove the temporary signing keychain')
}

export function signAndNotarize({ env = process.env, run = runTool, root = process.cwd(), platform = process.platform } = {}) {
  validateCredentials(env)
  if (platform !== 'darwin') throw new Error('Signing and notarization require a macOS runner')
  const { directory, keychain, savedList } = locations(env)
  const file = path.join(root, 'release-assets', assetName('aarch64-apple-darwin'))
  if (!fs.statSync(file).isFile()) throw new Error('The staged macOS binary is missing')
  // Refuse to reuse another invocation's keychain, including on self-hosted runners.
  fs.mkdirSync(directory, { mode: 0o700 })
  const archive = path.join(root, 'release-assets', macosArchive)
  const diagnostics = path.join(env.RUNNER_TEMP, 'apifuck-notarization')
  fs.mkdirSync(diagnostics, { recursive: true })
  const password = randomBytes(32).toString('hex')
  const certificate = path.join(directory, 'certificate.p12')
  const apiKey = path.join(directory, 'AuthKey.p8')
  const credentials = ['--key', apiKey, '--key-id', env.APPLE_API_KEY_ID, '--issuer', env.APPLE_API_ISSUER]
  // Hashes must be regenerated after signing changes the binary's bytes.
  fs.rmSync(`${file}.sha256`, { force: true })
  fs.rmSync(`${archive}.sha256`, { force: true })
  fs.rmSync(archive, { force: true })
  try {
    fs.writeFileSync(certificate, Buffer.from(env.APPLE_CERT_P12, 'base64'), { mode: 0o600 })
    fs.writeFileSync(apiKey, env.APPLE_API_KEY_P8, { mode: 0o600 })
    const original = [...run('security', ['list-keychains', '-d', 'user']).matchAll(/"([^"\n]+)"/g)].map(match => match[1])
    if (!original.length) throw new Error('Cannot determine the original keychain search list')
    fs.writeFileSync(savedList, JSON.stringify(original), { mode: 0o600 })
    run('security', ['create-keychain', '-p', password, keychain])
    run('security', ['set-keychain-settings', '-lut', '21600', keychain])
    run('security', ['unlock-keychain', '-p', password, keychain])
    run('security', ['import', certificate, '-k', keychain, '-P', env.APPLE_CERT_PASSWORD, '-T', '/usr/bin/codesign'])
    run('security', ['set-key-partition-list', '-S', 'apple-tool:,apple:,codesign:', '-s', '-k', password, keychain])
    run('security', ['list-keychains', '-d', 'user', '-s', keychain, ...original])
    const identity = developerIdentity(run('security', ['find-identity', '-v', '-p', 'codesigning', keychain]))
    fs.chmodSync(file, 0o755)
    run('codesign', ['--force', '--sign', identity, '--keychain', keychain, '--options', 'runtime', '--timestamp', '--identifier', 'dev.fastvibe.apifuck', file])
    run('codesign', ['--verify', '--strict', '--verbose=2', file])
    run('ditto', ['-c', '-k', '--keepParent', file, archive])
    const response = run('xcrun', ['notarytool', 'submit', archive, ...credentials, '--wait', '--timeout', '30m', '--output-format', 'json', '--no-progress'])
    fs.writeFileSync(path.join(diagnostics, 'result.json'), response)
    const result = JSON.parse(response)
    if (result.status !== 'Accepted') {
      if (typeof result.id === 'string') {
        try { run('xcrun', ['notarytool', 'log', result.id, ...credentials, path.join(diagnostics, 'log.json')]) }
        catch { /* The original rejection remains the release-blocking error. */ }
      }
      throw new Error(`Apple notarization was not Accepted (${result.status ?? 'unknown'}); see notarization diagnostics`)
    }
    run('codesign', ['--verify', '--strict', '--verbose=2', file])
    for (const asset of [file, archive]) fs.writeFileSync(`${asset}.sha256`, `${digest(asset)}  ${path.basename(asset)}\n`)
    console.log('macOS Developer ID signature verified; Apple notarization Accepted')
  } finally {
    cleanup(env, run)
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    if (process.argv[2] === 'cleanup') cleanup()
    else if (process.argv[2] === 'check') validateCredentials(process.env)
    else if (!process.argv[2]) signAndNotarize()
    else throw new Error('Usage: macos-sign.mjs [check|cleanup]')
  } catch (error) {
    console.error(error.message)
    process.exitCode = 1
  }
}
