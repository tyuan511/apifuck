export const assetNames = {
  mac: "apifuck-desktop-aarch64-apple-darwin.dmg",
  windows: "apifuck-desktop-x86_64-pc-windows-msvc.exe",
  linux: "apifuck-desktop-x86_64-unknown-linux-gnu.AppImage",
};
// Only the three supported installer assets are offered.
export function nativeAssets(release) {
  if (
    !release ||
    release.draft ||
    release.prerelease ||
    !Array.isArray(release.assets)
  )
    return {};
  return Object.fromEntries(
    Object.entries(assetNames).flatMap(([platform, name]) => {
      const asset = release.assets.find((a) => a.name === name && a.size > 0);
      if (!asset || typeof release.tag_name !== "string") return [];
      const expected = `https://github.com/tyuan511/apifuck/releases/download/${encodeURIComponent(release.tag_name)}/${asset.name}`;
      return asset.browser_download_url === expected
        ? [[platform, { url: expected, version: release.tag_name }]]
        : [];
    }),
  );
}
