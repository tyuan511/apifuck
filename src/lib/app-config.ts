import type { WorkbenchPanelTab } from '@/features/workbench/types'
import type { ProjectSnapshot } from '@/lib/project'
import { invoke } from '@tauri-apps/api/core'

export type AppTheme = 'light' | 'dark' | 'system'
export type AppPrimaryColor = 'slate' | 'blue' | 'green' | 'amber' | 'rose' | 'violet'

export interface OpenRequestTab {
  entityType: 'request' | 'collection' | 'project'
  requestId: string
  entityId: string
  title: string
  method: string
  dirty: boolean
  lastFocusedAt: number
  editorTab: WorkbenchPanelTab
}

export interface AppConfig {
  schemaVersion: number
  createdAt: string
  updatedAt: string
  lastOpenedProjectPath: string | null
  recentProjectPaths: string[]
  theme: AppTheme
  primaryColor: AppPrimaryColor
  openRequestTabs: OpenRequestTab[]
  activeRequestId: string | null
}

export interface UpdateAppConfigInput {
  lastOpenedProjectPath: string | null
  recentProjectPaths?: string[]
  theme: AppTheme
  primaryColor: AppPrimaryColor
}

export interface UpdateTabStateInput {
  openRequestTabs: OpenRequestTab[]
  activeRequestId: string | null
}

export interface AppStartupState {
  appConfig: AppConfig
  projectPath: string
  projectSnapshot: ProjectSnapshot
}

export function openStartupProject() {
  return invoke<AppStartupState>('open_startup_project')
}

export function readAppConfig() {
  return invoke<AppConfig>('read_app_config')
}

export function updateAppConfig(input: UpdateAppConfigInput) {
  return invoke<AppConfig>('update_app_config', { input })
}

export function readTabState() {
  return invoke<[OpenRequestTab[], string | null]>('read_tab_state')
}

export function updateTabState(input: UpdateTabStateInput) {
  return invoke<AppConfig>('update_tab_state', { input })
}
