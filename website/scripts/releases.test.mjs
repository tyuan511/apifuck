import { test } from "node:test";
import assert from "node:assert/strict";
import { assetNames, nativeAssets } from "../src/releases.mjs";
const release = () => ({
  tag_name: "v0.1.0",
  assets: Object.values(assetNames("0.1.0")).map((name) => ({
    name,
    size: 1024,
    browser_download_url: `https://github.com/tyuan511/apifuck/releases/download/v0.1.0/${name}`,
  })),
});
test("offers exact native binaries from the verified repository", () => {
  assert.equal(Object.keys(nativeAssets(release())).length, 3);
});
test("supports legacy assets, prefers versioned names, and rejects other versions", () => {
  const data = release();
  const oldNames = ["apifuck-desktop-aarch64-apple-darwin.dmg", "apifuck-desktop-x86_64-pc-windows-msvc.exe", "apifuck-desktop-x86_64-unknown-linux-gnu.AppImage"];
  const legacy = oldNames.map(name => ({ name, size: 1024, browser_download_url: `https://github.com/tyuan511/apifuck/releases/download/v0.1.0/${name}` }));
  data.assets.unshift(...legacy);
  assert.ok(nativeAssets(data).mac.url.endsWith("ApiFuck-0.1.0-arm64.dmg"));
  data.assets = legacy;
  assert.equal(Object.keys(nativeAssets(data)).length, 3);
  const wrong = release();
  wrong.assets.forEach(asset => { asset.name = asset.name.replace("0.1.0", "9.9.9"); });
  assert.deepEqual(nativeAssets(wrong), {});
  for (const tag of ["v../1.2.3", "v1.2.3-beta.1", "bad", "v01.2.3"]) {
    assert.deepEqual(nativeAssets({ ...release(), tag_name: tag }), {});
  }
});
test("build metadata is URL-encoded without weakening repository checks", () => {
  const version = "1.2.3+build.4";
  const data = { tag_name: `v${version}`, assets: Object.values(assetNames(version)).map(name => ({ name, size: 100, browser_download_url: `https://github.com/tyuan511/apifuck/releases/download/${encodeURIComponent(`v${version}`)}/${encodeURIComponent(name)}` })) };
  assert.equal(Object.keys(nativeAssets(data)).length, 3);
  data.assets[0].browser_download_url += "?other=1";
  assert.equal(nativeAssets(data).mac, undefined);
});
test("does not offer retired macOS archives or Linux binaries", () => {
  const data = release();
  data.assets = data.assets.filter(asset => !asset.name.endsWith('.dmg') && !asset.name.endsWith('.AppImage'));
  for (const name of ['apifuck-desktop-aarch64-apple-darwin.zip', 'apifuck-desktop-aarch64-apple-darwin.app.tar.gz', 'apifuck-desktop-x86_64-unknown-linux-gnu']) {
    data.assets.push({name, size: 1024, browser_download_url: `https://github.com/tyuan511/apifuck/releases/download/v0.1.0/${name}`});
  }
  assert.deepEqual(Object.keys(nativeAssets(data)), ['windows']);
});
test("never offers historical installers, drafts, prereleases or empty assets", () => {
  assert.deepEqual(
    nativeAssets({
      tag_name: "v0.0.8",
      assets: [{ name: "ApiFuck_0.0.8_aarch64.dmg", size: 1000 }],
    }),
    {},
  );
  for (const flag of ["draft", "prerelease"])
    assert.deepEqual(nativeAssets({ ...release(), [flag]: true }), {});
  const empty = release();
  empty.assets.forEach((asset) => (asset.size = 0));
  assert.deepEqual(nativeAssets(empty), {});
});
test("rejects a foreign or mismatched asset URL and allows partial platform availability", () => {
  const data = release();
  data.assets[0].browser_download_url = "https://example.com/malware";
  data.assets.pop();
  assert.deepEqual(Object.keys(nativeAssets(data)), ["windows"]);
});
