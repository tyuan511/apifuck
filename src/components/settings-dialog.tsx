import type { AppPrimaryColor, AppTheme } from '@/lib/app-config'
import { getName, getVersion } from '@tauri-apps/api/app'
import {
  CheckIcon,
  InfoIcon,
  MonitorCogIcon,
  MoonStarIcon,
  PaletteIcon,
  RefreshCcwIcon,
  SunMediumIcon,
  XIcon,
} from 'lucide-react'
import { useEffect, useState } from 'react'
import { createPortal } from 'react-dom'
import { Button } from '@/components/ui/button'
import { Separator } from '@/components/ui/separator'
import { primaryColorOptions } from '@/lib/appearance'
import { manuallyCheckForAppUpdates } from '@/lib/updater'
import { cn } from '@/lib/utils'
import appIcon from '../../src-tauri/icons/128x128.png'

type SettingsSection = 'appearance' | 'about'

interface SettingsDialogProps {
  currentPrimaryColor: AppPrimaryColor
  currentTheme: AppTheme
  onOpenChange: (open: boolean) => void
  onPrimaryColorChange: (value: AppPrimaryColor) => void
  onThemeChange: (value: AppTheme) => void
  open: boolean
}

interface ThemeOptionCardProps {
  active: boolean
  description: string
  icon: typeof MonitorCogIcon
  label: string
  onSelect: () => void
}

const menuItems: Array<{
  description: string
  icon: typeof PaletteIcon
  label: string
  value: SettingsSection
}> = [
  {
    value: 'appearance',
    label: '外观设置',
    description: '主题模式和主色',
    icon: PaletteIcon,
  },
  {
    value: 'about',
    label: '关于',
    description: '应用信息与更新',
    icon: InfoIcon,
  },
]

const themeOptions: Array<{
  description: string
  icon: typeof MonitorCogIcon
  label: string
  value: AppTheme
}> = [
  {
    value: 'system',
    label: '跟随系统',
    description: '跟随系统明暗切换',
    icon: MonitorCogIcon,
  },
  {
    value: 'light',
    label: '浅色',
    description: '使用浅色界面',
    icon: SunMediumIcon,
  },
  {
    value: 'dark',
    label: '深色',
    description: '使用深色界面',
    icon: MoonStarIcon,
  },
]

function ThemeOptionCard(props: ThemeOptionCardProps) {
  const Icon = props.icon

  return (
    <button
      type="button"
      className={cn(
        'flex min-w-0 items-start gap-3 rounded-xl border p-3 text-left transition',
        props.active
          ? 'border-primary bg-primary/8 text-foreground shadow-sm'
          : 'border-border bg-background hover:border-primary/40 hover:bg-muted/40',
      )}
      onClick={props.onSelect}
    >
      <div className={cn(
        'mt-0.5 flex size-9 shrink-0 items-center justify-center rounded-lg border',
        props.active
          ? 'border-primary/20 bg-primary text-primary-foreground'
          : 'border-border bg-muted text-muted-foreground',
      )}
      >
        <Icon />
      </div>
      <div className="min-w-0 flex-1">
        <div className="flex items-center justify-between gap-2">
          <span className="font-medium">{props.label}</span>
          {props.active ? <CheckIcon className="text-primary" /> : null}
        </div>
        <p className="mt-1 text-xs leading-5 text-muted-foreground">{props.description}</p>
      </div>
    </button>
  )
}

function PrimaryColorCard(props: {
  active: boolean
  description: string
  label: string
  previewClassName: string
  onSelect: () => void
}) {
  return (
    <button
      type="button"
      className={cn(
        'flex min-w-0 items-center gap-3 rounded-xl border p-3 text-left transition',
        props.active
          ? 'border-primary bg-primary/8 text-foreground shadow-sm'
          : 'border-border bg-background hover:border-primary/40 hover:bg-muted/40',
      )}
      onClick={props.onSelect}
    >
      <span className={cn('size-5 shrink-0 rounded-full ring-2 ring-background', props.previewClassName)} />
      <div className="min-w-0 flex-1">
        <div className="flex items-center justify-between gap-2">
          <span className="font-medium">{props.label}</span>
          {props.active ? <CheckIcon className="text-primary" /> : null}
        </div>
        <p className="mt-1 text-xs leading-5 text-muted-foreground">{props.description}</p>
      </div>
    </button>
  )
}

const colorPreviewClassNames: Record<AppPrimaryColor, string> = {
  slate: 'bg-neutral-700 dark:bg-neutral-200',
  blue: 'bg-blue-500 dark:bg-blue-400',
  green: 'bg-emerald-500 dark:bg-emerald-400',
  amber: 'bg-amber-500 dark:bg-amber-400',
  rose: 'bg-rose-500 dark:bg-rose-400',
  violet: 'bg-violet-500 dark:bg-violet-400',
}

const releaseNotesModules = import.meta.glob('../../docs/release/*.md', {
  query: '?raw',
  import: 'default',
  eager: true,
}) as Record<string, string>

const releaseNotesByVersion = Object.fromEntries(
  Object.entries(releaseNotesModules).map(([path, content]) => {
    const match = path.match(/\/(\d+\.\d+\.\d+)\.md$/)
    return [match?.[1] ?? path, content]
  }),
) as Record<string, string>

function ReleaseNotesContent(props: { version: string }) {
  const content = releaseNotesByVersion[props.version]?.trim()

  if (!content) {
    return (
      <p className="text-sm leading-6 text-muted-foreground">
        当前版本暂未提供版本介绍。
      </p>
    )
  }

  const lines = content.split('\n').map(line => line.trimEnd())

  return (
    <div className="space-y-1">
      {lines.map((line, index) => {
        const key = `${index}-${line}`
        if (!line.trim()) {
          return <div key={key} className="h-1" />
        }

        if (line.startsWith('## ')) {
          return (
            <h3 key={key} className="pt-1 text-sm font-semibold text-foreground">
              {line.slice(3)}
            </h3>
          )
        }

        if (line.startsWith('- ')) {
          return (
            <div key={key} className="flex gap-2 text-sm leading-6 text-muted-foreground">
              <span className="mt-[0.45rem] size-1 shrink-0 rounded-full bg-muted-foreground/60" />
              <span>{line.slice(2)}</span>
            </div>
          )
        }

        return (
          <p key={key} className="text-sm leading-6 text-muted-foreground">
            {line}
          </p>
        )
      })}
    </div>
  )
}

export function SettingsDialog(props: SettingsDialogProps) {
  const [activeSection, setActiveSection] = useState<SettingsSection>('appearance')
  const [appName, setAppName] = useState('ApiFuck')
  const [appVersion, setAppVersion] = useState('0.0.1')
  const [isCheckingUpdates, setIsCheckingUpdates] = useState(false)

  useEffect(() => {
    if (!props.open) {
      return
    }

    let cancelled = false

    void Promise.all([getName(), getVersion()])
      .then(([name, version]) => {
        if (cancelled) {
          return
        }

        setAppName(name)
        setAppVersion(version)
      })
      .catch(() => {
        if (cancelled) {
          return
        }

        setAppName('ApiFuck')
        setAppVersion('0.0.1')
      })

    return () => {
      cancelled = true
    }
  }, [props.open])

  useEffect(() => {
    if (props.open) {
      setActiveSection('appearance')
    }
  }, [props.open])

  useEffect(() => {
    if (!props.open) {
      return
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        props.onOpenChange(false)
      }
    }

    window.addEventListener('keydown', handleKeyDown)
    return () => {
      window.removeEventListener('keydown', handleKeyDown)
    }
  }, [props.open, props.onOpenChange])

  async function handleCheckUpdates() {
    setIsCheckingUpdates(true)
    try {
      await manuallyCheckForAppUpdates()
    }
    finally {
      setIsCheckingUpdates(false)
    }
  }

  if (!props.open || typeof document === 'undefined') {
    return null
  }

  return createPortal(
    <div
      aria-labelledby="settings-dialog-title"
      aria-modal="true"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/10 p-4 supports-backdrop-filter:backdrop-blur-xs"
      role="dialog"
      onClick={() => props.onOpenChange(false)}
    >
      <div
        className="relative grid h-[680px] max-h-[calc(100vh-3rem)] w-full max-w-4xl min-w-0 overflow-hidden rounded-xl bg-popover text-sm text-popover-foreground ring-1 ring-foreground/10 md:grid-cols-[220px_minmax(0,1fr)]"
        onClick={event => event.stopPropagation()}
      >
        <Button
          className="absolute top-2 right-2 z-10"
          size="icon-sm"
          variant="ghost"
          onClick={() => props.onOpenChange(false)}
        >
          <XIcon />
          <span className="sr-only">关闭设置</span>
        </Button>

        <aside className="flex h-full min-h-0 flex-col border-b bg-muted/25 md:border-r md:border-b-0">
          <div className="px-4 pt-4 pb-3">
            <p className="text-base font-semibold text-foreground" id="settings-dialog-title">设置</p>
            <p className="mt-1 text-xs text-muted-foreground">外观、主色和应用信息</p>
          </div>
          <nav className="flex flex-col gap-1 px-2.5 pb-3">
            {menuItems.map((item) => {
              const Icon = item.icon
              const active = activeSection === item.value

              return (
                <button
                  key={item.value}
                  type="button"
                  className={cn(
                    'flex items-start gap-2.5 rounded-lg px-2.5 py-2 text-left transition',
                    active
                      ? 'bg-background text-foreground ring-1 ring-border'
                      : 'text-muted-foreground hover:bg-background/70 hover:text-foreground',
                  )}
                  onClick={() => setActiveSection(item.value)}
                >
                  <div className={cn(
                    'mt-0.5 flex size-7 shrink-0 items-center justify-center rounded-md',
                    active ? 'bg-primary text-primary-foreground' : 'bg-muted text-muted-foreground',
                  )}
                  >
                    <Icon className="size-4" />
                  </div>
                  <div className="min-w-0">
                    <div className="text-sm font-medium">{item.label}</div>
                    <div className="mt-0.5 text-[11px] leading-4 text-muted-foreground">{item.description}</div>
                  </div>
                </button>
              )
            })}
          </nav>
        </aside>

        <section className="flex h-full min-h-0 min-w-0 flex-col overflow-hidden">
          {activeSection === 'appearance'
            ? (
                <div className="flex min-h-0 flex-1 flex-col">
                  <div className="shrink-0 px-6 pt-4">
                    <h2 className="text-lg font-semibold text-foreground">外观设置</h2>
                    <p className="mt-0.5 text-sm text-muted-foreground">调整系统主题模式和应用主色，修改会立即生效。</p>
                  </div>

                  <Separator className="mt-4 shrink-0" />

                  <div className="min-h-0 flex-1 overflow-y-auto">
                    <div className="flex flex-col gap-8 px-6 py-6">
                      <div>
                        <div className="mb-3">
                          <p className="font-medium text-foreground">系统主题</p>
                          <p className="mt-1 text-sm text-muted-foreground">支持亮色、暗色和跟随系统，默认跟随系统。</p>
                        </div>
                        <div className="grid gap-3 lg:grid-cols-3">
                          {themeOptions.map(option => (
                            <ThemeOptionCard
                              key={option.value}
                              active={props.currentTheme === option.value}
                              description={option.description}
                              icon={option.icon}
                              label={option.label}
                              onSelect={() => props.onThemeChange(option.value)}
                            />
                          ))}
                        </div>
                      </div>

                      <div>
                        <div className="mb-3">
                          <p className="font-medium text-foreground">主色</p>
                          <p className="mt-1 text-sm text-muted-foreground">选择常见主色方案，直接更新应用的 `--primary` 视觉表达。</p>
                        </div>
                        <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
                          {primaryColorOptions.map(option => (
                            <PrimaryColorCard
                              key={option.value}
                              active={props.currentPrimaryColor === option.value}
                              description={option.description}
                              label={option.label}
                              previewClassName={colorPreviewClassNames[option.value]}
                              onSelect={() => props.onPrimaryColorChange(option.value)}
                            />
                          ))}
                        </div>
                      </div>
                    </div>
                  </div>
                </div>
              )
            : (
                <div className="flex min-h-0 flex-1 flex-col">
                  <div className="shrink-0 px-6 pt-4">
                    <h2 className="text-lg font-semibold text-foreground">关于</h2>
                    <p className="mt-0.5 text-sm text-muted-foreground">查看应用图标、名称、版本号，并手动检查更新。</p>
                  </div>

                  <Separator className="mt-4 shrink-0" />

                  <div className="min-h-0 flex-1 overflow-y-auto">
                    <div className="flex flex-col gap-6 px-6 py-6">
                      <div className="rounded-2xl border bg-card p-5">
                        <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
                          <div className="flex items-center gap-4">
                            <img
                              alt="ApiFuck 应用图标"
                              className="size-16"
                              src={appIcon}
                            />
                            <div>
                              <p className="text-lg font-semibold text-foreground">{appName}</p>
                              <p className="mt-1 text-sm text-muted-foreground">专注于本地接口调试和请求组织的桌面应用。</p>
                              <p className="mt-2 text-sm font-medium text-foreground">
                                当前版本：
                                {' '}
                                v
                                {appVersion}
                              </p>
                            </div>
                          </div>
                          <Button onClick={() => { void handleCheckUpdates() }} disabled={isCheckingUpdates}>
                            <RefreshCcwIcon data-icon="inline-start" className={cn(isCheckingUpdates && 'animate-spin')} />
                            {isCheckingUpdates ? '检查中...' : '检查更新'}
                          </Button>
                        </div>
                      </div>

                      <div className="rounded-2xl border bg-card p-5">
                        <div className="mb-3">
                          <p className="text-sm font-semibold text-foreground">
                            版本介绍
                          </p>
                        </div>
                        <ReleaseNotesContent version={appVersion} />
                      </div>
                    </div>
                  </div>
                </div>
              )}
        </section>
      </div>
    </div>,
    document.body,
  )
}
