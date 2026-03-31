import { useTheme } from 'next-themes'
import { useEffect } from 'react'
import * as React from 'react'
import { SettingsDialog } from '@/components/settings-dialog'
import { readAppConfig, updateAppConfig } from '@/lib/app-config'
import { primaryColorPalettes } from '@/lib/appearance'
import { readApi } from '@/lib/project'
import { useWorkbenchShell } from '../hooks/use-workbench-shell'
import { buildRequestExportSnippets } from '../request-export'
import { isMacOSDesktop } from '../types'
import {
  ConfirmDeleteCollectionDialog,
  ConfirmDeleteEnvironmentDialog,
  ConfirmDeleteRequestDialog,
  ConfirmRemoveRecentProjectDialog,
  CopyRequestDialog,
  CreateCollectionDialog,
  CreateProjectDialog,
  CreateRequestDialog,
  EditRequestDialog,
} from './dialogs'
import { ProjectSidebar } from './project-sidebar'
import { RequestPane } from './request-pane'

export function WorkbenchShell() {
  const shell = useWorkbenchShell()
  const { setTheme } = useTheme()
  const [copyDialogOpen, setCopyDialogOpen] = React.useState(false)
  const [copyRequestName, setCopyRequestName] = React.useState('')
  const [copyRequestSnippets, setCopyRequestSnippets] = React.useState<ReturnType<typeof buildRequestExportSnippets> | null>(null)

  useEffect(() => {
    const root = document.documentElement
    const palette = primaryColorPalettes[shell.appPrimaryColor]
    const lightPalette = palette.light
    const darkPalette = palette.dark

    root.style.setProperty('--primary', lightPalette.primary)
    root.style.setProperty('--primary-foreground', lightPalette.primaryForeground)
    root.style.setProperty('--ring', lightPalette.ring)
    root.style.setProperty('--sidebar-primary', lightPalette.sidebarPrimary)
    root.style.setProperty('--sidebar-primary-foreground', lightPalette.sidebarPrimaryForeground)
    root.style.setProperty('--sidebar-ring', lightPalette.ring)

    root.style.setProperty('--primary-dark', darkPalette.primary)
    root.style.setProperty('--primary-foreground-dark', darkPalette.primaryForeground)
    root.style.setProperty('--ring-dark', darkPalette.ring)
    root.style.setProperty('--sidebar-primary-dark', darkPalette.sidebarPrimary)
    root.style.setProperty('--sidebar-primary-foreground-dark', darkPalette.sidebarPrimaryForeground)
    root.style.setProperty('--sidebar-ring-dark', darkPalette.ring)
  }, [shell.appPrimaryColor])

  async function persistAppearance(nextValues: {
    primaryColor?: typeof shell.appPrimaryColor
    theme?: typeof shell.appTheme
  }) {
    const appConfig = await readAppConfig()
    const nextTheme = nextValues.theme ?? shell.appTheme
    const nextPrimaryColor = nextValues.primaryColor ?? shell.appPrimaryColor

    await updateAppConfig({
      lastOpenedProjectPath: appConfig.lastOpenedProjectPath,
      recentProjectPaths: appConfig.recentProjectPaths,
      theme: nextTheme,
      primaryColor: nextPrimaryColor,
    })

    if (nextValues.theme) {
      shell.setAppTheme(nextTheme)
    }

    if (nextValues.primaryColor) {
      shell.setAppPrimaryColor(nextPrimaryColor)
    }
  }

  async function handleThemeChange(theme: typeof shell.appTheme) {
    shell.setAppTheme(theme)
    setTheme(theme)

    try {
      await persistAppearance({ theme })
    }
    catch {
      // Errors are already surfaced via the shared task toasts elsewhere.
    }
  }

  async function handlePrimaryColorChange(primaryColor: typeof shell.appPrimaryColor) {
    shell.setAppPrimaryColor(primaryColor)

    try {
      await persistAppearance({ primaryColor })
    }
    catch {
      // Errors are already surfaced by the Tauri invoke promise chain.
    }
  }

  async function handleOpenCopyRequestDialog(requestId: string) {
    const request = shell.requestDrafts[requestId]
      ?? shell.savedRequests[requestId]
      ?? await readApi(requestId)

    if (request.protocol === 'websocket') {
      throw new Error('暂不支持导出 WebSocket 请求')
    }

    setCopyRequestName(request.name)
    setCopyRequestSnippets(buildRequestExportSnippets({
      activeEnvironmentId: shell.activeEnvironmentId,
      environments: shell.environments,
      project: shell.project,
      request,
    }))
    setCopyDialogOpen(true)
  }

  return (
    <main className="relative h-screen overflow-hidden bg-background text-foreground">
      <div
        className="grid h-full min-h-0"
        style={{ gridTemplateColumns: 'clamp(248px, 23vw, 308px) minmax(0, 1fr)' }}
      >
        <ProjectSidebar
          collapsedCollectionIds={shell.collapsedCollectionIds}
          environments={shell.environments}
          activeEnvironmentId={shell.activeEnvironmentId}
          isMacOSDesktop={isMacOSDesktop}
          openRequestTabs={shell.openRequestTabs}
          project={shell.project}
          projectPath={shell.projectPath}
          recentProjectPaths={shell.recentProjectPaths}
          selectedTreeNode={shell.selectedTreeNode}
          onCreateCollection={shell.openCreateCollectionDialog}
          onCreateEnvironment={shell.startCreateEnvironmentInTab}
          onCreateProject={shell.openCreateProjectDialog}
          onEditProject={shell.openEditProjectDialog}
          onCreateRequest={shell.openCreateRequestDialog}
          onDeleteCollection={shell.requestDeleteCollection}
          onDeleteEnvironment={shell.requestDeleteEnvironment}
          onDeleteRequest={shell.requestDeleteRequest}
          onCopyRequest={(requestId) => { void handleOpenCopyRequestDialog(requestId) }}
          onEditCollection={shell.openEditCollectionDialog}
          onEditRequest={shell.openEditRequestDialog}
          onOpenEnvironmentTab={shell.openEnvironmentTab}
          onMoveTreeNode={shell.moveTreeNode}
          onOpenExistingProject={() => { void shell.handleOpenExistingProject() }}
          onOpenSettings={shell.openSettingsDialog}
          onRemoveRecentProject={shell.requestRemoveRecentProject}
          onSelectProject={(path) => { void shell.handleSelectProject(path) }}
          onOpenRequest={(summary, parentCollectionId) => { void shell.openRequestFromSummary(summary, parentCollectionId) }}
          onToggleCollection={shell.toggleCollection}
          onTreeSelectionChange={shell.setNodeSelection}
        />

        <RequestPane
          activeDraft={shell.activeDraft}
          activeCollectionDraft={shell.activeCollectionDraft}
          activeEnvironmentDraft={shell.activeEnvironmentDraft}
          activeEnvironmentId={shell.activeEnvironmentId}
          activeProjectDraft={shell.activeProjectDraft}
          activeTabRecord={shell.activeTabRecord}
          activeEditorTab={shell.activeEditorTab}
          activeRequestId={shell.activeRequestId}
          activeRequestIsLoading={shell.activeRequestIsLoading}
          activeResponse={shell.activeResponse}
          activeWebSocketSession={shell.activeWebSocketSession}
          dirtyRequestIds={shell.dirtyRequestIds}
          environmentDrafts={shell.environmentDrafts}
          environments={shell.environments}
          isBusy={shell.isBusy}
          isMacOSDesktop={isMacOSDesktop}
          openRequestTabs={shell.openRequestTabs}
          pendingCloseRequestId={shell.pendingCloseRequestId}
          splitRatio={shell.splitRatio}
          onActiveEditorTabChange={shell.setActiveEditorTab}
          onChangeDraft={shell.updateRequestDraft}
          onChangeCollectionDraft={shell.updateCollectionTabDraft}
          onChangeEnvironmentDraft={shell.updateEnvironmentTabDraft}
          onChangeProjectDraft={shell.updateProjectTabDraft}
          onCloseRequestDialogChange={() => shell.clearPendingCloseRequest()}
          onCloseRequestTab={shell.requestCloseRequestTab}
          onConfirmCloseRequestTab={shell.confirmCloseRequestTab}
          onFocusRequestTab={shell.focusRequestTab}
          onReorderRequestTabs={shell.reorderRequestTabs}
          onSaveRequest={() => { void shell.handleSaveRequest() }}
          onSaveCollection={() => { void shell.handleEditCollection() }}
          onSaveEnvironment={() => { void shell.saveEnvironmentFromTab() }}
          onSaveProject={() => { void shell.handleEditProject() }}
          onSendRequest={() => { void shell.handleSendRequest() }}
          onConnectWebSocket={() => { void shell.handleConnectWebSocket() }}
          onDisconnectWebSocket={() => { void shell.handleDisconnectWebSocket() }}
          onSendWebSocketMessage={() => { void shell.handleSendWebSocketMessage() }}
          onWebSocketDraftMessageChange={shell.setWebSocketDraftMessage}
          onWebSocketMessageFormatChange={shell.setWebSocketMessageFormat}
          onSetActiveEnvironment={(environmentId) => { void shell.handleSetActiveEnvironment(environmentId) }}
          onSelectEnvironmentForTab={shell.selectEnvironmentForTab}
          onStartCreateEnvironment={shell.startCreateEnvironmentInTab}
          onDeleteEnvironment={shell.requestDeleteEnvironment}
          onSplitRatioChange={shell.setSplitRatio}
        />
      </div>

      <CreateProjectDialog
        description={shell.projectDescriptionDraft}
        name={shell.projectNameDraft}
        requestConfig={shell.projectRequestConfigDraft}
        open={shell.projectDialogOpen}
        onDescriptionChange={shell.setProjectDescriptionDraft}
        onNameChange={shell.setProjectNameDraft}
        onRequestConfigChange={shell.setProjectRequestConfigDraft}
        onOpenChange={(open) => {
          if (!open) {
            shell.closeCreateProjectDialog()
          }
        }}
        onSubmit={() => {
          if (shell.projectDialogMode === 'edit') {
            void shell.handleEditProject()
          }
          else {
            void shell.handleCreateProject()
          }
        }}
      />

      <CreateCollectionDialog
        name={shell.collectionNameDraft}
        open={shell.collectionDialogOpen}
        parentLabel={shell.collectionDialogParentLabel}
        onNameChange={shell.setCollectionNameDraft}
        onOpenChange={(open) => {
          if (!open) {
            shell.closeCreateCollectionDialog()
          }
        }}
        onSubmit={() => { void shell.handleCreateCollection() }}
      />

      <CreateRequestDialog
        description={shell.requestDescriptionDraft}
        name={shell.requestNameDraft}
        open={shell.requestDialogOpen}
        parentLabel={shell.requestDialogParentLabel}
        protocol={shell.requestProtocolDraft}
        onDescriptionChange={shell.setRequestDescriptionDraft}
        onNameChange={shell.setRequestNameDraft}
        onProtocolChange={shell.setRequestProtocolDraft}
        onOpenChange={(open) => {
          if (!open) {
            shell.closeCreateRequestDialog()
          }
        }}
        onSubmit={() => { void shell.handleCreateRequest() }}
      />

      <EditRequestDialog
        description={shell.editRequestDescriptionDraft}
        name={shell.editRequestNameDraft}
        open={shell.editRequestDialogOpen}
        onDescriptionChange={shell.setEditRequestDescriptionDraft}
        onNameChange={shell.setEditRequestNameDraft}
        onOpenChange={(open) => {
          if (!open) {
            shell.closeEditRequestDialog()
          }
        }}
        onSubmit={() => { void shell.handleEditRequest() }}
      />

      <CopyRequestDialog
        open={copyDialogOpen}
        requestName={copyRequestName}
        snippets={copyRequestSnippets}
        onOpenChange={setCopyDialogOpen}
      />

      <ConfirmDeleteCollectionDialog
        deletion={shell.pendingCollectionDeletion}
        onConfirm={() => { void shell.handleDeleteCollection() }}
        onOpenChange={open => !open && shell.clearPendingCollectionDeletion()}
      />

      <ConfirmDeleteRequestDialog
        deletion={shell.pendingRequestDeletion}
        onConfirm={() => { void shell.handleDeleteRequest() }}
        onOpenChange={open => !open && shell.clearPendingRequestDeletion()}
      />

      <ConfirmDeleteEnvironmentDialog
        deletion={shell.pendingEnvironmentDeletion}
        onConfirm={() => { void shell.handleDeleteEnvironment() }}
        onOpenChange={open => !open && shell.clearPendingEnvironmentDeletion()}
      />

      <ConfirmRemoveRecentProjectDialog
        removal={shell.pendingRecentProjectRemoval}
        onDeleteLocalFilesChange={shell.setDeleteLocalFilesForPendingRecentProjectRemoval}
        onConfirm={() => { void shell.handleRemoveRecentProject() }}
        onOpenChange={open => !open && shell.clearPendingRecentProjectRemoval()}
      />

      <SettingsDialog
        currentPrimaryColor={shell.appPrimaryColor}
        currentTheme={shell.appTheme}
        open={shell.settingsDialogOpen}
        onOpenChange={shell.setSettingsDialogOpen}
        onPrimaryColorChange={(value) => { void handlePrimaryColorChange(value) }}
        onThemeChange={(value) => { void handleThemeChange(value) }}
      />
    </main>
  )
}
