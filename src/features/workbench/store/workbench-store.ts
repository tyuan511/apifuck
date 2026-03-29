import type { StateCreator } from 'zustand'
import type {
  EditorPanelTab,
  OpenRequestTab,
  PendingCollectionDeletion,
  PendingEnvironmentDeletion,
  PendingRecentProjectRemoval,
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
  Environment,
  KeyValue,
  ProjectSnapshot,
} from '@/lib/project'
import type { RequestConfig, ResponseData } from '@/lib/script-runner'
import { open } from '@tauri-apps/plugin-dialog'
import { startTransition } from 'react'
import { toast } from 'sonner'
import { create } from 'zustand'
import { readAppConfig, updateAppConfig, updateTabState } from '@/lib/app-config'
import {
  bootstrapProject,
  createApi,
  createCollection,
  createDefaultDocumentation,
  createDefaultMock,
  createDefaultRequest,
  createEnvironment,
  deleteEnvironment,
  deleteNode,
  deleteProjectFiles,
  moveNode,
  openProject,
  readApi,
  sendRequest,
  setActiveEnvironment,
  updateApi,
  updateCollection,
  updateEnvironment,
} from '@/lib/project'
import {
  createPostRequestContext,
  createPreRequestContext,
  runScript,
} from '@/lib/script-runner'
import {
  areApiDefinitionsEqual,
  cloneApiDefinition,
  collectCollectionIds,
  collectCollectionSubtree,
  createEmptyResponseState,
  findApiLocation,
  findApiLocationInProject,
  findCollectionName,
} from '../utils'

function generateId(): string {
  return crypto.randomUUID()
}

interface WorkbenchState {
  projectPath: string
  project: ProjectSnapshot | null
  recentProjectPaths: string[]
  isBooting: boolean
  isBusy: boolean
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
  pendingEnvironmentDeletion: PendingEnvironmentDeletion | null
  pendingRecentProjectRemoval: PendingRecentProjectRemoval | null
  environments: Environment[]
  activeEnvironmentId: string | null
  environmentDialogOpen: boolean
  editingEnvironmentId: string | null
  environmentNameDraft: string
  environmentBaseUrlDraft: string
  environmentVariablesDraft: KeyValue[]
}

interface WorkbenchActions {
  hydrateProject: (payload: WorkbenchBootPayload) => void
  hydrateTabState: (tabs: OpenRequestTab[], activeRequestId: string | null) => void
  setIsBooting: (value: boolean) => void
  setSplitRatio: (value: number) => void
  setActiveEditorTab: (value: EditorPanelTab) => void
  ensureTreeSelection: () => void
  toggleCollection: (collectionId: string) => void
  setNodeSelection: (selection: TreeSelection | null) => void
  focusRequestTab: (requestId: string) => void
  reorderRequestTabs: (tabs: OpenRequestTab[]) => void
  openCreateProjectDialog: () => void
  closeCreateProjectDialog: () => void
  setProjectNameDraft: (value: string) => void
  setProjectDescriptionDraft: (value: string) => void
  setRecentProjectPaths: (paths: string[]) => void
  handleCreateProject: () => Promise<void>
  handleOpenExistingProject: () => Promise<void>
  requestRemoveRecentProject: (path: string, name: string) => void
  clearPendingRecentProjectRemoval: () => void
  setDeleteLocalFilesForPendingRecentProjectRemoval: (value: boolean) => void
  handleRemoveRecentProject: () => Promise<void>
  handleSelectProject: (path: string) => Promise<void>
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
  refreshProjectInternal: () => Promise<ProjectSnapshot | null>
  setRequestLoaded: (definition: ApiDefinition, setActiveTab?: boolean) => void
  syncRequestState: (definition: ApiDefinition) => void
  syncTabState: () => void
  removeDeletedRequestStateInternal: (
    requestIds: string[],
    collectionIds: string[],
    nextProject: ProjectSnapshot | null,
  ) => void
  openCreateEnvironmentDialog: () => void
  openEditEnvironmentDialog: (environment: Environment) => void
  closeEnvironmentDialog: () => void
  setEnvironmentNameDraft: (value: string) => void
  setEnvironmentBaseUrlDraft: (value: string) => void
  setEnvironmentVariablesDraft: (variables: KeyValue[]) => void
  addEnvironmentVariable: () => void
  handleCreateEnvironment: () => Promise<void>
  handleEditEnvironment: () => Promise<void>
  requestDeleteEnvironment: (environmentId: string) => void
  clearPendingEnvironmentDeletion: () => void
  handleDeleteEnvironment: () => Promise<void>
  handleSetActiveEnvironment: (environmentId: string | null) => Promise<void>
  resolveEnvironmentVariables: (text: string, variables: KeyValue[]) => string
  resolveUrl: (url: string, baseUrl: string | undefined) => string
}

export type WorkbenchStore = WorkbenchState & WorkbenchActions

const initialState: WorkbenchState = {
  projectPath: '',
  project: null,
  recentProjectPaths: [],
  isBooting: true,
  isBusy: false,
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
  pendingEnvironmentDeletion: null,
  pendingRecentProjectRemoval: null,
  environments: [],
  activeEnvironmentId: null,
  environmentDialogOpen: false,
  editingEnvironmentId: null,
  environmentNameDraft: '',
  environmentBaseUrlDraft: '',
  environmentVariablesDraft: [],
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

async function openProjectInStore(
  set: WorkbenchSetState,
  get: () => WorkbenchStore,
  path: string,
  successMessage: string,
) {
  await runTask(set, async () => {
    const snapshot = await openProject(path)
    const appConfig = await readAppConfig()
    get().hydrateProject({
      projectPath: path,
      projectSnapshot: snapshot,
      recentProjectPaths: appConfig.recentProjectPaths,
    })
    get().syncTabState()
  }, successMessage)
}

const createWorkbenchStore: StateCreator<WorkbenchStore> = (set, get) => ({
  ...initialState,

  hydrateProject(payload) {
    startTransition(() => {
      set({
        projectPath: payload.projectPath,
        project: payload.projectSnapshot,
        recentProjectPaths: payload.recentProjectPaths,
        selectedTreeNode: null,
        openRequestTabs: [],
        activeRequestId: null,
        activeEditorTab: 'query',
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
        pendingEnvironmentDeletion: null,
        pendingRecentProjectRemoval: null,
        environments: payload.projectSnapshot.environments,
        activeEnvironmentId: payload.projectSnapshot.metadata.activeEnvironmentId ?? null,
      })
    })
  },

  hydrateTabState(tabs, activeRequestId) {
    const activeTab = tabs.find(tab => tab.requestId === activeRequestId)?.editorTab ?? 'query'
    set({
      openRequestTabs: tabs,
      activeRequestId,
      activeEditorTab: activeTab,
    })
  },

  setIsBooting(value) {
    set({ isBooting: value })
  },

  setSplitRatio(value) {
    set({ splitRatio: value })
  },

  setActiveEditorTab(value) {
    set((state) => {
      if (!state.activeRequestId) {
        return { activeEditorTab: value }
      }

      return {
        activeEditorTab: value,
        openRequestTabs: state.openRequestTabs.map(tab =>
          tab.requestId === state.activeRequestId ? { ...tab, editorTab: value } : tab,
        ),
      }
    })
  },

  ensureTreeSelection() {
    const { project, selectedTreeNode } = get()
    if (!project || !selectedTreeNode) {
      return
    }

    if (selectedTreeNode.type === 'collection') {
      const collectionIds = collectCollectionIds(project.children)
      if (!collectionIds.includes(selectedTreeNode.id)) {
        set({ selectedTreeNode: null })
      }
      return
    }

    const location = findApiLocationInProject(project.children, selectedTreeNode.id)
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

  focusRequestTab(requestId) {
    set((state) => {
      const nextActiveTab = state.openRequestTabs.find(tab => tab.requestId === requestId)?.editorTab ?? 'query'
      return {
        openRequestTabs: state.openRequestTabs.map(tab =>
          tab.requestId === requestId ? { ...tab, lastFocusedAt: Date.now() } : tab,
        ),
        activeRequestId: requestId,
        activeEditorTab: nextActiveTab,
      }
    })

    const location = get().project ? findApiLocation(get().project!, requestId) : null
    if (location) {
      set({
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
    set((state) => {
      const nextActiveRequestId = tabs.some(tab => tab.requestId === state.activeRequestId)
        ? state.activeRequestId
        : tabs.at(0)?.requestId ?? null
      const nextActiveTab = tabs.find(tab => tab.requestId === nextActiveRequestId)?.editorTab ?? 'query'

      return {
        activeRequestId: nextActiveRequestId,
        activeEditorTab: nextActiveTab,
        openRequestTabs: tabs,
      }
    })

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

  setRecentProjectPaths(paths) {
    set({ recentProjectPaths: paths })
  },

  async handleCreateProject() {
    const { projectNameDraft, projectDescriptionDraft } = get()
    const name = projectNameDraft.trim()
    if (!name) {
      toast.error('项目名称不能为空')
      return
    }

    try {
      const selectedPath = await open({
        directory: true,
        multiple: false,
        title: '选择新项目目录',
      })

      if (typeof selectedPath !== 'string') {
        return
      }

      await runTask(set, async () => {
        const snapshot = await bootstrapProject(selectedPath, {
          name,
          description: projectDescriptionDraft.trim(),
        })
        const appConfig = await readAppConfig()
        get().hydrateProject({
          projectPath: selectedPath,
          projectSnapshot: snapshot,
          recentProjectPaths: appConfig.recentProjectPaths,
        })
        get().closeCreateProjectDialog()
        get().syncTabState()
      }, '项目已创建')
    }
    catch (error) {
      toast.error(error instanceof Error ? error.message : String(error))
    }
  },

  async handleOpenExistingProject() {
    try {
      const selectedPath = await open({
        directory: true,
        multiple: false,
        title: '选择已有项目目录',
      })

      if (typeof selectedPath !== 'string') {
        return
      }

      await openProjectInStore(set, get, selectedPath, '项目已打开')
    }
    catch (error) {
      toast.error(error instanceof Error ? error.message : String(error))
    }
  },

  async handleSelectProject(path) {
    const nextPath = path.trim()
    if (!nextPath || nextPath === get().projectPath) {
      return
    }

    await openProjectInStore(set, get, nextPath, '项目已切换')
  },

  requestRemoveRecentProject(path, name) {
    set({
      pendingRecentProjectRemoval: {
        path,
        name,
        deleteLocalFiles: false,
      },
    })
  },

  clearPendingRecentProjectRemoval() {
    set({ pendingRecentProjectRemoval: null })
  },

  setDeleteLocalFilesForPendingRecentProjectRemoval(value) {
    set(state => ({
      pendingRecentProjectRemoval: state.pendingRecentProjectRemoval
        ? { ...state.pendingRecentProjectRemoval, deleteLocalFiles: value }
        : null,
    }))
  },

  async handleRemoveRecentProject() {
    const { pendingRecentProjectRemoval, projectPath, recentProjectPaths } = get()
    const targetPath = pendingRecentProjectRemoval?.path.trim() ?? ''

    if (!targetPath || targetPath === projectPath) {
      return
    }

    await runTask(set, async () => {
      if (pendingRecentProjectRemoval?.deleteLocalFiles) {
        await deleteProjectFiles(targetPath)
      }

      const appConfig = await readAppConfig()
      const updated = await updateAppConfig({
        lastOpenedProjectPath: appConfig.lastOpenedProjectPath,
        recentProjectPaths: recentProjectPaths.filter(item => item !== targetPath),
        theme: appConfig.theme,
      })
      set({
        pendingRecentProjectRemoval: null,
        recentProjectPaths: updated.recentProjectPaths,
      })
    }, '删除项目')
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
      collectionDialogParentCollectionId,
      collectionNameDraft,
      project,
    } = get()
    const name = collectionNameDraft.trim()

    if (!name || !project) {
      toast.error('请先选择项目并填写集合名称')
      return
    }

    const parentCollectionId = collectionDialogParentCollectionId
    await runTask(set, async () => {
      const collection = await createCollection({
        projectId: project.metadata.id,
        parentCollectionId: parentCollectionId ?? undefined,
        name,
        description: '',
      })
      await get().refreshProjectInternal()
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
      await get().refreshProjectInternal()
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
      requestDialogParentCollectionId,
      requestNameDraft,
      requestDescriptionDraft,
      project,
    } = get()
    const name = requestNameDraft.trim()
    if (!name || !project) {
      toast.error('请填写请求名称')
      return
    }

    const parentCollectionId = requestDialogParentCollectionId
    const input: CreateApiInput = {
      projectId: project.metadata.id,
      parentCollectionId: parentCollectionId ?? undefined,
      name,
      method: 'GET',
      url: '',
      description: requestDescriptionDraft.trim(),
      tags: [],
      request: createDefaultRequest(),
      documentation: createDefaultDocumentation(
        parentCollectionId ? findCollectionName(project.children, parentCollectionId) : '',
      ),
      mock: createDefaultMock(),
      preRequestScript: '',
      postRequestScript: '',
    }

    await runTask(set, async () => {
      const definition = await createApi(input)
      await get().refreshProjectInternal()
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
        preRequestScript: requestSource.preRequestScript,
        postRequestScript: requestSource.postRequestScript,
      })
      get().syncRequestState(saved)
      await get().refreshProjectInternal()
      get().closeEditRequestDialog()
    }, '请求已更新')
  },

  async openRequestFromSummary(summary, parentCollectionId) {
    const { requestDrafts } = get()

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
        preRequestScript: draft.preRequestScript,
        postRequestScript: draft.postRequestScript,
      })
      get().setRequestLoaded(saved)
      await get().refreshProjectInternal()
    }, '请求已保存')
  },

  async handleSendRequest() {
    const { activeRequestId, requestDrafts, project, environments, activeEnvironmentId, resolveUrl, resolveEnvironmentVariables } = get()
    if (!activeRequestId || !requestDrafts[activeRequestId]) {
      return
    }

    const draft = cloneApiDefinition(requestDrafts[activeRequestId])
    const activeProjectId = project?.metadata.id ?? null
    const activeEnv = activeEnvironmentId ? environments.find(e => e.id === activeEnvironmentId) ?? null : null

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

    // Script-level vars storage (persists across pre and post scripts)
    const scriptVars = new Map<string, unknown>()

    // Env mutations from pre-script
    const envMutations = new Map<string, string>()

    try {
      // Build mutable config from draft
      const mutableConfig: RequestConfig = {
        method: draft.method,
        url: draft.url,
        headers: draft.request.headers.map(h => ({ ...h })),
        query: draft.request.query.map(q => ({ ...q })),
        body: {
          mode: draft.request.body.mode,
          raw: draft.request.body.raw,
          json: draft.request.body.json,
          formData: draft.request.body.formData.map(item => ({ ...item })),
          urlEncoded: draft.request.body.urlEncoded.map(item => ({ ...item })),
        },
      }

      // Run pre-request script
      if (draft.preRequestScript.trim()) {
        const ctx = createPreRequestContext(
          mutableConfig,
          key => envMutations.get(key) ?? activeEnv?.variables.find(v => v.key === key && v.enabled)?.value,
          (key, value) => { envMutations.set(key, value) },
          key => scriptVars.get(key),
          (key, value) => { scriptVars.set(key, value) },
        )
        const result = runScript(draft.preRequestScript, ctx)
        if (result.error) {
          throw new Error(`Pre-request script error: ${result.error}`)
        }
      }

      const variables = activeEnv?.variables ?? []

      // Resolve URL with baseURL joining (use config modified by pre-script)
      const resolvedUrl = resolveUrl(mutableConfig.url, activeEnv?.baseUrl)

      // Resolve variables in headers
      const resolvedHeaders = mutableConfig.headers.map(h => ({
        ...h,
        value: resolveEnvironmentVariables(h.value, variables),
      }))

      // Resolve variables in query params
      const resolvedQuery = mutableConfig.query.map(q => ({
        ...q,
        key: resolveEnvironmentVariables(q.key, variables),
        value: resolveEnvironmentVariables(q.value, variables),
      }))

      // Resolve variables in URL
      const finalUrl = resolveEnvironmentVariables(resolvedUrl, variables)

      // Resolve variables in body
      const resolvedBody = {
        ...mutableConfig.body,
        raw: resolveEnvironmentVariables(mutableConfig.body.raw, variables),
        json: resolveEnvironmentVariables(mutableConfig.body.json, variables),
        formData: mutableConfig.body.formData.map(item => ({
          ...item,
          key: resolveEnvironmentVariables(item.key, variables),
          value: resolveEnvironmentVariables(item.value, variables),
        })),
        urlEncoded: mutableConfig.body.urlEncoded.map(item => ({
          ...item,
          key: resolveEnvironmentVariables(item.key, variables),
          value: resolveEnvironmentVariables(item.value, variables),
        })),
      }

      const resolvedAuth = {
        ...draft.request.auth,
        basic: {
          username: resolveEnvironmentVariables(draft.request.auth.basic.username, variables),
          password: resolveEnvironmentVariables(draft.request.auth.basic.password, variables),
        },
        bearerToken: resolveEnvironmentVariables(draft.request.auth.bearerToken, variables),
        apiKey: {
          ...draft.request.auth.apiKey,
          key: resolveEnvironmentVariables(draft.request.auth.apiKey.key, variables),
          value: resolveEnvironmentVariables(draft.request.auth.apiKey.value, variables),
        },
      }

      const response = await sendRequest({
        method: mutableConfig.method,
        url: finalUrl,
        request: {
          headers: resolvedHeaders,
          query: resolvedQuery,
          pathParams: draft.request.pathParams,
          cookies: draft.request.cookies,
          auth: resolvedAuth,
          body: {
            ...resolvedBody,
            binary: draft.request.body.binary,
            mode: resolvedBody.mode as 'none' | 'raw' | 'json' | 'form-data' | 'x-www-form-urlencoded' | 'binary',
          },
        },
      })

      // Build response data for post-script
      const responseData: ResponseData = {
        status: response.status,
        headers: Object.fromEntries(response.headers.map((h: { name: string, value: string }) => [h.name, h.value])),
        body: response.body,
        time: response.durationMs,
      }

      // Run post-request script
      let finalResponse = responseData
      if (draft.postRequestScript.trim()) {
        const postCtx = createPostRequestContext(
          mutableConfig,
          responseData,
          key => envMutations.get(key) ?? activeEnv?.variables.find(v => v.key === key && v.enabled)?.value,
          (key, value) => { envMutations.set(key, value) },
          key => scriptVars.get(key),
          (key, value) => { scriptVars.set(key, value) },
        )
        const postResult = runScript(draft.postRequestScript, postCtx)
        if (postResult.error) {
          throw new Error(`Post-request script error: ${postResult.error}`)
        }
        if (postResult.response) {
          finalResponse = postResult.response
        }
      }

      // Persist env mutations produced by fuck.env.set() back to the active environment.
      if (envMutations.size > 0 && activeEnv && activeProjectId) {
        const updatedVariables = activeEnv.variables.map(v =>
          envMutations.has(v.key) ? { ...v, value: String(envMutations.get(v.key)) } : v,
        )
        for (const [key, value] of envMutations) {
          if (!updatedVariables.some(v => v.key === key)) {
            updatedVariables.push({ id: generateId(), key, value: String(value), enabled: true, description: '' })
          }
        }
        await updateEnvironment({
          id: activeEnv.id,
          projectId: activeProjectId,
          name: activeEnv.name,
          baseUrl: activeEnv.baseUrl,
          variables: updatedVariables,
        })
        set(state => ({
          environments: state.environments.map(e =>
            e.id === activeEnv.id ? { ...e, variables: updatedVariables } : e,
          ),
        }))
      }

      set(state => ({
        requestResponses: {
          ...state.requestResponses,
          [activeRequestId]: {
            status: finalResponse.status,
            headers: Object.entries(finalResponse.headers).map(([name, value]) => ({ name, value, description: '' })),
            durationMs: finalResponse.time,
            sizeBytes: new Blob([finalResponse.body]).size,
            contentType: finalResponse.headers['content-type'] ?? '',
            responseType: finalResponse.body ? 'json' as const : null,
            body: finalResponse.body,
            isLoading: false,
            error: null,
          },
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
    const { project, selectedTreeNode } = get()

    set((state) => {
      const remaining = state.openRequestTabs.filter(tab => tab.requestId !== requestId)
      const nextActive = remaining.slice().sort((left, right) => right.lastFocusedAt - left.lastFocusedAt)[0]

      const nextState: Partial<WorkbenchStore> = {
        openRequestTabs: remaining,
        activeRequestId: nextActive?.requestId ?? null,
        activeEditorTab: nextActive?.editorTab ?? 'query',
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

      if (nextActive && project) {
        const location = findApiLocation(project, nextActive.requestId)
        if (location) {
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
      const nextProject = await get().refreshProjectInternal()
      get().removeDeletedRequestStateInternal([deletion.id], [], nextProject)
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
      const nextProject = await get().refreshProjectInternal()
      get().removeDeletedRequestStateInternal(deletion.apiIds, deletion.collectionIds, nextProject)
      set(state => ({
        collapsedCollectionIds: state.collapsedCollectionIds.filter(id => !deletion.collectionIds.includes(id)),
        pendingCollectionDeletion: null,
      }))
      get().syncTabState()
    }, '目录已删除')
  },

  async moveTreeNode(nodeId, targetParentCollectionId, position) {
    const { project } = get()

    if (!project) {
      toast.error('请先选择项目')
      return
    }

    await runTask(set, async () => {
      await moveNode({
        nodeId,
        targetProjectId: project.metadata.id,
        targetCollectionId: targetParentCollectionId ?? undefined,
        position,
      })

      await get().refreshProjectInternal()

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

  async refreshProjectInternal() {
    const { projectPath } = get()
    if (!projectPath) {
      return null
    }

    const snapshot = await openProject(projectPath)
    set({
      project: snapshot,
      environments: snapshot.environments,
      activeEnvironmentId: snapshot.metadata.activeEnvironmentId ?? null,
    })
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
        editorTab: state.openRequestTabs.find(tab => tab.requestId === definition.id)?.editorTab ?? 'query',
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
        activeEditorTab: setActiveTab ? nextTab.editorTab : state.activeEditorTab,
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

  removeDeletedRequestStateInternal(requestIds, collectionIds, nextProject) {
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
        activeEditorTab: nextActive?.editorTab ?? 'query',
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

      if (nextActive && activeRemoved && nextProject) {
        const location = findApiLocation(nextProject, nextActive.requestId)
        if (location) {
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

  openCreateEnvironmentDialog() {
    set({
      environmentDialogOpen: true,
      editingEnvironmentId: null,
      environmentNameDraft: '',
      environmentBaseUrlDraft: '',
      environmentVariablesDraft: [],
    })
  },

  openEditEnvironmentDialog(environment) {
    set({
      environmentDialogOpen: true,
      editingEnvironmentId: environment.id,
      environmentNameDraft: environment.name,
      environmentBaseUrlDraft: environment.baseUrl,
      environmentVariablesDraft: environment.variables.map(v => ({ ...v })),
    })
  },

  closeEnvironmentDialog() {
    set({
      environmentDialogOpen: false,
      editingEnvironmentId: null,
      environmentNameDraft: '',
      environmentBaseUrlDraft: '',
      environmentVariablesDraft: [],
    })
  },

  setEnvironmentNameDraft(value) {
    set({ environmentNameDraft: value })
  },

  setEnvironmentBaseUrlDraft(value) {
    set({ environmentBaseUrlDraft: value })
  },

  setEnvironmentVariablesDraft(variables) {
    set({ environmentVariablesDraft: variables })
  },

  addEnvironmentVariable() {
    set(state => ({
      environmentVariablesDraft: [
        ...state.environmentVariablesDraft,
        { id: generateId(), key: '', value: '', enabled: true, description: '' },
      ],
    }))
  },

  async handleCreateEnvironment() {
    const { project, environmentNameDraft, environmentBaseUrlDraft, environmentVariablesDraft } = get()
    const activeProjectId = project?.metadata.id ?? null
    if (!activeProjectId) {
      toast.error('请先选择项目')
      return
    }
    const name = environmentNameDraft.trim()
    if (!name) {
      toast.error('环境名称不能为空')
      return
    }

    await runTask(set, async () => {
      await createEnvironment({
        projectId: activeProjectId,
        name,
        baseUrl: environmentBaseUrlDraft.trim(),
        variables: environmentVariablesDraft,
      })
      const snapshot = await get().refreshProjectInternal()
      if (snapshot) {
        set({
          environments: snapshot.environments,
        })
      }
      get().closeEnvironmentDialog()
    }, '环境已创建')
  },

  async handleEditEnvironment() {
    const { project, editingEnvironmentId, environmentNameDraft, environmentBaseUrlDraft, environmentVariablesDraft } = get()
    const activeProjectId = project?.metadata.id ?? null
    if (!activeProjectId || !editingEnvironmentId) {
      return
    }
    const name = environmentNameDraft.trim()
    if (!name) {
      toast.error('环境名称不能为空')
      return
    }

    await runTask(set, async () => {
      await updateEnvironment({
        id: editingEnvironmentId,
        projectId: activeProjectId,
        name,
        baseUrl: environmentBaseUrlDraft.trim(),
        variables: environmentVariablesDraft,
      })
      const snapshot = await get().refreshProjectInternal()
      if (snapshot) {
        set({
          environments: snapshot.environments,
        })
      }
      get().closeEnvironmentDialog()
    }, '环境已更新')
  },

  requestDeleteEnvironment(environmentId) {
    const environment = get().environments.find(item => item.id === environmentId)
    if (!environment) {
      return
    }

    set(state => ({
      pendingEnvironmentDeletion: {
        id: environment.id,
        name: environment.name,
      },
      environmentDialogOpen: state.editingEnvironmentId === environmentId ? false : state.environmentDialogOpen,
      editingEnvironmentId: state.editingEnvironmentId === environmentId ? null : state.editingEnvironmentId,
      environmentNameDraft: state.editingEnvironmentId === environmentId ? '' : state.environmentNameDraft,
      environmentBaseUrlDraft: state.editingEnvironmentId === environmentId ? '' : state.environmentBaseUrlDraft,
      environmentVariablesDraft: state.editingEnvironmentId === environmentId ? [] : state.environmentVariablesDraft,
    }))
  },

  clearPendingEnvironmentDeletion() {
    set({ pendingEnvironmentDeletion: null })
  },

  async handleDeleteEnvironment() {
    const activeProjectId = get().project?.metadata.id ?? null
    const pendingEnvironmentDeletion = get().pendingEnvironmentDeletion
    if (!activeProjectId || !pendingEnvironmentDeletion) {
      return
    }

    await runTask(set, async () => {
      await deleteEnvironment({
        id: pendingEnvironmentDeletion.id,
        projectId: activeProjectId,
      })
      const snapshot = await get().refreshProjectInternal()
      if (snapshot) {
        set({
          environments: snapshot.environments,
          activeEnvironmentId: snapshot.metadata.activeEnvironmentId ?? null,
          pendingEnvironmentDeletion: null,
        })
      }
    }, '环境已删除')
  },

  async handleSetActiveEnvironment(environmentId) {
    const activeProjectId = get().project?.metadata.id ?? null
    if (!activeProjectId) {
      return
    }

    await runTask(set, async () => {
      const metadata = await setActiveEnvironment({
        projectId: activeProjectId,
        environmentId,
      })
      set({ activeEnvironmentId: metadata.activeEnvironmentId ?? null })
    })
  },

  resolveEnvironmentVariables(text, variables) {
    let result = text
    for (const variable of variables) {
      if (!variable.enabled)
        continue
      const pattern = `\${${variable.key}}`
      result = result.split(pattern).join(variable.value)
    }
    return result
  },

  resolveUrl(url, baseUrl) {
    if (!baseUrl) {
      return url
    }

    if (/^https?:\/\//.test(url)) {
      return url
    }
    const base = baseUrl.endsWith('/') ? baseUrl.slice(0, -1) : baseUrl
    const path = url.startsWith('/') ? url : `/${url}`
    return `${base}${path}`
  },
})

export const useWorkbenchStore = create<WorkbenchStore>()(createWorkbenchStore)
