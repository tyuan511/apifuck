import fs from 'node:fs'
import path from 'node:path'
import { execFileSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { parseVersion, workspaceVersion } from './version.mjs'

const root = fileURLToPath(new URL('../../', import.meta.url))
try {
  const tag = process.argv[2]
  if (!tag?.startsWith('v')) throw new Error('Expected a v<semver> release tag')
  const version = tag.slice(1)
  const parsed = parseVersion(version)
  if (version !== workspaceVersion(root)) throw new Error('Tag/version mismatch')
  const notes = path.join(root, 'docs/release', `${version}.md`)
  if (!fs.existsSync(notes) || !fs.readFileSync(notes, 'utf8').trim()) throw new Error(`Missing release notes: ${notes}`)
  const metadata = JSON.parse(execFileSync('cargo', ['metadata', '--locked', '--no-deps', '--format-version', '1'], { cwd: root, encoding: 'utf8' }))
  for (const pkg of metadata.packages.filter(pkg => metadata.workspace_members.includes(pkg.id))) {
    if (pkg.version !== version) throw new Error(`Version mismatch in ${pkg.name}`)
  }
  if (process.env.GITHUB_OUTPUT) fs.appendFileSync(process.env.GITHUB_OUTPUT, `prerelease=${parsed.prerelease.length > 0}\n`)
  console.log(`Validated ${tag} and docs/release/${version}.md`)
} catch (error) {
  console.error(error.message)
  process.exitCode = 1
}
