import assert from 'node:assert/strict'
import { test } from 'node:test'
import { compareVersions, nextVersion, parseVersion } from './version.mjs'

test('version generation respects unreleased workspace versions and remote history', () => {
  assert.equal(nextVersion('0.1.0', ['v0.0.8']), '0.1.0')
  assert.equal(nextVersion('0.1.0', ['v0.1.0']), '0.1.1')
  assert.equal(nextVersion('0.1.0', ['v0.2.0']), '0.2.1')
  assert.equal(nextVersion('0.1.0', [], 'minor'), '0.2.0')
  assert.equal(nextVersion('0.1.0', [], 'major'), '1.0.0')
  assert.equal(nextVersion('0.1.0-rc.1', []), '0.1.0')
  assert.equal(nextVersion('0.1.0', [], 'v0.2.0-beta.1'), '0.2.0-beta.1')
  assert.throws(() => nextVersion('0.1.0', ['v0.1.1'], '0.1.0'))
})

test('semantic precedence and prerelease validation are strict', () => {
  const ordered = ['1.0.0-alpha', '1.0.0-alpha.1', '1.0.0-alpha.beta', '1.0.0-beta', '1.0.0-beta.2', '1.0.0-beta.11', '1.0.0-rc.1', '1.0.0', '1.0.1']
  assert.deepEqual([...ordered].reverse().sort(compareVersions), ordered)
  assert.equal(compareVersions('1.0.0+first', '1.0.0+second'), 0)
  for (const invalid of ['01.2.3', '1.2.3-01', '1.2.3-a..b', '1.2', '../1.2.3', 'v1.2.3']) assert.throws(() => parseVersion(invalid))
})
