import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const root = dirname(fileURLToPath(import.meta.url));
const output = resolve(root, '../../crates/core/resources/javascript');
const commit = '23fcb63383afdfa16df0da61476a6051930ada5b';
const cache = resolve(root, '.cache');
await mkdir(cache, { recursive: true });
// Keep the temporary source below root so its imports can resolve node_modules.
const upstream = await mkdtemp(resolve(cache, 'bruno-'));

function git(...args) {
  const result = spawnSync('git', ['-C', upstream, ...args], { stdio: 'inherit' });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`git ${args[0]} failed (${result.status})`);
}

async function bundle(entrypoint, plugins = []) {
  const result = await Bun.build({
    entrypoints: [resolve(root, entrypoint)],
    target: 'browser',
    format: 'iife',
    minify: true,
    plugins,
  });
  if (!result.success) throw new AggregateError(result.logs, `Failed to bundle ${entrypoint}`);
  if (result.outputs.length !== 1) throw new Error(`Unexpected outputs for ${entrypoint}`);
  return result.outputs[0].text();
}

try {
  git('init', '--quiet');
  git('remote', 'add', 'origin', 'https://github.com/usebruno/bruno.git');
  git('sparse-checkout', 'set', 'packages/bruno-common', 'packages/bruno-filestore', 'packages/bruno-lang');
  git('fetch', '--depth=1', '--filter=blob:none', 'origin', commit);
  git('checkout', '--detach', 'FETCH_HEAD');

  // Preserve request-scoped response variables on Bru round trips.
  const actions = resolve(upstream, 'packages/bruno-filestore/src/formats/yml/common/actions.ts');
  const source = await readFile(actions, 'utf8');
  if (source.split('local: false').length !== 2) throw new Error('Bruno actions patch no longer matches');
  await writeFile(actions, source.replace('local: false', "local: setVarAction.variable?.scope === 'request'"));

  const bridge = await bundle('bridge.ts', [{
    name: 'native-conversion-boundary',
    setup(build) {
      build.onResolve({ filter: /^@bruno-source\// }, ({ path }) => ({
        path: resolve(upstream, 'packages', path.slice('@bruno-source/'.length)),
      }));
      for (const [name, file] of [
        ['@usebruno/lang', 'lang-facade.ts'],
        ['@usebruno/common', 'common-facade.ts'],
        ['@usebruno/common/utils', 'utils-facade.ts'],
        ['nanoid', 'node_modules/nanoid/non-secure/index.js'],
      ]) {
        build.onResolve({ filter: new RegExp(`^${name}$`) }, () => ({ path: resolve(root, file) }));
      }
    },
  }]);
  const runtime = await bundle('runtime.ts');
  await mkdir(output, { recursive: true });
  await writeFile(resolve(output, 'bridge.bundle.js'), bridge);
  await writeFile(resolve(output, 'runtime.bundle.js'), runtime);
} finally {
  await rm(upstream, { recursive: true, force: true });
}
