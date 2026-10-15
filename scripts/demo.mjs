#!/usr/bin/env node
// Launch a private local API and a writable copy of the complete playground.
import { spawn } from 'node:child_process';
import { cp, mkdir, readFile, writeFile, access, rename } from 'node:fs/promises';
import { dirname, resolve, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createInterface } from 'node:readline';
const repository = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const args = new Set(process.argv.slice(2));
const check = args.has('--check');
const headless = args.has('--headless');
const suffix = process.platform === 'win32' ? '.exe' : '';
const children = new Set();
function cleanup() { for (const child of children) if (child.exitCode === null && child.signalCode === null) child.kill(); }
for (const signal of ['SIGINT', 'SIGTERM', 'SIGHUP']) process.on(signal, () => { cleanup(); process.exit(0); });
process.on('exit', cleanup);
function start(command, argv, options = {}) {
  const child = spawn(command, argv, { cwd: repository, ...options });
  children.add(child); child.once('exit', () => children.delete(child)); return child;
}
async function command(program, argv, capture = false) {
  const child = start(program, argv, { stdio: capture ? ['ignore', 'pipe', 'pipe'] : 'inherit' });
  let stdout = '', stderr = '';
  if (capture) { child.stdout.on('data', chunk => stdout += chunk); child.stderr.on('data', chunk => stderr += chunk); }
  const code = await new Promise((resolve, reject) => { child.once('error', reject); child.once('exit', code => resolve(code)); });
  if (code !== 0) throw new Error(`${program} exited with ${code}\n${stderr}`);
  return stdout;
}
async function exists(path) { try { await access(path); return true; } catch { return false; } }
async function main() {
  if (check) {
    await command('cargo', ['test', '--locked', '-p', 'apifuck-demo', '--test', 'playground']);
    return;
  }
  if (!args.has('--no-build')) {
    console.log('Building ApiFuck native + local demo API…');
    await command('cargo', ['+1.95', 'build', '--manifest-path', join(repository, 'Cargo.toml'), '--locked', '-p', 'apifuck-demo', ...(headless ? [] : ['-p', 'apifuck-desktop'])]);
  }
  const workspace = join(repository, '.demo-workspace');
  if (args.has('--fresh') && await exists(workspace)) {
    const backup = `${workspace}-backup-${Date.now()}`;
    await rename(workspace, backup);
    console.log(`Previous demo workspace preserved at ${backup}`);
  }
  const fresh = !await exists(workspace);
  if (fresh) await cp(join(repository, 'examples/playground'), workspace, { recursive: true });
  const server = start(join(repository, `target/debug/apifuck-demo${suffix}`), [], { env: { ...process.env, APIFUCK_DEMO_PORT: '0', APIFUCK_DEMO_WORKSPACE: workspace }, stdio: ['ignore', 'pipe', 'inherit'] });
  const lines = createInterface({ input: server.stdout });
  const endpoint = await new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error('Demo API startup timed out')), 15000);
    server.once('error', error => { clearTimeout(timer); reject(error); });
    server.once('exit', code => { clearTimeout(timer); reject(new Error(`Demo API exited: ${code}`)); });
    lines.once('line', line => { clearTimeout(timer); try { const data = JSON.parse(line); if (data.service !== 'apifuck-demo') throw new Error('Unexpected demo API'); resolve(data); } catch (error) { reject(error); } });
  });
  const health = await fetch(`${endpoint.base_url}/health`).then(response => response.json());
  if (health.service !== 'apifuck-demo') throw new Error('Demo API readiness check failed');
  const local = join(workspace, '.apifuck'); await mkdir(local, { recursive: true });
  const variablesPath = join(local, 'variables.json');
  const variables = await exists(variablesPath) ? JSON.parse(await readFile(variablesPath, 'utf8')) : {};
  variables['env:local'] = { ...(variables['env:local'] || {}), base_url: endpoint.base_url, ws_url: endpoint.ws_url, created_user_id: '3', token: 'demo-token-apifuck' };
  await writeFile(variablesPath, JSON.stringify(variables, null, 2) + '\n');
  const statePath = join(local, 'workspace.json');
  const state = await exists(statePath) ? JSON.parse(await readFile(statePath, 'utf8')) : {
    tabs: ['00-overview/health.yml', '02-users/list.yml', '04-payloads/json.yml', '05-graphql/user.yml'],
    active: '00-overview/health.yml', appearance: 'light', accent: 'amber',
    collapsed_folders: ['01-auth/folder.yml', '03-params/folder.yml', '04-payloads/folder.yml', '05-graphql/folder.yml', '06-realtime/folder.yml', '07-errors/folder.yml', '08-utilities/folder.yml'],
  };
  state.environment = 'local';
  await writeFile(statePath, JSON.stringify(state, null, 2) + '\n');
  console.log(`Demo API: ${endpoint.base_url}\nWorkspace: ${workspace}`);
  if (headless) {
    console.log(`Ready. Open "${workspace}" in ApiFuck and select the local environment. Ctrl+C stops the API.`);
    await new Promise(resolve => server.once('exit', resolve));
  } else {
    console.log('Opening ApiFuck. Quit the app or press Ctrl+C here to stop the local API.');
    await command(join(repository, `target/debug/apifuck-desktop${suffix}`), [workspace]);
    cleanup();
  }
}
main().catch(error => { console.error(error.message); cleanup(); process.exitCode = 1; });
