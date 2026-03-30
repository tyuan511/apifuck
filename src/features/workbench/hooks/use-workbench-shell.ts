import { useMemo } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { useWorkbenchStore } from '../store/workbench-store'
import { collectCollectionIds, describeCollectionParent, findCollectionName } from '../utils'

export function useWorkbenchShell() {
  const state = useWorkbenchStore(useShallow(store => ({
    project: store.project,
    projectPath: store.projectPath,
    recentProjectPaths: store.recentProjectPaths,
    appTheme: store.appTheme,
    appPrimaryColor: store.appPrimaryColor,
    selectedTreeNode: store.selectedTreeNode,
    collapsedCollectionIds: store.collapsedCollectionIds,
    openRequestTabs: store.openRequestTabs,
    activeRequestId: store.activeRequestId,
    requestDrafts: store.requestDrafts,
    collectionDrafts: store.collectionDrafts,
    projectDrafts: store.projectDrafts,
    requestResponses: store.requestResponses,
    loadingRequestIds: store.loadingRequestIds,
    dirtyRequestIds: store.dirtyRequestIds,
    activeEditorTab: store.activeEditorTab,
    splitRatio: store.splitRatio,
    settingsDialogOpen: store.settingsDialogOpen,
    isBusy: store.isBusy,
    projectDialogOpen: store.projectDialogOpen,
    projectDialogMode: store.projectDialogMode,
    projectNameDraft: store.projectNameDraft,
    projectDescriptionDraft: store.projectDescriptionDraft,
    projectRequestConfigDraft: store.projectRequestConfigDraft,
    collectionDialogOpen: store.collectionDialogOpen,
    collectionDialogParentCollectionId: store.collectionDialogParentCollectionId,
    collectionNameDraft: store.collectionNameDraft,
    requestDialogOpen: store.requestDialogOpen,
    requestDialogParentCollectionId: store.requestDialogParentCollectionId,
    requestNameDraft: store.requestNameDraft,
    requestDescriptionDraft: store.requestDescriptionDraft,
    editRequestDialogOpen: store.editRequestDialogOpen,
    editRequestNameDraft: store.editRequestNameDraft,
    editRequestDescriptionDraft: store.editRequestDescriptionDraft,
    pendingCloseRequestId: store.pendingCloseRequestId,
    pendingCollectionDeletion: store.pendingCollectionDeletion,
    pendingEnvironmentDeletion: store.pendingEnvironmentDeletion,
    pendingRecentProjectRemoval: store.pendingRecentProjectRemoval,
    pendingRequestDeletion: store.pendingRequestDeletion,
    environments: store.environments,
    activeEnvironmentId: store.activeEnvironmentId,
    environmentDialogOpen: store.environmentDialogOpen,
    editingEnvironmentId: store.editingEnvironmentId,
    environmentNameDraft: store.environmentNameDraft,
    environmentBaseUrlDraft: store.environmentBaseUrlDraft,
    environmentVariablesDraft: store.environmentVariablesDraft,
  })))

  const actions = useWorkbenchStore(useShallow(store => ({
    setProjectNameDraft: store.setProjectNameDraft,
    setProjectDescriptionDraft: store.setProjectDescriptionDraft,
    setProjectRequestConfigDraft: store.setProjectRequestConfigDraft,
    setRecentProjectPaths: store.setRecentProjectPaths,
    setAppTheme: store.setAppTheme,
    setAppPrimaryColor: store.setAppPrimaryColor,
    openCreateProjectDialog: store.openCreateProjectDialog,
    openEditProjectDialog: store.openEditProjectDialog,
    closeCreateProjectDialog: store.closeCreateProjectDialog,
    handleCreateProject: store.handleCreateProject,
    handleEditProject: store.handleEditProject,
    updateProjectTabDraft: store.updateProjectTabDraft,
    handleOpenExistingProject: store.handleOpenExistingProject,
    requestRemoveRecentProject: store.requestRemoveRecentProject,
    clearPendingRecentProjectRemoval: store.clearPendingRecentProjectRemoval,
    setDeleteLocalFilesForPendingRecentProjectRemoval: store.setDeleteLocalFilesForPendingRecentProjectRemoval,
    handleRemoveRecentProject: store.handleRemoveRecentProject,
    handleSelectProject: store.handleSelectProject,
    openCreateCollectionDialog: store.openCreateCollectionDialog,
    closeCreateCollectionDialog: store.closeCreateCollectionDialog,
    setCollectionNameDraft: store.setCollectionNameDraft,
    handleCreateCollection: store.handleCreateCollection,
    openEditCollectionDialog: store.openEditCollectionDialog,
    updateCollectionTabDraft: store.updateCollectionTabDraft,
    handleEditCollection: store.handleEditCollection,
    openCreateRequestDialog: store.openCreateRequestDialog,
    closeCreateRequestDialog: store.closeCreateRequestDialog,
    setRequestNameDraft: store.setRequestNameDraft,
    setRequestDescriptionDraft: store.setRequestDescriptionDraft,
    handleCreateRequest: store.handleCreateRequest,
    openEditRequestDialog: store.openEditRequestDialog,
    closeEditRequestDialog: store.closeEditRequestDialog,
    setEditRequestNameDraft: store.setEditRequestNameDraft,
    setEditRequestDescriptionDraft: store.setEditRequestDescriptionDraft,
    handleEditRequest: store.handleEditRequest,
    requestDeleteRequest: store.requestDeleteRequest,
    clearPendingRequestDeletion: store.clearPendingRequestDeletion,
    handleDeleteRequest: store.handleDeleteRequest,
    requestDeleteCollection: store.requestDeleteCollection,
    clearPendingCollectionDeletion: store.clearPendingCollectionDeletion,
    handleDeleteCollection: store.handleDeleteCollection,
    moveTreeNode: store.moveTreeNode,
    toggleCollection: store.toggleCollection,
    setNodeSelection: store.setNodeSelection,
    openRequestFromSummary: store.openRequestFromSummary,
    focusRequestTab: store.focusRequestTab,
    reorderRequestTabs: store.reorderRequestTabs,
    openSettingsDialog: store.openSettingsDialog,
    closeSettingsDialog: store.closeSettingsDialog,
    setSettingsDialogOpen: store.setSettingsDialogOpen,
    requestCloseRequestTab: store.requestCloseRequestTab,
    clearPendingCloseRequest: store.clearPendingCloseRequest,
    confirmCloseRequestTab: store.confirmCloseRequestTab,
    setActiveEditorTab: store.setActiveEditorTab,
    updateRequestDraft: store.updateRequestDraft,
    handleSaveRequest: store.handleSaveRequest,
    handleSendRequest: store.handleSendRequest,
    setSplitRatio: store.setSplitRatio,
    openCreateEnvironmentDialog: store.openCreateEnvironmentDialog,
    openEditEnvironmentDialog: store.openEditEnvironmentDialog,
    closeEnvironmentDialog: store.closeEnvironmentDialog,
    setEnvironmentNameDraft: store.setEnvironmentNameDraft,
    setEnvironmentBaseUrlDraft: store.setEnvironmentBaseUrlDraft,
    setEnvironmentVariablesDraft: store.setEnvironmentVariablesDraft,
    handleCreateEnvironment: store.handleCreateEnvironment,
    handleEditEnvironment: store.handleEditEnvironment,
    requestDeleteEnvironment: store.requestDeleteEnvironment,
    clearPendingEnvironmentDeletion: store.clearPendingEnvironmentDeletion,
    handleDeleteEnvironment: store.handleDeleteEnvironment,
    handleSetActiveEnvironment: store.handleSetActiveEnvironment,
  })))

  const activeTabRecord = state.activeRequestId
    ? state.openRequestTabs.find(tab => tab.requestId === state.activeRequestId) ?? null
    : null
  const activeDraft = activeTabRecord?.entityType === 'request'
    ? state.requestDrafts[activeTabRecord.entityId] ?? null
    : null
  const activeCollectionDraft = activeTabRecord?.entityType === 'collection'
    ? state.collectionDrafts[activeTabRecord.entityId] ?? null
    : null
  const activeProjectDraft = activeTabRecord?.entityType === 'project'
    ? state.projectDrafts[activeTabRecord.entityId] ?? null
    : null
  const activeResponse = activeTabRecord?.entityType === 'request'
    ? state.requestResponses[activeTabRecord.entityId] ?? null
    : null
  const activeRequestIsLoading = Boolean(
    activeTabRecord?.entityType === 'request'
    && state.loadingRequestIds.includes(activeTabRecord.entityId)
    && !activeDraft,
  )

  const currentProjectCollectionNames = useMemo(() => new Map(
    (state.project ? collectCollectionIds(state.project.children) : [])
      .map(id => [id, findCollectionName(state.project?.children ?? [], id)]),
  ), [state.project])

  const collectionDialogParentLabel = describeCollectionParent(
    state.collectionDialogParentCollectionId,
    currentProjectCollectionNames,
  )
  const requestDialogParentLabel = describeCollectionParent(
    state.requestDialogParentCollectionId,
    currentProjectCollectionNames,
  )

  return {
    ...state,
    ...actions,
    activeDraft,
    activeCollectionDraft,
    activeProjectDraft,
    activeTabRecord,
    activeRequestIsLoading,
    activeResponse,
    collectionDialogParentLabel,
    requestDialogParentLabel,
  }
}
