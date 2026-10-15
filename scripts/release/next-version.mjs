import { execFileSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { nextVersion, workspaceVersion } from './version.mjs'

const root = fileURLToPath(new URL('../../', import.meta.url))
try {
  const tags = execFileSync('git', ['tag', '--list'], { cwd: root, encoding: 'utf8' }).trim().split('\n')
  console.log(nextVersion(workspaceVersion(root), tags, process.argv[2] ?? 'auto'))
} catch (error) {
  console.error(error.message)
  process.exitCode = 1
}
