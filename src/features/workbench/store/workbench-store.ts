import type { StateCreator } from 'zustand'
import type {
  EditorPanelTab,
  OpenRequestTab,
  PendingCollectionDeletion,
  PendingRequestDeletion,
  RequestEditorDraft,
  ResponseState,
  TreeSelection,
  WorkbenchBootPayload,
} from '../types'
import type {
  ApiDefinition,
  ApiSummary,
  CollectionTreeNode,
  CreateApiInput,
  WorkspaceSnapshot,
} from '@/lib/workspace'
import { startTransition } from 'react'
import { toast } from 'sonner'
import { create } from 'zustand'
import { updateTabState } from '@/lib/app-config'
import {
  createApi,
  createCollection,
  createDefaultDocumentation,
  createDefaultMock,
  createDefaultRequest,
  createProject,
  deleteNode,
  moveNode,
  openWorkspace,
  readApi,
  sendRequest,
  updateApi,
  updateCollection,
} from '@/lib/workspace'
import {
  areApiDefinitionsEqual,
  cloneApiDefinition,
  collectCollectionIds,
  collectCollectionSubtree,
  createEmptyResponseState,
  findApiLocation,
  findApiLocationInProject,
  findCollectionName,
  getActiveProject,
  mapSendResponseToState,
} from '../utils'

interface WorkbenchState {
  workspacePath: string
  workspace: WorkspaceSnapshot | null
  isBooting: boolean
  isBusy: boolean
  activeProjectId: string | null
  selectedTreeNode: TreeSelection | null
  collapsedCollectionIds: string[]
  openRequestTabs: OpenRequestTab[]
  activeRequestId: string | null
  requestDrafts: Record<string, RequestEditorDraft>
  savedRequests: Record<string, ApiDefinition>
  dirtyRequestIds: Set<string>
  loadingRequestIds: string[]
  requestResponses: Record<string, ResponseState>
  activeEditorTab: EditorPanelTab
  splitRatio: number
  projectDialogOpen: boolean
  projectNameDraft: string
  projectDescriptionDraft: string
  collectionDialogOpen: boolean
  collectionDialogParentCollectionId: string | null
  collectionNameDraft: string
  editCollectionDialogOpen: boolean
  editingCollectionId: string | null
  editCollectionNameDraft: string
  editCollectionDescriptionDraft: string
  requestDialogOpen: boolean
  requestDialogParentCollectionId: string | null
  requestNameDraft: string
  requestDescriptionDraft: string
  editRequestDialogOpen: boolean
  editingRequestId: string | null
  editRequestNameDraft: string
  editRequestDescriptionDraft: string
  pendingCloseRequestId: string | null
  pendingRequestDeletion: PendingRequestDeletion | null
  pendingCollectionDeletion: PendingCollectionDeletion | null
}

interface WorkbenchActions {
  hydrateWorkspace: (payload: WorkbenchBootPayload) => void
  hydrateTabState: (tabs: OpenRequestTab[], activeRequestId: string | null) => void
  setIsBooting: (value: boolean) => void
  setSplitRatio: (value: number) => void
  setActiveEditorTab: (value: EditorPanelTab) => void
  ensureActiveProject: () => void
  ensureTreeSelection: () => void
  toggleCollection: (collectionId: string) => void
  setNodeSelection: (selection: TreeSelection | null) => void
  setActiveProject: (projectId: string | null) => void
  focusRequestTab: (requestId: string) => void
  reorderRequestTabs: (tabs: OpenRequestTab[]) => void
  openCreateProjectDialog: () => void
  closeCreateProjectDialog: () => void
  setProjectNameDraft: (value: string) => void
  setProjectDescriptionDraft: (value: string) => void
  handleCreateProject: () => Promise<void>
  openCreateCollectionDialog: (parentCollectionId: string | null) => void
  closeCreateCollectionDialog: () => void
  setCollectionNameDraft: (value: string) => void
  handleCreateCollection: () => Promise<void>
  openEditCollectionDialog: (node: CollectionTreeNode) => void
  closeEditCollectionDialog: () => void
  setEditCollectionNameDraft: (value: string) => void
  setEditCollectionDescriptionDraft: (value: string) => void
  handleEditCollection: () => Promise<void>
  openCreateRequestDialog: (parentCollectionId: string | null) => void
  closeCreateRequestDialog: () => void
  setRequestNameDraft: (value: string) => void
  setRequestDescriptionDraft: (value: string) => void
  handleCreateRequest: () => Promise<void>
  openEditRequestDialog: (summary: ApiSummary) => void
  closeEditRequestDialog: () => void
  setEditRequestNameDraft: (value: string) => void
  setEditRequestDescriptionDraft: (value: string) => void
  handleEditRequest: () => Promise<void>
  openRequestFromSummary: (summary: ApiSummary, parentCollectionId: string | null) => Promise<void>
  updateRequestDraft: (updater: (draft: RequestEditorDraft) => RequestEditorDraft) => void
  handleSaveRequest: () => Promise<void>
  handleSendRequest: () => Promise<void>
  requestCloseRequestTab: (requestId: string) => void
  closeRequestTab: (requestId: string) => void
  clearPendingCloseRequest: () => void
  confirmCloseRequestTab: () => void
  requestDeleteRequest: (summary: ApiSummary) => void
  clearPendingRequestDeletion: () => void
  handleDeleteRequest: () => Promise<void>
  requestDeleteCollection: (node: CollectionTreeNode) => void
  clearPendingCollectionDeletion: () => void
  handleDeleteCollection: () => Promise<void>
  moveTreeNode: (nodeId: string, targetParentCollectionId: string | null, position: number) => Promise<void>
  refreshWorkspaceInternal: () => Promise<WorkspaceSnapshot | null>
  setRequestLoaded: (definition: ApiDefinition, setActiveTab?: boolean) => void
  syncRequestState: (definition: ApiDefinition) => void
  syncTabState: () => void
  removeDeletedRequestStateInternal: (
    requestIds: string[],
    collectionIds: string[],
    nextWorkspace: WorkspaceSnapshot | null,
  ) => void
}

export type WorkbenchStore = WorkbenchState & WorkbenchActions

const initialState: WorkbenchState = {
  workspacePath: '',
  workspace: null,
  isBooting: true,
  isBusy: false,
  activeProjectId: null,
  selectedTreeNode: null,
  collapsedCollectionIds: [],
  openRequestTabs: [],
  activeRequestId: null,
  requestDrafts: {},
  savedRequests: {},
  dirtyRequestIds: new Set(),
  loadingRequestIds: [],
  requestResponses: {},
  activeEditorTab: 'query',
  splitRatio: 0.55,
  projectDialogOpen: false,
  projectNameDraft: '',
  projectDescriptionDraft: '',
  collectionDialogOpen: false,
  collectionDialogParentCollectionId: null,
  collectionNameDraft: '',
  editCollectionDialogOpen: false,
  editingCollectionId: null,
  editCollectionNameDraft: '',
  editCollectionDescriptionDraft: '',
  requestDialogOpen: false,
  requestDialogParentCollectionId: null,
  requestNameDraft: '',
  requestDescriptionDraft: '',
  editRequestDialogOpen: false,
  editingRequestId: null,
  editRequestNameDraft: '',
  editRequestDescriptionDraft: '',
  pendingCloseRequestId: null,
  pendingRequestDeletion: null,
  pendingCollectionDeletion: null,
}

type WorkbenchSetState = (
  partial: Partial<WorkbenchStore> | ((state: WorkbenchStore) => Partial<WorkbenchStore>),
) => void

async function runTask<T>(
  set: WorkbenchSetState,
  task: () => Promise<T>,
  successMessage?: string,
) {
  set({ isBusy: true })
  try {
    const result = await task()
    if (successMessage) {
      toast.success(successMessage)
    }
    return result
  }
  catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    toast.error(message)
    throw error
  }
  finally {
    set({ isBusy: false })
  }
}

const createWorkbenchStore: StateCreator<WorkbenchStore> = (set, get) => ({
  ...initialState,

  hydrateWorkspace(payload) {
    startTransition(() => {
      set({
        workspacePath: payload.workspacePath,
        workspace: payload.workspaceSnapshot,
        activeProjectId: payload.workspaceSnapshot.lastOpenedProjectId,
        selectedTreeNode: null,
        openRequestTabs: [],
        activeRequestId: null,
        requestDrafts: {},
        savedRequests: {},
        dirtyRequestIds: new Set(),
        requestResponses: {},
        loadingRequestIds: [],
        projectDialogOpen: false,
        projectNameDraft: '',
        projectDescriptionDraft: '',
        collectionDialogOpen: false,
        collectionDialogParentCollectionId: null,
        collectionNameDraft: '',
        editCollectionDialogOpen: false,
        editingCollectionId: null,
        editCollectionNameDraft: '',
        editCollectionDescriptionDraft: '',
        requestDialogOpen: false,
        requestDialogParentCollectionId: null,
        requestNameDraft: '',
        requestDescriptionDraft: '',
        editRequestDialogOpen: false,
        editingRequestId: null,
        editRequestNameDraft: '',
        editRequestDescriptionDraft: '',
        pendingCloseRequestId: null,
        pendingRequestDeletion: null,
        pendingCollectionDeletion: null,
      })
    })
  },

  hydrateTabState(tabs, activeRequestId) {
    set({
      openRequestTabs: tabs,
      activeRequestId,
    })
  },

  setIsBooting(value) {
    set({ isBooting: value })
  },

  setSplitRatio(value) {
    set({ splitRatio: value })
  },

  setActiveEditorTab(value) {
    set({ activeEditorTab: value })
  },

  ensureActiveProject() {
    const { workspace, activeProjectId } = get()
    if (!workspace) {
      return
    }

    const activeExists = activeProjectId && workspace.projects.some(project => project.metadata.id === activeProjectId)
    if (!activeExists) {
      set({ activeProjectId: workspace.lastOpenedProjectId || workspace.defaultProjectId })
    }
  },

  ensureTreeSelection() {
    const { workspace, activeProjectId, selectedTreeNode } = get()
    const activeProject = getActiveProject(workspace, activeProjectId)
    if (!activeProject || !selectedTreeNode) {
      return
    }

    if (selectedTreeNode.type === 'collection') {
      const collectionIds = collectCollectionIds(activeProject.children)
      if (!collectionIds.includes(selectedTreeNode.id)) {
        set({ selectedTreeNode: null })
      }
      return
    }

    const location = findApiLocationInProject(activeProject.children, selectedTreeNode.id)
    if (!location) {
      set({ selectedTreeNode: null })
    }
  },

  toggleCollection(collectionId) {
    set((state) => {
      if (state.collapsedCollectionIds.includes(collectionId)) {
        return {
          collapsedCollectionIds: state.collapsedCollectionIds.filter(id => id !== collectionId),
        }
      }

      return {
        collapsedCollectionIds: [...state.collapsedCollectionIds, collectionId],
      }
    })
  },

  setNodeSelection(selection) {
    set({ selectedTreeNode: selection })
  },

  setActiveProject(projectId) {
    set({
      activeProjectId: projectId,
      selectedTreeNode: null,
    })
  },

  focusRequestTab(requestId) {
    const { workspace } = get()

    set(state => ({
      openRequestTabs: state.openRequestTabs.map(tab =>
        tab.requestId === requestId ? { ...tab, lastFocusedAt: Date.now() } : tab,
      ),
      activeRequestId: requestId,
    }))

    const location = workspace ? findApiLocation(workspace, requestId) : null
    if (location) {
      set({
        activeProjectId: location.projectId,
        selectedTreeNode: {
          type: 'api',
          id: requestId,
          parentCollectionId: location.parentCollectionId,
        },
      })
    }

    get().syncTabState()
  },

  reorderRequestTabs(tabs) {
    set(state => ({
      activeRequestId: tabs.some(tab => tab.requestId === state.activeRequestId)
        ? state.activeRequestId
        : tabs.at(0)?.requestId ?? null,
      openRequestTabs: tabs,
    }))

    get().syncTabState()
  },

  openCreateProjectDialog() {
    set({
      projectDialogOpen: true,
      projectNameDraft: '',
      projectDescriptionDraft: '',
    })
  },

  closeCreateProjectDialog() {
    set({
      projectDialogOpen: false,
      projectNameDraft: '',
      projectDescriptionDraft: '',
    })
  },

  setProjectNameDraft(value) {
    set({ projectNameDraft: value })
  },

  setProjectDescriptionDraft(value) {
    set({ projectDescriptionDraft: value })
  },

  async handleCreateProject() {
    const { projectNameDraft, projectDescriptionDraft } = get()
    const name = projectNameDraft.trim()
    if (!name) {
      toast.error('项目名称不能为空')
      return
    }

    await runTask(set, async () => {
      const project = await createProject({
        name,
        description: projectDescriptionDraft.trim(),
      })
      const snapshot = await get().refreshWorkspaceInternal()
      if (snapshot) {
        set({ activeProjectId: project.id })
      }
      get().closeCreateProjectDialog()
    }, '项目已创建')
  },

  openCreateCollectionDialog(parentCollectionId) {
    set({
      collectionDialogOpen: true,
      collectionDialogParentCollectionId: parentCollectionId,
      collectionNameDraft: '',
    })
  },

  closeCreateCollectionDialog() {
    set({
      collectionDialogOpen: false,
      collectionDialogParentCollectionId: null,
      collectionNameDraft: '',
    })
  },

  setCollectionNameDraft(value) {
    set({ collectionNameDraft: value })
  },

  async handleCreateCollection() {
    const {
      activeProjectId,
      collectionDialogParentCollectionId,
      collectionNameDraft,
      workspace,
    } = get()
    const activeProject = getActiveProject(workspace, activeProjectId)
    const name = collectionNameDraft.trim()

    if (!name || !activeProject) {
      toast.error('请先选择项目并填写集合名称')
      return
    }

    const parentCollectionId = collectionDialogParentCollectionId
    await runTask(set, async () => {
      const collection = await createCollection({
        projectId: activeProject.metadata.id,
        parentCollectionId: parentCollectionId ?? undefined,
        name,
        description: '',
      })
      await get().refreshWorkspaceInternal()
      set(state => ({
        selectedTreeNode: {
          type: 'collection',
          id: collection.id,
          parentCollectionId,
        },
        collapsedCollectionIds: state.collapsedCollectionIds.filter(
          id => id !== collection.id && id !== parentCollectionId,
        ),
      }))
      get().closeCreateCollectionDialog()
    }, '集合已创建')
  },

  openEditCollectionDialog(node) {
    set({
      editCollectionDialogOpen: true,
      editingCollectionId: node.id,
      editCollectionNameDraft: node.name,
      editCollectionDescriptionDraft: node.description,
    })
  },

  closeEditCollectionDialog() {
    set({
      editCollectionDialogOpen: false,
      editingCollectionId: null,
      editCollectionNameDraft: '',
      editCollectionDescriptionDraft: '',
    })
  },

  setEditCollectionNameDraft(value) {
    set({ editCollectionNameDraft: value })
  },

  setEditCollectionDescriptionDraft(value) {
    set({ editCollectionDescriptionDraft: value })
  },

  async handleEditCollection() {
    const {
      editCollectionDescriptionDraft,
      editCollectionNameDraft,
      editingCollectionId,
    } = get()
    const name = editCollectionNameDraft.trim()
    if (!editingCollectionId || !name) {
      toast.error('请填写目录名称')
      return
    }

    await runTask(set, async () => {
      await updateCollection({
        id: editingCollectionId,
        name,
        description: editCollectionDescriptionDraft.trim(),
      })
      await get().refreshWorkspaceInternal()
      get().closeEditCollectionDialog()
    }, '目录已更新')
  },

  openCreateRequestDialog(parentCollectionId) {
    set({
      requestDialogOpen: true,
      requestDialogParentCollectionId: parentCollectionId,
      requestNameDraft: '',
      requestDescriptionDraft: '',
    })
  },

  closeCreateRequestDialog() {
    set({
      requestDialogOpen: false,
      requestDialogParentCollectionId: null,
      requestNameDraft: '',
      requestDescriptionDraft: '',
    })
  },

  setRequestNameDraft(value) {
    set({ requestNameDraft: value })
  },

  setRequestDescriptionDraft(value) {
    set({ requestDescriptionDraft: value })
  },

  async handleCreateRequest() {
    const {
      activeProjectId,
      requestDialogParentCollectionId,
      requestNameDraft,
      requestDescriptionDraft,
      workspace,
    } = get()
    const activeProject = getActiveProject(workspace, activeProjectId)
    const name = requestNameDraft.trim()
    if (!name || !activeProject) {
      toast.error('请填写请求名称')
      return
    }

    const parentCollectionId = requestDialogParentCollectionId
    const input: CreateApiInput = {
      projectId: activeProject.metadata.id,
      parentCollectionId: parentCollectionId ?? undefined,
      name,
      method: 'GET',
      url: '',
      description: requestDescriptionDraft.trim(),
      tags: [],
      request: createDefaultRequest(),
      documentation: createDefaultDocumentation(
        parentCollectionId ? findCollectionName(activeProject.children, parentCollectionId) : '',
      ),
      mock: createDefaultMock(),
    }

    await runTask(set, async () => {
      const definition = await createApi(input)
      await get().refreshWorkspaceInternal()
      get().setRequestLoaded(definition)
      set({
        selectedTreeNode: {
          type: 'api',
          id: definition.id,
          parentCollectionId,
        },
      })
      get().closeCreateRequestDialog()
    }, '请求已创建')
  },

  openEditRequestDialog(summary) {
    const { requestDrafts, savedRequests } = get()
    const requestSource = requestDrafts[summary.id] ?? savedRequests[summary.id] ?? summary
    set({
      editingRequestId: summary.id,
      editRequestNameDraft: requestSource.name,
      editRequestDescriptionDraft: requestSource.description,
      editRequestDialogOpen: true,
    })
  },

  closeEditRequestDialog() {
    set({
      editRequestDialogOpen: false,
      editingRequestId: null,
      editRequestNameDraft: '',
      editRequestDescriptionDraft: '',
    })
  },

  setEditRequestNameDraft(value) {
    set({ editRequestNameDraft: value })
  },

  setEditRequestDescriptionDraft(value) {
    set({ editRequestDescriptionDraft: value })
  },

  async handleEditRequest() {
    const {
      editingRequestId,
      editRequestNameDraft,
      editRequestDescriptionDraft,
      requestDrafts,
      savedRequests,
    } = get()
    const name = editRequestNameDraft.trim()
    if (!editingRequestId || !name) {
      toast.error('请填写请求名称')
      return
    }

    const requestSource = cloneApiDefinition(
      requestDrafts[editingRequestId]
      ?? savedRequests[editingRequestId]
      ?? await readApi(editingRequestId),
    )

    await runTask(set, async () => {
      const saved = await updateApi({
        id: requestSource.id,
        name,
        method: requestSource.method,
        url: requestSource.url,
        description: editRequestDescriptionDraft.trim(),
        tags: requestSource.tags,
        request: requestSource.request,
        documentation: requestSource.documentation,
        mock: requestSource.mock,
      })
      get().syncRequestState(saved)
      await get().refreshWorkspaceInternal()
      get().closeEditRequestDialog()
    }, '请求已更新')
  },

  async openRequestFromSummary(summary, parentCollectionId) {
    const { workspace, requestDrafts } = get()
    const projectLocation = workspace ? findApiLocation(workspace, summary.id) : null
    if (projectLocation) {
      set({ activeProjectId: projectLocation.projectId })
    }

    set({
      selectedTreeNode: { type: 'api', id: summary.id, parentCollectionId },
    })

    if (requestDrafts[summary.id]) {
      get().focusRequestTab(summary.id)
      return
    }

    set(state => ({
      loadingRequestIds: state.loadingRequestIds.includes(summary.id)
        ? state.loadingRequestIds
        : [...state.loadingRequestIds, summary.id],
    }))

    try {
      const definition = await readApi(summary.id)
      get().setRequestLoaded(definition)
    }
    catch (error) {
      toast.error(error instanceof Error ? error.message : String(error))
    }
    finally {
      set(state => ({
        loadingRequestIds: state.loadingRequestIds.filter(id => id !== summary.id),
      }))
    }
  },

  updateRequestDraft(updater) {
    const { activeRequestId, requestDrafts, savedRequests } = get()
    if (!activeRequestId) {
      return
    }

    const currentDraft = requestDrafts[activeRequestId]
    if (!currentDraft) {
      return
    }

    const nextDraft = updater(cloneApiDefinition(currentDraft))
    const saved = savedRequests[activeRequestId]
    const dirty = saved ? !areApiDefinitionsEqual(saved, nextDraft) : true

    set((state) => {
      const nextDirtyIds = new Set(state.dirtyRequestIds)
      if (dirty) {
        nextDirtyIds.add(activeRequestId)
      }
      else {
        nextDirtyIds.delete(activeRequestId)
      }

      return {
        requestDrafts: { ...state.requestDrafts, [activeRequestId]: nextDraft },
        dirtyRequestIds: nextDirtyIds,
        openRequestTabs: state.openRequestTabs.map(tab =>
          tab.requestId === activeRequestId
            ? {
                ...tab,
                title: nextDraft.name,
                method: nextDraft.method,
                dirty,
              }
            : tab,
        ),
      }
    })
  },

  async handleSaveRequest() {
    const { activeRequestId, requestDrafts } = get()
    if (!activeRequestId || !requestDrafts[activeRequestId]) {
      return
    }

    const draft = cloneApiDefinition(requestDrafts[activeRequestId])
    await runTask(set, async () => {
      const saved = await updateApi({
        id: draft.id,
        name: draft.name,
        method: draft.method,
        url: draft.url,
        description: draft.description,
        tags: draft.tags,
        request: draft.request,
        documentation: draft.documentation,
        mock: draft.mock,
      })
      get().setRequestLoaded(saved)
      await get().refreshWorkspaceInternal()
    }, '请求已保存')
  },

  async handleSendRequest() {
    const { activeRequestId, requestDrafts } = get()
    if (!activeRequestId || !requestDrafts[activeRequestId]) {
      return
    }

    const draft = cloneApiDefinition(requestDrafts[activeRequestId])
    set(state => ({
      requestResponses: {
        ...state.requestResponses,
        [activeRequestId]: {
          ...(state.requestResponses[activeRequestId] ?? createEmptyResponseState()),
          isLoading: true,
          error: null,
        },
      },
    }))

    try {
      const response = await sendRequest({
        method: draft.method,
        url: draft.url,
        request: draft.request,
      })
      set(state => ({
        requestResponses: {
          ...state.requestResponses,
          [activeRequestId]: mapSendResponseToState(response),
        },
      }))
    }
    catch (error) {
      set(state => ({
        requestResponses: {
          ...state.requestResponses,
          [activeRequestId]: {
            ...(state.requestResponses[activeRequestId] ?? createEmptyResponseState()),
            isLoading: false,
            error: error instanceof Error ? error.message : String(error),
          },
        },
      }))
      toast.error(error instanceof Error ? error.message : String(error))
    }
  },

  requestCloseRequestTab(requestId) {
    if (get().dirtyRequestIds.has(requestId)) {
      set({ pendingCloseRequestId: requestId })
      return
    }

    get().closeRequestTab(requestId)
  },

  closeRequestTab(requestId) {
    const { workspace, selectedTreeNode } = get()

    set((state) => {
      const remaining = state.openRequestTabs.filter(tab => tab.requestId !== requestId)
      const nextActive = remaining.slice().sort((left, right) => right.lastFocusedAt - left.lastFocusedAt)[0]

      const nextState: Partial<WorkbenchStore> = {
        openRequestTabs: remaining,
        activeRequestId: nextActive?.requestId ?? null,
        requestDrafts: Object.fromEntries(
          Object.entries(state.requestDrafts).filter(([id]) => id !== requestId),
        ),
        savedRequests: Object.fromEntries(
          Object.entries(state.savedRequests).filter(([id]) => id !== requestId),
        ),
        requestResponses: Object.fromEntries(
          Object.entries(state.requestResponses).filter(([id]) => id !== requestId),
        ),
        dirtyRequestIds: new Set([...state.dirtyRequestIds].filter(id => id !== requestId)),
      }

      if (nextActive && workspace) {
        const location = findApiLocation(workspace, nextActive.requestId)
        if (location) {
          nextState.activeProjectId = location.projectId
          nextState.selectedTreeNode = {
            type: 'api',
            id: nextActive.requestId,
            parentCollectionId: location.parentCollectionId,
          }
        }
      }
      else if (!nextActive && selectedTreeNode?.type === 'api' && selectedTreeNode.id === requestId) {
        nextState.selectedTreeNode = null
      }

      return nextState
    })

    get().syncTabState()
  },

  clearPendingCloseRequest() {
    set({ pendingCloseRequestId: null })
  },

  confirmCloseRequestTab() {
    const requestId = get().pendingCloseRequestId
    if (!requestId) {
      return
    }

    get().closeRequestTab(requestId)
    set({ pendingCloseRequestId: null })
  },

  requestDeleteRequest(summary) {
    set({
      pendingRequestDeletion: {
        id: summary.id,
        name: summary.name,
      },
    })
  },

  clearPendingRequestDeletion() {
    set({ pendingRequestDeletion: null })
  },

  async handleDeleteRequest() {
    const deletion = get().pendingRequestDeletion
    if (!deletion) {
      return
    }

    await runTask(set, async () => {
      await deleteNode(deletion.id)
      const nextWorkspace = await get().refreshWorkspaceInternal()
      get().removeDeletedRequestStateInternal([deletion.id], [], nextWorkspace)
      set({ pendingRequestDeletion: null })
      get().syncTabState()
    }, '请求已删除')
  },

  requestDeleteCollection(node) {
    const subtree = collectCollectionSubtree(node)
    set({
      pendingCollectionDeletion: {
        id: node.id,
        name: node.name,
        apiIds: subtree.apiIds,
        collectionIds: subtree.collectionIds,
      },
    })
  },

  clearPendingCollectionDeletion() {
    set({ pendingCollectionDeletion: null })
  },

  async handleDeleteCollection() {
    const deletion = get().pendingCollectionDeletion
    if (!deletion) {
      return
    }

    await runTask(set, async () => {
      await deleteNode(deletion.id)
      const nextWorkspace = await get().refreshWorkspaceInternal()
      get().removeDeletedRequestStateInternal(deletion.apiIds, deletion.collectionIds, nextWorkspace)
      set(state => ({
        collapsedCollectionIds: state.collapsedCollectionIds.filter(id => !deletion.collectionIds.includes(id)),
        pendingCollectionDeletion: null,
      }))
      get().syncTabState()
    }, '目录已删除')
  },

  async moveTreeNode(nodeId, targetParentCollectionId, position) {
    const { activeProjectId, workspace } = get()
    const activeProject = getActiveProject(workspace, activeProjectId)

    if (!activeProject) {
      toast.error('请先选择项目')
      return
    }

    await runTask(set, async () => {
      await moveNode({
        nodeId,
        targetProjectId: activeProject.metadata.id,
        targetCollectionId: targetParentCollectionId ?? undefined,
        position,
      })

      await get().refreshWorkspaceInternal()

      set(state => ({
        collapsedCollectionIds: targetParentCollectionId
          ? state.collapsedCollectionIds.filter(id => id !== targetParentCollectionId)
          : state.collapsedCollectionIds,
        selectedTreeNode: state.selectedTreeNode && state.selectedTreeNode.id === nodeId
          ? { ...state.selectedTreeNode, parentCollectionId: targetParentCollectionId }
          : state.selectedTreeNode,
      }))
    })
  },

  async refreshWorkspaceInternal() {
    const { workspacePath } = get()
    if (!workspacePath) {
      return null
    }

    const snapshot = await openWorkspace(workspacePath)
    set({ workspace: snapshot })
    return snapshot
  },

  setRequestLoaded(definition, setActiveTab = true) {
    const draft = cloneApiDefinition(definition)
    const saved = cloneApiDefinition(definition)

    set((state) => {
      const nextTab: OpenRequestTab = {
        requestId: definition.id,
        title: definition.name,
        method: definition.method,
        dirty: false,
        lastFocusedAt: Date.now(),
      }
      const existing = state.openRequestTabs.some(tab => tab.requestId === definition.id)
      const nextDirtyIds = new Set(state.dirtyRequestIds)
      nextDirtyIds.delete(definition.id)

      return {
        requestDrafts: { ...state.requestDrafts, [definition.id]: draft },
        savedRequests: { ...state.savedRequests, [definition.id]: saved },
        requestResponses: state.requestResponses[definition.id]
          ? state.requestResponses
          : { ...state.requestResponses, [definition.id]: createEmptyResponseState() },
        dirtyRequestIds: nextDirtyIds,
        openRequestTabs: existing
          ? state.openRequestTabs.map(tab => (tab.requestId === definition.id ? nextTab : tab))
          : [...state.openRequestTabs, nextTab],
        activeRequestId: setActiveTab ? definition.id : state.activeRequestId,
      }
    })

    get().syncTabState()
  },

  syncRequestState(definition) {
    const draft = cloneApiDefinition(definition)
    const saved = cloneApiDefinition(definition)

    set((state) => {
      const nextDirtyIds = new Set(state.dirtyRequestIds)
      nextDirtyIds.delete(definition.id)

      return {
        requestDrafts: state.requestDrafts[definition.id]
          ? { ...state.requestDrafts, [definition.id]: draft }
          : state.requestDrafts,
        savedRequests: { ...state.savedRequests, [definition.id]: saved },
        dirtyRequestIds: nextDirtyIds,
        openRequestTabs: state.openRequestTabs.map(tab =>
          tab.requestId === definition.id
            ? {
                ...tab,
                title: definition.name,
                method: definition.method,
                dirty: false,
              }
            : tab,
        ),
      }
    })
  },

  removeDeletedRequestStateInternal(requestIds, collectionIds, nextWorkspace) {
    const removedRequestIds = new Set(requestIds)
    const removedCollectionIds = new Set(collectionIds)
    const { activeRequestId, editingRequestId, selectedTreeNode } = get()

    set((state) => {
      const remaining = state.openRequestTabs.filter(tab => !removedRequestIds.has(tab.requestId))
      const activeRemoved = activeRequestId ? removedRequestIds.has(activeRequestId) : false
      const nextActive = activeRemoved
        ? remaining.slice().sort((left, right) => right.lastFocusedAt - left.lastFocusedAt)[0]
        : remaining.find(tab => tab.requestId === activeRequestId) ?? null

      const nextState: Partial<WorkbenchStore> = {
        openRequestTabs: remaining,
        requestDrafts: Object.fromEntries(
          Object.entries(state.requestDrafts).filter(([id]) => !removedRequestIds.has(id)),
        ),
        savedRequests: Object.fromEntries(
          Object.entries(state.savedRequests).filter(([id]) => !removedRequestIds.has(id)),
        ),
        requestResponses: Object.fromEntries(
          Object.entries(state.requestResponses).filter(([id]) => !removedRequestIds.has(id)),
        ),
        loadingRequestIds: state.loadingRequestIds.filter(requestId => !removedRequestIds.has(requestId)),
        dirtyRequestIds: new Set([...state.dirtyRequestIds].filter(id => !removedRequestIds.has(id))),
        pendingCloseRequestId: state.pendingCloseRequestId && removedRequestIds.has(state.pendingCloseRequestId)
          ? null
          : state.pendingCloseRequestId,
        pendingRequestDeletion: state.pendingRequestDeletion && removedRequestIds.has(state.pendingRequestDeletion.id)
          ? null
          : state.pendingRequestDeletion,
      }

      if (state.editingCollectionId && removedCollectionIds.has(state.editingCollectionId)) {
        nextState.editCollectionDialogOpen = false
        nextState.editingCollectionId = null
        nextState.editCollectionNameDraft = ''
        nextState.editCollectionDescriptionDraft = ''
      }

      if (editingRequestId && removedRequestIds.has(editingRequestId)) {
        nextState.editRequestDialogOpen = false
        nextState.editingRequestId = null
        nextState.editRequestNameDraft = ''
        nextState.editRequestDescriptionDraft = ''
      }

      if (activeRemoved) {
        nextState.activeRequestId = nextActive?.requestId ?? null
      }

      if (nextActive && activeRemoved && nextWorkspace) {
        const location = findApiLocation(nextWorkspace, nextActive.requestId)
        if (location) {
          nextState.activeProjectId = location.projectId
          nextState.selectedTreeNode = {
            type: 'api',
            id: nextActive.requestId,
            parentCollectionId: location.parentCollectionId,
          }
        }
      }
      else if (
        selectedTreeNode
        && (
          (selectedTreeNode.type === 'api' && removedRequestIds.has(selectedTreeNode.id))
          || (selectedTreeNode.type === 'collection' && removedCollectionIds.has(selectedTreeNode.id))
        )
      ) {
        nextState.selectedTreeNode = null
      }

      return nextState
    })
  },

  syncTabState() {
    const { openRequestTabs, activeRequestId } = get()
    updateTabState({
      openRequestTabs,
      activeRequestId,
    }).catch((error) => {
      console.error('Failed to sync tab state:', error)
    })
  },
})

export const useWorkbenchStore = create<WorkbenchStore>()(createWorkbenchStore)
