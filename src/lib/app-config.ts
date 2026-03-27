import type { WorkspaceSnapshot } from '@/lib/workspace'
import { invoke } from '@tauri-apps/api/core'

export type AppTheme = 'light' | 'dark' | 'system'

export interface AppConfig {
  schemaVersion: number
  createdAt: string
  updatedAt: string
  lastOpenedWorkspacePath: string | null
  theme: AppTheme
}

export interface UpdateAppConfigInput {
  lastOpenedWorkspacePath: string | null
  theme: AppTheme
}

export interface AppStartupState {
  appConfig: AppConfig
  workspacePath: string
  workspaceSnapshot: WorkspaceSnapshot
}

export function openStartupWorkspace() {
  return invoke<AppStartupState>('open_startup_workspace')
}

export function readAppConfig() {
  return invoke<AppConfig>('read_app_config')
}

export function updateAppConfig(input: UpdateAppConfigInput) {
  return invoke<AppConfig>('update_app_config', { input })
}
