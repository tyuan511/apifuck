import { useMemo } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { useWorkbenchStore } from '../store/workbench-store'
import { collectCollectionIds, describeCollectionParent, findCollectionName, getActiveProject } from '../utils'

export function useWorkbenchShell() {
  const state = useWorkbenchStore(useShallow(store => ({
    workspace: store.workspace,
    activeProjectId: store.activeProjectId,
    selectedTreeNode: store.selectedTreeNode,
    collapsedCollectionIds: store.collapsedCollectionIds,
    openRequestTabs: store.openRequestTabs,
    activeRequestId: store.activeRequestId,
    requestDrafts: store.requestDrafts,
    requestResponses: store.requestResponses,
    loadingRequestIds: store.loadingRequestIds,
    dirtyRequestIds: store.dirtyRequestIds,
    activeEditorTab: store.activeEditorTab,
    splitRatio: store.splitRatio,
    isDraggingSplit: store.isDraggingSplit,
    isBusy: store.isBusy,
    projectDialogOpen: store.projectDialogOpen,
    projectNameDraft: store.projectNameDraft,
    projectDescriptionDraft: store.projectDescriptionDraft,
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
    pendingRequestDeletion: store.pendingRequestDeletion,
  })))

  const actions = useWorkbenchStore(useShallow(store => ({
    setProjectNameDraft: store.setProjectNameDraft,
    setProjectDescriptionDraft: store.setProjectDescriptionDraft,
    openCreateProjectDialog: store.openCreateProjectDialog,
    closeCreateProjectDialog: store.closeCreateProjectDialog,
    handleCreateProject: store.handleCreateProject,
    openCreateCollectionDialog: store.openCreateCollectionDialog,
    closeCreateCollectionDialog: store.closeCreateCollectionDialog,
    setCollectionNameDraft: store.setCollectionNameDraft,
    handleCreateCollection: store.handleCreateCollection,
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
    toggleCollection: store.toggleCollection,
    setNodeSelection: store.setNodeSelection,
    setActiveProject: store.setActiveProject,
    openRequestFromSummary: store.openRequestFromSummary,
    focusRequestTab: store.focusRequestTab,
    requestCloseRequestTab: store.requestCloseRequestTab,
    clearPendingCloseRequest: store.clearPendingCloseRequest,
    confirmCloseRequestTab: store.confirmCloseRequestTab,
    setActiveEditorTab: store.setActiveEditorTab,
    updateRequestDraft: store.updateRequestDraft,
    handleSaveRequest: store.handleSaveRequest,
    handleSendRequest: store.handleSendRequest,
    setIsDraggingSplit: store.setIsDraggingSplit,
  })))

  const activeProject = getActiveProject(state.workspace, state.activeProjectId)
  const activeDraft = state.activeRequestId ? state.requestDrafts[state.activeRequestId] ?? null : null
  const activeResponse = state.activeRequestId ? state.requestResponses[state.activeRequestId] ?? null : null
  const activeRequestIsLoading = Boolean(
    state.activeRequestId && state.loadingRequestIds.includes(state.activeRequestId) && !activeDraft,
  )

  const currentProjectCollectionNames = useMemo(() => new Map(
    (activeProject ? collectCollectionIds(activeProject.children) : [])
      .map(id => [id, findCollectionName(activeProject?.children ?? [], id)]),
  ), [activeProject])

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
    activeProject,
    activeRequestIsLoading,
    activeResponse,
    collectionDialogParentLabel,
    requestDialogParentLabel,
  }
}
