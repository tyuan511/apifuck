import fs from 'node:fs'
import path from 'node:path'
import { execFileSync, spawnSync } from 'node:child_process'
import { compareVersions, parseVersion } from './version.mjs'
import { verifyArtifacts, verifyUploadedAssets } from './artifacts.mjs'

function gh(args) {
  return execFileSync('gh', args, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'inherit'] }).trim()
}

try {
  const [tag, directory] = process.argv.slice(2)
  const repo = process.env.GITHUB_REPOSITORY
  if (!/^[\w.-]+\/[\w.-]+$/.test(repo ?? '')) throw new Error('GITHUB_REPOSITORY is required')
  if (!tag?.startsWith('v') || !directory) throw new Error('Usage: publish.mjs v<semver> <artifacts-directory>')
  const prerelease = parseVersion(tag.slice(1)).prerelease.length > 0
  const notes = `docs/release/${tag.slice(1)}.md`
  if (!fs.readFileSync(notes, 'utf8').trim()) throw new Error('Release notes are empty')
  const assets = verifyArtifacts(directory, tag.slice(1))
  // A failed lookup must never be interpreted as permission to edit a public release.
  const lookup = spawnSync('gh', ['release', 'view', tag, '--json', 'isDraft'], { encoding: 'utf8' })
  if (lookup.status !== 0) {
    gh(['release', 'create', tag, '--title', tag, '--notes-file', notes, '--draft', '--verify-tag', ...(prerelease ? ['--prerelease'] : [])])
  }
  const existing = JSON.parse(gh(['release', 'view', tag, '--json', 'isDraft']))
  if (!existing.isDraft) throw new Error(`${tag} is already public; refusing to replace it`)
  gh(['release', 'edit', tag, '--title', tag, '--notes-file', notes, `--prerelease=${prerelease}`])
  gh(['release', 'upload', tag, ...assets.map(a => path.join(directory, a.name)), path.join(directory, 'SHA256SUMS'), '--clobber'])
  // Authenticate the uploaded bytes through the same GitHub metadata used by the app.
  const releases = JSON.parse(gh(['api', '--paginate', '--slurp', `repos/${repo}/releases?per_page=100`])).flat()
  const release = releases.find(item => item.tag_name === tag)
  if (!release?.draft) throw new Error('Expected a draft release before publication')
  verifyUploadedAssets(release, assets)
  const newerStable = releases.some(item => {
    if (item.draft || item.prerelease || !item.tag_name.startsWith('v')) return false
    try { return compareVersions(item.tag_name.slice(1), tag.slice(1)) > 0 }
    catch { return false }
  })
  gh(['release', 'edit', tag, '--draft=false', `--latest=${!prerelease && !newerStable}`])
  console.log(`Published ${tag} with verified assets`)
} catch (error) {
  console.error(error.message)
  process.exitCode = 1
}
