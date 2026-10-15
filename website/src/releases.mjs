const legacyAssetNames = {
  mac: "apifuck-desktop-aarch64-apple-darwin.dmg",
  windows: "apifuck-desktop-x86_64-pc-windows-msvc.exe",
  linux: "apifuck-desktop-x86_64-unknown-linux-gnu.AppImage",
};
export function assetNames(version) {
  const identifier = "(?:0|[1-9][0-9]*|[0-9]*[A-Za-z-][0-9A-Za-z-]*)";
  const semver = new RegExp(`^(0|[1-9][0-9]*)\\.(0|[1-9][0-9]*)\\.(0|[1-9][0-9]*)(?:-(${identifier}(?:\\.${identifier})*))?(?:\\+([0-9A-Za-z-]+(?:\\.[0-9A-Za-z-]+)*))?$`);
  if (typeof version !== "string" || !semver.test(version)) throw new Error("Invalid release version");
  return {
    mac: `ApiFuck-${version}-arm64.dmg`,
    windows: `ApiFuck-${version}.exe`,
    linux: `ApiFuck-${version}.AppImage`,
  };
}
// Only the three supported installer assets are offered.
export function nativeAssets(release) {
  if (
    !release ||
    release.draft ||
    release.prerelease ||
    !Array.isArray(release.assets)
  )
    return {};
  if (typeof release.tag_name !== "string" || !release.tag_name.startsWith("v")) return {};
  const version = release.tag_name.slice(1);
  if (version.split("+")[0].includes("-")) return {};
  let names;
  try { names = assetNames(version); } catch { return {}; }
  return Object.fromEntries(
    Object.entries(names).flatMap(([platform, name]) => {
      const asset = [name, legacyAssetNames[platform]].map(candidate => release.assets.find((a) => a.name === candidate && a.size > 0)).find(Boolean);
      if (!asset) return [];
      const expected = `https://github.com/tyuan511/apifuck/releases/download/${encodeURIComponent(release.tag_name)}/${encodeURIComponent(asset.name)}`;
      return asset.browser_download_url === expected || asset.browser_download_url === expected.replaceAll("%2B", "+")
        ? [[platform, { url: expected, version: release.tag_name }]]
        : [];
    }),
  );
}
