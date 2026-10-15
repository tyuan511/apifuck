import { test } from "node:test";
import assert from "node:assert/strict";
import { assetNames, nativeAssets } from "../src/releases.mjs";
const release = () => ({
  tag_name: "v0.1.0",
  assets: Object.values(assetNames).map((name) => ({
    name,
    size: 1024,
    browser_download_url: `https://github.com/tyuan511/apifuck/releases/download/v0.1.0/${name}`,
  })),
});
test("offers exact native binaries from the verified repository", () => {
  assert.equal(Object.keys(nativeAssets(release())).length, 3);
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
