import fs from 'node:fs'
import path from 'node:path'
import { setTimeout as sleep } from 'node:timers/promises'

function fail(message) {
  console.error(message)
  process.exit(1)
}

function parseArgs(argv) {
  const [version, ...rest] = argv

  if (!version) {
    fail('Usage: node scripts/release/update-readme-downloads.mjs <version> [--repo owner/repo]')
  }

  let repo = process.env.GITHUB_REPOSITORY

  for (let index = 0; index < rest.length; index += 1) {
    const arg = rest[index]

    if (arg === '--repo') {
      repo = rest[index + 1]
      index += 1
    }
  }

  if (!repo) {
    fail('Missing repository. Pass --repo <owner/repo> or set GITHUB_REPOSITORY.')
  }

  return { repo, version }
}

function buildHeaders() {
  const headers = {
    Accept: 'application/vnd.github+json',
    'User-Agent': 'apifuck-release-readme-sync',
  }

  const token = process.env.GH_TOKEN || process.env.GITHUB_TOKEN

  if (token) {
    headers.Authorization = `Bearer ${token}`
  }

  return headers
}

async function fetchRelease(repo, tagName) {
  const response = await fetch(`https://api.github.com/repos/${repo}/releases/tags/${tagName}`, {
    headers: buildHeaders(),
  })

  if (response.status === 404) {
    return null
  }

  if (!response.ok) {
    const body = await response.text()
    throw new Error(`Failed to fetch release ${tagName}: ${response.status} ${response.statusText}\n${body}`)
  }

  return response.json()
}

async function listReleases(repo) {
  const response = await fetch(`https://api.github.com/repos/${repo}/releases?per_page=100`, {
    headers: buildHeaders(),
  })

  if (!response.ok) {
    const body = await response.text()
    throw new Error(`Failed to list releases for ${repo}: ${response.status} ${response.statusText}\n${body}`)
  }

  return response.json()
}

async function resolveRelease(repo, tagName) {
  const release = await fetchRelease(repo, tagName)

  if (release) {
    return release
  }

  const releases = await listReleases(repo)
  return releases.find(item => item.tag_name === tagName) ?? null
}

function findAsset(assets, matcher) {
  return assets.find(asset => matcher.test(asset.name))
}

function buildReleaseDownloadUrl(repo, tagName, assetName) {
  return `https://github.com/${repo}/releases/download/${tagName}/${encodeURIComponent(assetName)}`
}

function buildReleasePageUrl(repo, tagName) {
  return `https://github.com/${repo}/releases/tag/${tagName}`
}

async function resolveAssets(repo, tagName) {
  const requiredMatchers = {
    macos: /\.dmg$/i,
    windows: /-setup\.exe$/i,
    linux: /\.AppImage$/i,
  }

  for (let attempt = 1; attempt <= 12; attempt += 1) {
    const release = await resolveRelease(repo, tagName)

    if (!release) {
      if (attempt < 12) {
        console.log(`Release ${tagName} is not ready yet (attempt ${attempt}/12), retrying in 5 seconds...`)
        await sleep(5000)
        continue
      }

      fail(`Release ${tagName} was not found.`)
    }

    const assets = release.assets ?? []
    const selectedAssets = {
      macos: findAsset(assets, requiredMatchers.macos),
      windows: findAsset(assets, requiredMatchers.windows),
      linux: findAsset(assets, requiredMatchers.linux),
    }

    if (selectedAssets.macos && selectedAssets.windows && selectedAssets.linux) {
      return {
        release,
        selectedAssets,
      }
    }

    if (attempt < 12) {
      console.log(`Release assets are not ready yet (attempt ${attempt}/12), retrying in 5 seconds...`)
      await sleep(5000)
    }
  }

  fail(`Release ${tagName} is missing one or more required assets (.dmg, -setup.exe, .AppImage).`)
}

function replaceBetweenMarkers(content, startMarker, endMarker, replacement) {
  const start = content.indexOf(startMarker)
  const end = content.indexOf(endMarker)

  if (start === -1 || end === -1 || end < start) {
    fail(`Could not find marker pair ${startMarker} ... ${endMarker}`)
  }

  const before = content.slice(0, start + startMarker.length)
  const after = content.slice(end)
  return `${before}\n${replacement}\n${after}`
}

function writeIfChanged(filePath, nextContent) {
  const currentContent = fs.readFileSync(filePath, 'utf8')

  if (currentContent === nextContent) {
    return false
  }

  fs.writeFileSync(filePath, nextContent)
  return true
}

function buildChineseTable(selectedAssets) {
  return [
    '| 平台 | 下载地址 |',
    '|------|---------|',
    `| macOS | [${selectedAssets.macos.name}](${selectedAssets.macos.downloadUrl}) |`,
    `| Windows | [${selectedAssets.windows.name}](${selectedAssets.windows.downloadUrl}) |`,
    `| Linux | [${selectedAssets.linux.name}](${selectedAssets.linux.downloadUrl}) |`,
  ].join('\n')
}

function buildEnglishDownloadBlock(tagName, selectedAssets, releaseHtmlUrl) {
  return [
    `- Latest release: [${tagName}](${releaseHtmlUrl})`,
    `- macOS: [${selectedAssets.macos.name}](${selectedAssets.macos.downloadUrl})`,
    `- Windows: [${selectedAssets.windows.name}](${selectedAssets.windows.downloadUrl})`,
    `- Linux: [${selectedAssets.linux.name}](${selectedAssets.linux.downloadUrl})`,
  ].join('\n')
}

async function main() {
  const { repo, version } = parseArgs(process.argv.slice(2))
  const tagName = version.startsWith('v') ? version : `v${version}`
  const rootDir = process.cwd()
  const readmePath = path.join(rootDir, 'README.md')
  const readmeEnPath = path.join(rootDir, 'README_en.md')
  const { selectedAssets } = await resolveAssets(repo, tagName)
  const normalizedAssets = {
    macos: {
      ...selectedAssets.macos,
      downloadUrl: buildReleaseDownloadUrl(repo, tagName, selectedAssets.macos.name),
    },
    windows: {
      ...selectedAssets.windows,
      downloadUrl: buildReleaseDownloadUrl(repo, tagName, selectedAssets.windows.name),
    },
    linux: {
      ...selectedAssets.linux,
      downloadUrl: buildReleaseDownloadUrl(repo, tagName, selectedAssets.linux.name),
    },
  }
  const releasePageUrl = buildReleasePageUrl(repo, tagName)

  const readmeContent = fs.readFileSync(readmePath, 'utf8')
  const nextReadmeContent = replaceBetweenMarkers(
    readmeContent,
    '<!-- release-downloads:start -->',
    '<!-- release-downloads:end -->',
    buildChineseTable(normalizedAssets),
  )

  const changedFiles = []

  if (writeIfChanged(readmePath, nextReadmeContent)) {
    changedFiles.push('README.md')
  }

  if (fs.existsSync(readmeEnPath)) {
    const readmeEnContent = fs.readFileSync(readmeEnPath, 'utf8')
    const nextReadmeEnContent = replaceBetweenMarkers(
      readmeEnContent,
      '<!-- release-downloads-en:start -->',
      '<!-- release-downloads-en:end -->',
      buildEnglishDownloadBlock(tagName, normalizedAssets, releasePageUrl),
    )

    if (writeIfChanged(readmeEnPath, nextReadmeEnContent)) {
      changedFiles.push('README_en.md')
    }
  }

  if (changedFiles.length === 0) {
    console.log(`README download links are already up to date for ${tagName}`)
    return
  }

  console.log(`Updated release download links for ${tagName}: ${changedFiles.join(', ')}`)
}

main().catch((error) => {
  fail(error instanceof Error ? error.message : String(error))
})
