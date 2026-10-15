import assert from 'node:assert/strict'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { test } from 'node:test'
import { prepareAppDir, verifyAppImage } from './linux-package.mjs'

test('AppDir has desktop integration, icon, and an executable launcher for bundled libraries', t => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'apifuck-appdir-'))
  t.after(() => fs.rmSync(root, { recursive: true, force: true }))
  const binary = path.join(root, 'binary'), icon = path.join(root, 'icon')
  fs.writeFileSync(binary, 'program'); fs.writeFileSync(icon, 'image')
  const appdir = path.join(root, 'ApiFuck.AppDir')
  prepareAppDir({ binary, icon, appdir })
  assert.equal(fs.readFileSync(path.join(appdir, 'usr/bin/apifuck-desktop'), 'utf8'), 'program')
  assert.match(fs.readFileSync(path.join(appdir, 'AppRun'), 'utf8'), /LD_LIBRARY_PATH/)
  assert.match(fs.readFileSync(path.join(appdir, 'apifuck.desktop'), 'utf8'), /Exec=apifuck-desktop/)
  assert.throws(() => verifyAppImage(binary))
  const bytes = Buffer.alloc(64); bytes.set([127,69,76,70]); bytes[4] = 2; bytes[5] = 1; bytes.set([65,73,2],8); bytes[18] = 62
  fs.writeFileSync(binary, bytes); verifyAppImage(binary)
  bytes[8] = 0; fs.writeFileSync(binary, bytes); assert.throws(() => verifyAppImage(binary))
})
