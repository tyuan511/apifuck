import type { DownloadEvent } from '@tauri-apps/plugin-updater'
import { relaunch } from '@tauri-apps/plugin-process'
import { check } from '@tauri-apps/plugin-updater'
import { toast } from 'sonner'

let hasCheckedForUpdates = false
let installPromise: Promise<void> | null = null

function formatReleaseNotes(notes?: string) {
  if (!notes) {
    return '发现新版本，准备好后可直接下载安装。'
  }

  const singleLine = notes.replace(/\s+/g, ' ').trim()
  return singleLine.length > 120 ? `${singleLine.slice(0, 117)}...` : singleLine
}

export async function checkForAppUpdates() {
  if (hasCheckedForUpdates) {
    return
  }

  hasCheckedForUpdates = true

  try {
    const update = await check()

    if (!update) {
      return
    }

    toast.info(`发现新版本 v${update.version}`, {
      description: formatReleaseNotes(update.body),
      duration: 12000,
      dismissible: true,
      action: {
        label: '立即更新',
        onClick: () => {
          void installAppUpdate()
        },
      },
      cancel: {
        label: '稍后',
        onClick: () => {},
      },
    })
  }
  catch (error) {
    console.error('Failed to check for app updates', error)
  }
}

export async function installAppUpdate() {
  if (installPromise) {
    return installPromise
  }

  installPromise = (async () => {
    try {
      const update = await check()

      if (!update) {
        toast.success('当前已经是最新版本')
        return
      }

      let downloadedBytes = 0
      let totalBytes = 0

      const toastId = toast.loading(`正在下载更新 v${update.version}`, {
        description: '下载完成后会自动安装，并重新启动应用。',
        duration: Infinity,
      })

      const handleEvent = (event: DownloadEvent) => {
        if (event.event === 'Started') {
          totalBytes = event.data.contentLength ?? 0
          return
        }

        if (event.event === 'Progress') {
          downloadedBytes += event.data.chunkLength

          if (totalBytes > 0) {
            const progress = Math.min(100, Math.round((downloadedBytes / totalBytes) * 100))
            toast.loading(`正在下载更新 v${update.version}`, {
              id: toastId,
              description: `下载进度 ${progress}%`,
              duration: Infinity,
            })
          }

          return
        }

        toast.loading(`正在安装更新 v${update.version}`, {
          id: toastId,
          description: '安装完成后会立即重启应用。',
          duration: Infinity,
        })
      }

      await update.downloadAndInstall(handleEvent)

      toast.success(`已安装更新 v${update.version}`, {
        id: toastId,
        description: '应用即将自动重启。',
      })

      await relaunch()
    }
    catch (error) {
      const message = error instanceof Error ? error.message : String(error)
      toast.error('自动更新失败', {
        description: message,
      })
      throw error
    }
    finally {
      installPromise = null
    }
  })()

  return installPromise
}
