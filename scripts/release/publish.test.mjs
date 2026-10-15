import assert from 'node:assert/strict'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { test } from 'node:test'
import { digest, releaseAssetNames } from './artifacts.mjs'

const script = fileURLToPath(new URL('./publish.mjs', import.meta.url))
// A local fake gh exercises the publication boundary without contacting GitHub.
for (const scenario of ['new', 'public', 'corrupt', 'prerelease', 'older']) {
  test(`publication handles ${scenario} releases without exposing incomplete assets`, { skip: process.platform === 'win32' }, t => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'apifuck-publish-'))
    t.after(() => fs.rmSync(root, { recursive: true, force: true }))
    fs.mkdirSync(path.join(root, 'bin'))
    fs.mkdirSync(path.join(root, 'docs/release'), { recursive: true })
    fs.mkdirSync(path.join(root, 'assets'))
    const version = scenario === 'prerelease' ? '0.2.0-beta.1' : '0.2.0'
    fs.writeFileSync(path.join(root, `docs/release/${version}.md`), `# ${version}\n\nRelease notes.\n`)
    for (const name of releaseAssetNames) {
      const file = path.join(root, 'assets', name)
      fs.writeFileSync(file, `fixture executable ${name}`)
      fs.writeFileSync(`${file}.sha256`, `${digest(file)}  ${name}\n`)
    }
    const releases = scenario === 'public' ? [{ tag_name: `v${version}`, draft: false, assets: [] }]
      : scenario === 'older' ? [{ tag_name: 'v0.3.0', draft: false, prerelease: false, assets: [] }] : []
    fs.writeFileSync(path.join(root, 'state.json'), JSON.stringify(releases))
    fs.writeFileSync(path.join(root, 'bin/gh'), `#!/usr/bin/env node
const fs = require('node:fs'), path = require('node:path'), crypto = require('node:crypto');
const args = process.argv.slice(2), state = JSON.parse(fs.readFileSync('state.json'));
fs.appendFileSync('calls.jsonl', JSON.stringify(args)+'\\n');
let release = state.find(r => r.tag_name === args[2]);
if (args[0] === 'api') { console.log(JSON.stringify([state])); process.exit(0); }
if (args[1] === 'view') { if (!release) process.exit(1); console.log(JSON.stringify({isDraft:release.draft})); }
if (args[1] === 'create') { if (release) process.exit(1); release = {tag_name:args[2],draft:true,assets:[]}; state.push(release); }
if (args[1] === 'upload') {
 if (!release.draft) throw Error('attempted public mutation');
 release.assets = args.slice(3).filter(a => !a.startsWith('--')).map(file => ({name:path.basename(file),size:fs.statSync(file).size,state:'uploaded',digest:'sha256:'+crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex')}));
 if (process.env.SCENARIO === 'corrupt') release.assets[0].digest = 'sha256:bad';
}
if (args[1] === 'edit') {
 if (args.includes('--draft=false')) release.draft = false;
 if (args.includes('--latest=true')) release.latest = true;
 if (args.includes('--latest=false')) release.latest = false;
 release.prerelease ||= args.includes('--prerelease=true');
}
fs.writeFileSync('state.json', JSON.stringify(state));
`, { mode: 0o755 })
    const result = spawnSync(process.execPath, [script, `v${version}`, 'assets'], {
      cwd: root, encoding: 'utf8', timeout: 30000,
      env: { ...process.env, PATH: `${path.join(root, 'bin')}${path.delimiter}${process.env.PATH}`, GITHUB_REPOSITORY: 'example/apifuck', SCENARIO: scenario },
    })
    const final = JSON.parse(fs.readFileSync(path.join(root, 'state.json'))).find(r => r.tag_name === `v${version}`)
    const calls = fs.readFileSync(path.join(root, 'calls.jsonl'), 'utf8').trim().split('\n').map(JSON.parse)
    if (scenario === 'public' || scenario === 'corrupt') {
      assert.notEqual(result.status, 0)
      assert.equal(calls.some(args => args.includes('--draft=false')), false)
      if (scenario === 'public') assert.equal(calls.some(args => args[1] === 'upload' || args[1] === 'edit'), false)
      else assert.equal(final.draft, true)
    } else {
      assert.equal(result.status, 0, result.stderr)
      assert.equal(final.draft, false)
      assert.equal(final.assets.length, releaseAssetNames.length + 1)
      assert.equal(final.latest, scenario === 'new')
    }
  })
}
