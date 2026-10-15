import fs from 'node:fs'

const identifier = '(?:0|[1-9][0-9]*|[0-9]*[A-Za-z-][0-9A-Za-z-]*)'
const semver = new RegExp(`^(0|[1-9][0-9]*)\\.(0|[1-9][0-9]*)\\.(0|[1-9][0-9]*)(?:-(${identifier}(?:\\.${identifier})*))?(?:\\+([0-9A-Za-z-]+(?:\\.[0-9A-Za-z-]+)*))?$`)

export function parseVersion(value) {
  const match = typeof value === 'string' && value.match(semver)
  if (!match) throw new Error(`Invalid semantic version: ${value}`)
  return { parts: match.slice(1, 4).map(BigInt), prerelease: match[4]?.split('.') ?? [] }
}

export function compareVersions(left, right) {
  const a = parseVersion(left), b = parseVersion(right)
  for (let i = 0; i < 3; i++) {
    if (a.parts[i] !== b.parts[i]) return a.parts[i] > b.parts[i] ? 1 : -1
  }
  if (!a.prerelease.length || !b.prerelease.length) {
    return Number(!a.prerelease.length) - Number(!b.prerelease.length)
  }
  for (let i = 0; i < Math.max(a.prerelease.length, b.prerelease.length); i++) {
    const x = a.prerelease[i], y = b.prerelease[i]
    if (x === y) continue
    if (x === undefined || y === undefined) return x === undefined ? -1 : 1
    const xn = /^\d+$/.test(x), yn = /^\d+$/.test(y)
    if (xn && yn) return BigInt(x) > BigInt(y) ? 1 : -1
    if (xn !== yn) return xn ? -1 : 1
    return x > y ? 1 : -1
  }
  return 0
}

export function nextVersion(current, tags, bump = 'auto') {
  parseVersion(current)
  const versions = tags.flatMap(tag => {
    if (!tag.startsWith('v')) return []
    try { parseVersion(tag.slice(1)); return [tag.slice(1)] }
    catch { return [] }
  })
  const latest = versions.sort(compareVersions).at(-1)
  if (!['auto', 'patch', 'minor', 'major'].includes(bump)) {
    const explicit = bump.replace(/^v/, '')
    parseVersion(explicit)
    if (latest && compareVersions(explicit, latest) <= 0) throw new Error(`Version must be newer than v${latest}`)
    return explicit
  }
  if (bump === 'auto' && !parseVersion(current).prerelease.length && (!latest || compareVersions(current, latest) > 0)) return current
  const base = latest && compareVersions(latest, current) > 0 ? latest : current
  const { parts, prerelease } = parseVersion(base)
  if (bump === 'auto' && prerelease.length) return parts.join('.')
  const index = { major: 0, minor: 1, patch: 2, auto: 2 }[bump]
  parts[index]++
  for (let i = index + 1; i < 3; i++) parts[i] = 0n
  return parts.join('.')
}

export function workspaceVersion(root) {
  const source = fs.readFileSync(`${root}/Cargo.toml`, 'utf8')
  const section = source.split(/^\[workspace\.package\]\s*$/m)[1]?.split(/^\[/m)[0]
  const version = section?.match(/^version\s*=\s*"([^"]+)"/m)?.[1]
  parseVersion(version)
  return version
}
