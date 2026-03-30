import type { StateCreator } from 'zustand'
import type {
  CollectionEditorDraft,
  EditorPanelTab,
  OpenRequestTab,
  PendingCollectionDeletion,
  PendingEnvironmentDeletion,
  PendingRecentProjectRemoval,
  PendingRequestDeletion,
  ProjectEditorDraft,
  RequestEditorDraft,
  ResponseState,
  SettingsPanelTab,
  TreeSelection,
  WorkbenchBootPayload,
  WorkbenchPanelTab,
} from '../types'
import type { AppPrimaryColor, AppTheme } from '@/lib/app-config'
import type {
  ApiDefinition,
  ApiSummary,
  CollectionTreeNode,
  CreateApiInput,
  Environment,
  KeyValue,
  ProjectSnapshot,
  RequestScopeConfig,
  SendRequestStreamEvent,
} from '@/lib/project'
import type { RequestConfig, ResponseData } from '@/lib/script-runner'
import { open } from '@tauri-apps/plugin-dialog'
import { startTransition } from 'react'
import { toast } from 'sonner'
import { create } from 'zustand'
import { readAppConfig, updateAppConfig, updateTabState } from '@/lib/app-config'
import { bootstrapProject, createApi, createCollection, createDefaultDocumentation, createDefaultMock, createDefaultRequest, createDefaultRequestScopeConfig, createEnvironment, deleteEnvironment, deleteNode, deleteProjectFiles, moveNode, openProject, readApi, sendRequest, setActiveEnvironment, updateApi, updateCollection, updateEnvironment, updateProject } from '@/lib/project'
import {
  createPostRequestContext,
  createPreRequestContext,
  runScript,
} from '@/lib/script-runner'
import {
  areApiDefinitionsEqual,
  cloneApiDefinition,
  cloneRequestScopeConfig,
  collectCollectionIds,
  collectCollectionSubtree,
  createEmptyResponseState,
  findApiLocation,
  findApiLocationInProject,
  findCollectionById,
  findCollectionName,
  findCollectionPath,
  mergeKeyValueEntries,
  resolveInheritedAuth,
} from '../utils'

function generateId(): string {
  return crypto.randomUUID()
}

interface WorkbenchState {
  projectPath: string
  project: ProjectSnapshot | null
  recentProjectPaths: string[]
  appTheme: AppTheme
  appPrimaryColor: AppPrimaryColor
  isBooting: boolean
  isBusy: boolean
  selectedTreeNode: TreeSelection | null
  collapsedCollectionIds: string[]
  openRequestTabs: OpenRequestTab[]
  activeRequestId: string | null
  requestDrafts: Record<string, RequestEditorDraft>
  collectionDrafts: Record<string, CollectionEditorDraft>
  projectDrafts: Record<string, ProjectEditorDraft>
  savedRequests: Record<string, ApiDefinition>
  savedCollections: Record<string, CollectionEditorDraft>
  savedProjects: Record<string, ProjectEditorDraft>
  dirtyRequestIds: Set<string>
  loadingRequestIds: string[]
  requestResponses: Record<string, ResponseState>
  activeEditorTab: WorkbenchPanelTab
  splitRatio: number
  settingsDialogOpen: boolean
  projectDialogOpen: boolean
  projectDialogMode: 'create' | 'edit'
  projectNameDraft: string
  projectDescriptionDraft: string
  projectRequestConfigDraft: RequestScopeConfig
  collectionDialogOpen: boolean
  collectionDialogParentCollectionId: string | null
  collectionNameDraft: string
  editCollectionDialogOpen: boolean
  editingCollectionId: string | null
  editCollectionNameDraft: string
  editCollectionDescriptionDraft: string
  editCollectionRequestConfigDraft: RequestScopeConfig
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
  setActiveEditorTab: (value: WorkbenchPanelTab) => void
  ensureTreeSelection: () => void
  toggleCollection: (collectionId: string) => void
  setNodeSelection: (selection: TreeSelection | null) => void
  focusRequestTab: (requestId: string) => void
  reorderRequestTabs: (tabs: OpenRequestTab[]) => void
  openSettingsDialog: () => void
  closeSettingsDialog: () => void
  setSettingsDialogOpen: (open: boolean) => void
  openCreateProjectDialog: () => void
  openEditProjectDialog: () => void
  closeCreateProjectDialog: () => void
  setProjectNameDraft: (value: string) => void
  setProjectDescriptionDraft: (value: string) => void
  setProjectRequestConfigDraft: (value: RequestScopeConfig) => void
  setRecentProjectPaths: (paths: string[]) => void
  setAppTheme: (theme: AppTheme) => void
  setAppPrimaryColor: (color: AppPrimaryColor) => void
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
  openCollectionTab: (node: CollectionTreeNode, parentCollectionId: string | null) => void
  openEditCollectionDialog: (node: CollectionTreeNode) => void
  closeEditCollectionDialog: () => void
  setEditCollectionNameDraft: (value: string) => void
  setEditCollectionDescriptionDraft: (value: string) => void
  setEditCollectionRequestConfigDraft: (value: RequestScopeConfig) => void
  updateCollectionTabDraft: (updater: (draft: CollectionEditorDraft) => CollectionEditorDraft) => void
  handleEditCollection: () => Promise<void>
  handleEditProject: () => Promise<void>
  openProjectTab: () => void
  updateProjectTabDraft: (updater: (draft: ProjectEditorDraft) => ProjectEditorDraft) => void
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
  appTheme: 'system',
  appPrimaryColor: 'slate',
  isBooting: true,
  isBusy: false,
  selectedTreeNode: null,
  collapsedCollectionIds: [],
  openRequestTabs: [],
  activeRequestId: null,
  requestDrafts: {},
  collectionDrafts: {},
  projectDrafts: {},
  savedRequests: {},
  savedCollections: {},
  savedProjects: {},
  dirtyRequestIds: new Set(),
  loadingRequestIds: [],
  requestResponses: {},
  activeEditorTab: 'query',
  splitRatio: 0.55,
  settingsDialogOpen: false,
  projectDialogOpen: false,
  projectDialogMode: 'create',
  projectNameDraft: '',
  projectDescriptionDraft: '',
  projectRequestConfigDraft: createDefaultRequestScopeConfig(),
  collectionDialogOpen: false,
  collectionDialogParentCollectionId: null,
  collectionNameDraft: '',
  editCollectionDialogOpen: false,
  editingCollectionId: null,
  editCollectionNameDraft: '',
  editCollectionDescriptionDraft: '',
  editCollectionRequestConfigDraft: createDefaultRequestScopeConfig(),
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
      appTheme: appConfig.theme,
      appPrimaryColor: appConfig.primaryColor,
    })
    get().syncTabState()
  }, successMessage)
}

function getCollectionRequestConfig(
  project: ProjectSnapshot | null,
  collectionId: string | null,
): RequestScopeConfig[] {
  if (!project || !collectionId) {
    return []
  }

  return findCollectionPath(project.children, collectionId)
    .map(collection => cloneRequestScopeConfig(collection.requestConfig ?? createDefaultRequestScopeConfig()))
}

function getRequestScopeChain(
  project: ProjectSnapshot | null,
  parentCollectionId: string | null,
  request: ApiDefinition,
) {
  const projectConfig = cloneRequestScopeConfig(project?.metadata.requestConfig ?? createDefaultRequestScopeConfig())
  const collectionConfigs = getCollectionRequestConfig(project, parentCollectionId)
  const resolvedBaseUrl = [
    projectConfig.baseUrl,
    ...collectionConfigs.map(config => config.baseUrl),
  ].find(baseUrl => baseUrl.trim()) ?? ''

  return {
    projectConfig,
    collectionConfigs,
    resolvedBaseUrl,
    mergedHeaders: mergeKeyValueEntries(
      projectConfig.headers,
      ...collectionConfigs.map(config => config.headers),
      request.request.headers,
    ),
    mergedAuth: resolveInheritedAuth(
      projectConfig.auth,
      ...collectionConfigs.map(config => config.auth),
      request.request.auth,
    ),
    preRequestScripts: [
      projectConfig.preRequestScript,
      ...collectionConfigs.map(config => config.preRequestScript),
      request.preRequestScript,
    ].filter(script => script.trim()),
    postRequestScripts: [
      projectConfig.postRequestScript,
      ...collectionConfigs.map(config => config.postRequestScript),
      request.postRequestScript,
    ].filter(script => script.trim()),
  }
}

function createProjectEditorDraft(project: ProjectSnapshot): ProjectEditorDraft {
  return JSON.parse(JSON.stringify(project.metadata)) as ProjectEditorDraft
}

function createCollectionEditorDraft(collection: CollectionTreeNode): CollectionEditorDraft {
  return JSON.parse(JSON.stringify({
    schemaVersion: 1,
    entityType: 'collection',
    id: collection.id,
    createdAt: collection.createdAt,
    updatedAt: collection.updatedAt,
    slug: collection.slug,
    name: collection.name,
    description: collection.description,
    order: collection.children.map(child => child.id),
    requestConfig: collection.requestConfig,
  })) as CollectionEditorDraft
}

function getActiveRequestEntityId(state: Pick<WorkbenchStore, 'activeRequestId' | 'openRequestTabs'>): string | null {
  if (!state.activeRequestId) {
    return null
  }
  const tab = state.openRequestTabs.find(item => item.requestId === state.activeRequestId)
  return tab?.entityType === 'request' ? tab.entityId : null
}

function getDefaultTabEditorTab(entityType: OpenRequestTab['entityType']): WorkbenchPanelTab {
  return entityType === 'request' ? 'query' : 'info'
}

function buildTreeSelectionForTab(project: ProjectSnapshot | null, tab: OpenRequestTab | null): TreeSelection | null {
  if (!project || !tab) {
    return null
  }

  if (tab.entityType === 'request') {
    const location = findApiLocation(project, tab.entityId)
    if (!location) {
      return null
    }

    return {
      type: 'api',
      id: tab.entityId,
      parentCollectionId: location.parentCollectionId,
    }
  }

  if (tab.entityType === 'collection') {
    const path = findCollectionPath(project.children, tab.entityId)
    if (path.length === 0) {
      return null
    }

    return {
      type: 'collection',
      id: tab.entityId,
      parentCollectionId: path.length > 1 ? path.at(-2)?.id ?? null : null,
    }
  }

  return null
}

function isTabRemoved(
  tab: OpenRequestTab,
  removedRequestIds: Set<string>,
  removedCollectionIds: Set<string>,
) {
  if (tab.entityType === 'request') {
    return removedRequestIds.has(tab.entityId)
  }

  if (tab.entityType === 'collection') {
    return removedCollectionIds.has(tab.entityId)
  }

  return false
}

function findResponseEntityIdByRequestId(
  requestResponses: Record<string, ResponseState>,
  requestId: string,
) {
  return Object.entries(requestResponses).find(([, response]) => response.requestId === requestId)?.[0] ?? null
}

function applySendRequestStreamEvent(
  set: WorkbenchSetState,
  get: () => WorkbenchStore,
  event: SendRequestStreamEvent,
) {
  logSseDebug(event.requestId, `channel:received kind=${event.kind}`)
  const entityId = findResponseEntityIdByRequestId(get().requestResponses, event.requestId)
  if (!entityId) {
    logSseDebug(event.requestId, 'channel:dropped missing-entity')
    return
  }

  set((state) => {
    const current = state.requestResponses[entityId]
    if (!current || current.requestId !== event.requestId) {
      logSseDebug(event.requestId, 'store:dropped stale-request')
      return {}
    }

    if (event.kind === 'started') {
      logSseDebug(event.requestId, 'store:apply started')
      return {
        requestResponses: {
          ...state.requestResponses,
          [entityId]: {
            ...current,
            status: event.status,
            headers: event.headers,
            contentType: event.contentType,
            responseType: 'eventstream',
            body: '',
            durationMs: 0,
            sizeBytes: 0,
            error: null,
            isLoading: true,
          },
        },
      }
    }

    if (event.kind === 'chunk') {
      logSseDebug(
        event.requestId,
        `store:apply chunk chunkLen=${event.chunk.length} nextBodyLen=${current.body.length + event.chunk.length}`,
      )
      return {
        requestResponses: {
          ...state.requestResponses,
          [entityId]: {
            ...current,
            body: `${current.body}${event.chunk}`,
            sizeBytes: event.receivedBytes,
            responseType: 'eventstream',
            isLoading: true,
          },
        },
      }
    }

    logSseDebug(event.requestId, 'store:apply finished')
    return {
      requestResponses: {
        ...state.requestResponses,
        [entityId]: {
          ...current,
          durationMs: event.durationMs,
          sizeBytes: event.sizeBytes,
          responseType: event.responseType,
          body: event.body,
          isLoading: true,
        },
      },
    }
  })
}

function logSseDebug(requestId: string, message: string) {
  if (!import.meta.env.DEV || !requestId.trim()) {
    return
  }

  console.warn(`[sse-debug][${Date.now()}][${requestId}] ${message}`)
}

const createWorkbenchStore: StateCreator<WorkbenchStore> = (set, get) => ({
  ...initialState,

  hydrateProject(payload) {
    startTransition(() => {
      set({
        projectPath: payload.projectPath,
        project: payload.projectSnapshot,
        recentProjectPaths: payload.recentProjectPaths,
        appTheme: payload.appTheme,
        appPrimaryColor: payload.appPrimaryColor,
        selectedTreeNode: null,
        openRequestTabs: [],
        activeRequestId: null,
        activeEditorTab: 'query',
        requestDrafts: {},
        collectionDrafts: {},
        projectDrafts: {},
        savedRequests: {},
        savedCollections: {},
        savedProjects: {},
        dirtyRequestIds: new Set(),
        requestResponses: {},
        loadingRequestIds: [],
        settingsDialogOpen: false,
        projectDialogOpen: false,
        projectDialogMode: 'create',
        projectNameDraft: '',
        projectDescriptionDraft: '',
        projectRequestConfigDraft: cloneRequestScopeConfig(payload.projectSnapshot.metadata.requestConfig ?? createDefaultRequestScopeConfig()),
        collectionDialogOpen: false,
        collectionDialogParentCollectionId: null,
        collectionNameDraft: '',
        editCollectionDialogOpen: false,
        editingCollectionId: null,
        editCollectionNameDraft: '',
        editCollectionDescriptionDraft: '',
        editCollectionRequestConfigDraft: createDefaultRequestScopeConfig(),
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
    const project = get().project
    const collectionDrafts: Record<string, CollectionEditorDraft> = {}
    const savedCollections: Record<string, CollectionEditorDraft> = {}
    const projectDrafts: Record<string, ProjectEditorDraft> = {}
    const savedProjects: Record<string, ProjectEditorDraft> = {}

    for (const tab of tabs) {
      if (tab.entityType === 'project' && project?.metadata.id === tab.entityId) {
        const draft = createProjectEditorDraft(project)
        projectDrafts[tab.entityId] = draft
        savedProjects[tab.entityId] = draft
      }

      if (tab.entityType === 'collection' && project) {
        const collection = findCollectionById(project.children, tab.entityId)
        if (!collection) {
          continue
        }

        const draft = createCollectionEditorDraft(collection)
        collectionDrafts[tab.entityId] = draft
        savedCollections[tab.entityId] = draft
      }
    }

    const activeRecord = tabs.find(tab => tab.requestId === activeRequestId) ?? null
    const activeTab = activeRecord?.editorTab ?? getDefaultTabEditorTab(activeRecord?.entityType ?? 'request')
    set({
      openRequestTabs: tabs,
      activeRequestId,
      activeEditorTab: activeTab,
      collectionDrafts,
      savedCollections,
      projectDrafts,
      savedProjects,
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
      const activeRecord = state.openRequestTabs.find(tab => tab.requestId === requestId) ?? null
      const nextActiveTab = activeRecord?.editorTab ?? getDefaultTabEditorTab(activeRecord?.entityType ?? 'request')
      return {
        openRequestTabs: state.openRequestTabs.map(tab =>
          tab.requestId === requestId ? { ...tab, lastFocusedAt: Date.now() } : tab,
        ),
        activeRequestId: requestId,
        activeEditorTab: nextActiveTab,
      }
    })

    const state = get()
    const activeTab = state.openRequestTabs.find(tab => tab.requestId === requestId) ?? null
    const nextSelection = buildTreeSelectionForTab(state.project, activeTab)
    if (nextSelection || activeTab?.entityType === 'project') {
      set({ selectedTreeNode: nextSelection })
    }

    get().syncTabState()
  },

  reorderRequestTabs(tabs) {
    set((state) => {
      const nextActiveRequestId = tabs.some(tab => tab.requestId === state.activeRequestId)
        ? state.activeRequestId
        : tabs.at(0)?.requestId ?? null
      const activeRecord = tabs.find(tab => tab.requestId === nextActiveRequestId) ?? null
      const nextActiveTab = activeRecord?.editorTab ?? getDefaultTabEditorTab(activeRecord?.entityType ?? 'request')

      return {
        activeRequestId: nextActiveRequestId,
        activeEditorTab: nextActiveTab,
        openRequestTabs: tabs,
      }
    })

    get().syncTabState()
  },

  openSettingsDialog() {
    set({ settingsDialogOpen: true })
  },

  closeSettingsDialog() {
    set({ settingsDialogOpen: false })
  },

  setSettingsDialogOpen(open) {
    set({ settingsDialogOpen: open })
  },

  openCreateProjectDialog() {
    set({
      projectDialogOpen: true,
      projectDialogMode: 'create',
      projectNameDraft: '',
      projectDescriptionDraft: '',
      projectRequestConfigDraft: createDefaultRequestScopeConfig(),
    })
  },

  openEditProjectDialog() {
    get().openProjectTab()
  },

  closeCreateProjectDialog() {
    set({
      projectDialogOpen: false,
      projectDialogMode: 'create',
      projectNameDraft: '',
      projectDescriptionDraft: '',
      projectRequestConfigDraft: createDefaultRequestScopeConfig(),
    })
  },

  setProjectNameDraft(value) {
    set({ projectNameDraft: value })
  },

  setProjectDescriptionDraft(value) {
    set({ projectDescriptionDraft: value })
  },

  setProjectRequestConfigDraft(value) {
    set({ projectRequestConfigDraft: value })
  },

  setRecentProjectPaths(paths) {
    set({ recentProjectPaths: paths })
  },

  setAppTheme(theme) {
    set({ appTheme: theme })
  },

  setAppPrimaryColor(color) {
    set({ appPrimaryColor: color })
  },

  openProjectTab() {
    const { project } = get()
    if (!project) {
      toast.error('请先选择项目')
      return
    }

    const entityId = project.metadata.id
    const requestId = `project:${entityId}`
    const draft = createProjectEditorDraft(project)

    set((state) => {
      const existing = state.openRequestTabs.find(tab => tab.requestId === requestId)
      const nextTab: OpenRequestTab = {
        entityType: 'project',
        requestId,
        entityId,
        title: `${project.metadata.name} 设置`,
        method: 'PROJ',
        dirty: false,
        lastFocusedAt: Date.now(),
        editorTab: existing?.editorTab as SettingsPanelTab ?? 'info',
      }

      return {
        projectDrafts: { ...state.projectDrafts, [entityId]: draft },
        savedProjects: { ...state.savedProjects, [entityId]: createProjectEditorDraft(project) },
        openRequestTabs: existing
          ? state.openRequestTabs.map(tab => tab.requestId === requestId ? nextTab : tab)
          : [...state.openRequestTabs, nextTab],
        activeRequestId: requestId,
        activeEditorTab: nextTab.editorTab,
      }
    })

    get().syncTabState()
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
          appTheme: appConfig.theme,
          appPrimaryColor: appConfig.primaryColor,
        })
        get().closeCreateProjectDialog()
        get().syncTabState()
      }, '项目已创建')
    }
    catch (error) {
      toast.error(error instanceof Error ? error.message : String(error))
    }
  },

  async handleEditProject() {
    const { project, activeRequestId, openRequestTabs, projectDrafts } = get()
    const activeTab = activeRequestId ? openRequestTabs.find(tab => tab.requestId === activeRequestId) : null
    const entityId = activeTab?.entityType === 'project' ? activeTab.entityId : null
    const draft = entityId ? projectDrafts[entityId] : null
    const name = draft?.name.trim() ?? ''
    if (!project || !draft || !name) {
      toast.error('项目名称不能为空')
      return
    }

    await runTask(set, async () => {
      await updateProject({
        id: project.metadata.id,
        name,
        description: draft.description.trim(),
        docs: project.metadata.docs,
        mock: project.metadata.mock,
        requestConfig: cloneRequestScopeConfig(draft.requestConfig),
      })
      set((state) => {
        if (!entityId) {
          return {}
        }

        const nextDirtyIds = new Set(state.dirtyRequestIds)
        nextDirtyIds.delete(entityId)
        return {
          dirtyRequestIds: nextDirtyIds,
          openRequestTabs: state.openRequestTabs.map(tab =>
            tab.entityType === 'project' && tab.entityId === entityId
              ? { ...tab, dirty: false }
              : tab,
          ),
        }
      })
      await get().refreshProjectInternal()
    }, '项目已更新')
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
        primaryColor: appConfig.primaryColor,
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

  openCollectionTab(node, parentCollectionId) {
    const entityId = node.id
    const requestId = `collection:${entityId}`
    const draft = createCollectionEditorDraft(node)

    set((state) => {
      const existing = state.openRequestTabs.find(tab => tab.requestId === requestId)
      const nextTab: OpenRequestTab = {
        entityType: 'collection',
        requestId,
        entityId,
        title: `${node.name} 设置`,
        method: 'DIR',
        dirty: false,
        lastFocusedAt: Date.now(),
        editorTab: existing?.editorTab as SettingsPanelTab ?? 'info',
      }

      return {
        collectionDrafts: { ...state.collectionDrafts, [entityId]: draft },
        savedCollections: { ...state.savedCollections, [entityId]: createCollectionEditorDraft(node) },
        openRequestTabs: existing
          ? state.openRequestTabs.map(tab => tab.requestId === requestId ? nextTab : tab)
          : [...state.openRequestTabs, nextTab],
        activeRequestId: requestId,
        activeEditorTab: nextTab.editorTab,
        selectedTreeNode: { type: 'collection', id: entityId, parentCollectionId },
      }
    })

    get().syncTabState()
  },

  openEditCollectionDialog(node) {
    const location = get().project ? findCollectionPath(get().project!.children, node.id) : []
    const parentCollectionId = location.length > 1 ? location.at(-2)?.id ?? null : null
    get().openCollectionTab(node, parentCollectionId)
  },

  closeEditCollectionDialog() {
    set({
      editCollectionDialogOpen: false,
      editingCollectionId: null,
      editCollectionNameDraft: '',
      editCollectionDescriptionDraft: '',
      editCollectionRequestConfigDraft: createDefaultRequestScopeConfig(),
    })
  },

  setEditCollectionNameDraft(value) {
    set({ editCollectionNameDraft: value })
  },

  setEditCollectionDescriptionDraft(value) {
    set({ editCollectionDescriptionDraft: value })
  },

  setEditCollectionRequestConfigDraft(value) {
    set({ editCollectionRequestConfigDraft: value })
  },

  updateCollectionTabDraft(updater) {
    const { activeRequestId, openRequestTabs, collectionDrafts, savedCollections } = get() as WorkbenchStore & {
      updateCollectionTabDraft?: unknown
    }
    const activeTab = activeRequestId ? openRequestTabs.find(tab => tab.requestId === activeRequestId) : null
    const entityId = activeTab?.entityType === 'collection' ? activeTab.entityId : null
    if (!entityId || !collectionDrafts[entityId]) {
      return
    }

    const nextDraft = updater(JSON.parse(JSON.stringify(collectionDrafts[entityId])) as CollectionEditorDraft)
    const dirty = JSON.stringify(savedCollections[entityId]) !== JSON.stringify(nextDraft)
    set((state) => {
      const nextDirtyIds = new Set(state.dirtyRequestIds)
      if (dirty) {
        nextDirtyIds.add(entityId)
      }
      else {
        nextDirtyIds.delete(entityId)
      }
      return {
        collectionDrafts: { ...state.collectionDrafts, [entityId]: nextDraft },
        dirtyRequestIds: nextDirtyIds,
        openRequestTabs: state.openRequestTabs.map(tab =>
          tab.entityType === 'collection' && tab.entityId === entityId
            ? { ...tab, title: `${nextDraft.name} 设置`, dirty }
            : tab,
        ),
      }
    })
  },

  updateProjectTabDraft(updater) {
    const { activeRequestId, openRequestTabs, projectDrafts, savedProjects } = get() as WorkbenchStore & {
      updateProjectTabDraft?: unknown
    }
    const activeTab = activeRequestId ? openRequestTabs.find(tab => tab.requestId === activeRequestId) : null
    const entityId = activeTab?.entityType === 'project' ? activeTab.entityId : null
    if (!entityId || !projectDrafts[entityId]) {
      return
    }

    const nextDraft = updater(JSON.parse(JSON.stringify(projectDrafts[entityId])) as ProjectEditorDraft)
    const dirty = JSON.stringify(savedProjects[entityId]) !== JSON.stringify(nextDraft)
    set((state) => {
      const nextDirtyIds = new Set(state.dirtyRequestIds)
      if (dirty) {
        nextDirtyIds.add(entityId)
      }
      else {
        nextDirtyIds.delete(entityId)
      }
      return {
        projectDrafts: { ...state.projectDrafts, [entityId]: nextDraft },
        dirtyRequestIds: nextDirtyIds,
        openRequestTabs: state.openRequestTabs.map(tab =>
          tab.entityType === 'project' && tab.entityId === entityId
            ? { ...tab, title: `${nextDraft.name} 设置`, dirty }
            : tab,
        ),
      }
    })
  },

  async handleEditCollection() {
    const { activeRequestId, openRequestTabs, collectionDrafts } = get()
    const activeTab = activeRequestId ? openRequestTabs.find(tab => tab.requestId === activeRequestId) : null
    const entityId = activeTab?.entityType === 'collection' ? activeTab.entityId : null
    const draft = entityId ? collectionDrafts[entityId] : null
    const name = draft?.name.trim() ?? ''
    if (!entityId || !draft || !name) {
      toast.error('请填写目录名称')
      return
    }

    await runTask(set, async () => {
      await updateCollection({
        id: entityId,
        name,
        description: draft.description.trim(),
        requestConfig: cloneRequestScopeConfig(draft.requestConfig),
      })
      set((state) => {
        if (!entityId) {
          return {}
        }

        const nextDirtyIds = new Set(state.dirtyRequestIds)
        nextDirtyIds.delete(entityId)
        return {
          dirtyRequestIds: nextDirtyIds,
          openRequestTabs: state.openRequestTabs.map(tab =>
            tab.entityType === 'collection' && tab.entityId === entityId
              ? { ...tab, dirty: false }
              : tab,
          ),
        }
      })
      await get().refreshProjectInternal()
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
    const requestTabId = `request:${summary.id}`

    set({
      selectedTreeNode: { type: 'api', id: summary.id, parentCollectionId },
    })

    if (requestDrafts[summary.id]) {
      get().focusRequestTab(requestTabId)
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
    const state = get()
    const entityId = getActiveRequestEntityId(state)
    if (!entityId) {
      return
    }

    const currentDraft = state.requestDrafts[entityId]
    if (!currentDraft) {
      return
    }

    const nextDraft = updater(cloneApiDefinition(currentDraft))
    const saved = state.savedRequests[entityId]
    const dirty = saved ? !areApiDefinitionsEqual(saved, nextDraft) : true

    set((currentState) => {
      const nextDirtyIds = new Set(currentState.dirtyRequestIds)
      if (dirty) {
        nextDirtyIds.add(entityId)
      }
      else {
        nextDirtyIds.delete(entityId)
      }

      return {
        requestDrafts: { ...currentState.requestDrafts, [entityId]: nextDraft },
        dirtyRequestIds: nextDirtyIds,
        openRequestTabs: currentState.openRequestTabs.map(tab =>
          tab.entityType === 'request' && tab.entityId === entityId
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
    const state = get()
    const entityId = getActiveRequestEntityId(state)
    if (!entityId || !state.requestDrafts[entityId]) {
      return
    }

    const draft = cloneApiDefinition(state.requestDrafts[entityId])
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
    const state = get()
    const { project, environments, activeEnvironmentId, resolveUrl, resolveEnvironmentVariables } = state
    const entityId = getActiveRequestEntityId(state)
    if (!entityId || !state.requestDrafts[entityId]) {
      return
    }

    const draft = cloneApiDefinition(state.requestDrafts[entityId])
    const parentCollectionId = findApiLocation(project!, draft.id)?.parentCollectionId ?? null
    const requestScopeChain = getRequestScopeChain(project, parentCollectionId, draft)
    const activeProjectId = project?.metadata.id ?? null
    const activeEnv = activeEnvironmentId ? environments.find(e => e.id === activeEnvironmentId) ?? null : null

    const requestId = generateId()

    set(state => ({
      requestResponses: {
        ...state.requestResponses,
        [entityId]: {
          ...(state.requestResponses[entityId] ?? createEmptyResponseState()),
          requestId,
          status: null,
          headers: [],
          durationMs: 0,
          sizeBytes: 0,
          contentType: '',
          responseType: null,
          body: '',
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
        headers: requestScopeChain.mergedHeaders.map(h => ({ ...h })),
        query: draft.request.query.map(q => ({ ...q })),
        body: {
          mode: draft.request.body.mode,
          raw: draft.request.body.raw,
          json: draft.request.body.json,
          formData: draft.request.body.formData.map(item => ({ ...item })),
          urlEncoded: draft.request.body.urlEncoded.map(item => ({ ...item })),
        },
      }

      for (const script of requestScopeChain.preRequestScripts) {
        const ctx = createPreRequestContext(
          mutableConfig,
          key => envMutations.get(key) ?? activeEnv?.variables.find(v => v.key === key && v.enabled)?.value,
          (key, value) => { envMutations.set(key, value) },
          key => scriptVars.get(key),
          (key, value) => { scriptVars.set(key, value) },
        )
        const result = runScript(script, ctx)
        if (result.error) {
          throw new Error(`Pre-request script error: ${result.error}`)
        }
      }

      const variables = activeEnv?.variables ?? []

      // Resolve URL with baseURL joining (use config modified by pre-script)
      const scopeBaseUrl = requestScopeChain.resolvedBaseUrl || activeEnv?.baseUrl
      const resolvedUrl = resolveUrl(mutableConfig.url, scopeBaseUrl)

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
        ...requestScopeChain.mergedAuth,
        basic: {
          username: resolveEnvironmentVariables(requestScopeChain.mergedAuth.basic.username, variables),
          password: resolveEnvironmentVariables(requestScopeChain.mergedAuth.basic.password, variables),
        },
        bearerToken: resolveEnvironmentVariables(requestScopeChain.mergedAuth.bearerToken, variables),
        apiKey: {
          ...requestScopeChain.mergedAuth.apiKey,
          key: resolveEnvironmentVariables(requestScopeChain.mergedAuth.apiKey.key, variables),
          value: resolveEnvironmentVariables(requestScopeChain.mergedAuth.apiKey.value, variables),
        },
      }

      const response = await sendRequest(
        {
          requestId,
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
        },
        event => applySendRequestStreamEvent(set, get, event),
      )

      // Build response data for post-script
      const responseData: ResponseData = {
        status: response.status,
        headers: Object.fromEntries(response.headers.map((h: { name: string, value: string }) => [h.name, h.value])),
        body: response.body,
        time: response.durationMs,
      }

      // Run post-request script
      let finalResponse = responseData
      for (const script of requestScopeChain.postRequestScripts) {
        const postCtx = createPostRequestContext(
          mutableConfig,
          responseData,
          key => envMutations.get(key) ?? activeEnv?.variables.find(v => v.key === key && v.enabled)?.value,
          (key, value) => { envMutations.set(key, value) },
          key => scriptVars.get(key),
          (key, value) => { scriptVars.set(key, value) },
        )
        const postResult = runScript(script, postCtx)
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
          [entityId]: {
            requestId,
            status: finalResponse.status,
            headers: Object.entries(finalResponse.headers).map(([name, value]) => ({ name, value, description: '' })),
            durationMs: finalResponse.time,
            sizeBytes: new Blob([finalResponse.body]).size,
            contentType: finalResponse.headers['content-type'] ?? '',
            responseType: response.responseType,
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
          [entityId]: {
            ...(state.requestResponses[entityId] ?? createEmptyResponseState()),
            requestId,
            isLoading: false,
            error: error instanceof Error ? error.message : String(error),
          },
        },
      }))
      toast.error(error instanceof Error ? error.message : String(error))
    }
  },

  requestCloseRequestTab(requestId) {
    const tab = get().openRequestTabs.find(item => item.requestId === requestId)
    if (tab && get().dirtyRequestIds.has(tab.entityId)) {
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
      const closingTab = state.openRequestTabs.find(tab => tab.requestId === requestId) ?? null

      const nextState: Partial<WorkbenchStore> = {
        openRequestTabs: remaining,
        activeRequestId: nextActive?.requestId ?? null,
        activeEditorTab: nextActive?.editorTab ?? getDefaultTabEditorTab(nextActive?.entityType ?? 'request'),
        requestDrafts: Object.fromEntries(
          Object.entries(state.requestDrafts).filter(([id]) => id !== closingTab?.entityId),
        ),
        savedRequests: Object.fromEntries(
          Object.entries(state.savedRequests).filter(([id]) => id !== closingTab?.entityId),
        ),
        collectionDrafts: Object.fromEntries(
          Object.entries(state.collectionDrafts).filter(([id]) => id !== closingTab?.entityId),
        ),
        savedCollections: Object.fromEntries(
          Object.entries(state.savedCollections).filter(([id]) => id !== closingTab?.entityId),
        ),
        projectDrafts: Object.fromEntries(
          Object.entries(state.projectDrafts).filter(([id]) => id !== closingTab?.entityId),
        ),
        savedProjects: Object.fromEntries(
          Object.entries(state.savedProjects).filter(([id]) => id !== closingTab?.entityId),
        ),
        requestResponses: Object.fromEntries(
          Object.entries(state.requestResponses).filter(([id]) => id !== closingTab?.entityId),
        ),
        dirtyRequestIds: new Set([...state.dirtyRequestIds].filter(id => id !== closingTab?.entityId)),
      }

      if (nextActive && project) {
        nextState.selectedTreeNode = buildTreeSelectionForTab(project, nextActive)
      }
      else if (
        !nextActive
        && closingTab
        && selectedTreeNode
        && (
          (selectedTreeNode.type === 'api' && closingTab.entityType === 'request' && selectedTreeNode.id === closingTab.entityId)
          || (selectedTreeNode.type === 'collection' && closingTab.entityType === 'collection' && selectedTreeNode.id === closingTab.entityId)
        )
      ) {
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
    const { projectPath, openRequestTabs } = get()
    if (!projectPath) {
      return null
    }

    const snapshot = await openProject(projectPath)
    set((state) => {
      const nextCollectionDrafts = { ...state.collectionDrafts }
      const nextSavedCollections = { ...state.savedCollections }
      const nextProjectDrafts = { ...state.projectDrafts }
      const nextSavedProjects = { ...state.savedProjects }
      const nextDirtyIds = new Set(state.dirtyRequestIds)
      const nextTabs = state.openRequestTabs.map((tab) => {
        if (tab.entityType === 'project' && tab.entityId === snapshot.metadata.id) {
          const saved = createProjectEditorDraft(snapshot)
          const currentDraft = state.projectDrafts[tab.entityId]
          const dirty = nextDirtyIds.has(tab.entityId)
          nextSavedProjects[tab.entityId] = saved
          nextProjectDrafts[tab.entityId] = dirty && currentDraft ? currentDraft : saved
          return {
            ...tab,
            title: `${(dirty && currentDraft ? currentDraft.name : saved.name) || saved.name} 设置`,
            dirty,
          }
        }

        if (tab.entityType === 'collection') {
          const collection = findCollectionById(snapshot.children, tab.entityId)
          if (!collection) {
            return tab
          }

          const saved = createCollectionEditorDraft(collection)
          const currentDraft = state.collectionDrafts[tab.entityId]
          const dirty = nextDirtyIds.has(tab.entityId)
          nextSavedCollections[tab.entityId] = saved
          nextCollectionDrafts[tab.entityId] = dirty && currentDraft ? currentDraft : saved
          return {
            ...tab,
            title: `${(dirty && currentDraft ? currentDraft.name : saved.name) || saved.name} 设置`,
            dirty,
          }
        }

        return tab
      })

      return {
        project: snapshot,
        environments: snapshot.environments,
        activeEnvironmentId: snapshot.metadata.activeEnvironmentId ?? null,
        openRequestTabs: nextTabs,
        collectionDrafts: nextCollectionDrafts,
        savedCollections: nextSavedCollections,
        projectDrafts: nextProjectDrafts,
        savedProjects: nextSavedProjects,
      }
    })

    const activeTab = openRequestTabs.find(tab => tab.requestId === get().activeRequestId) ?? null
    const nextSelection = buildTreeSelectionForTab(snapshot, activeTab)
    set({ selectedTreeNode: nextSelection })
    return snapshot
  },

  setRequestLoaded(definition, setActiveTab = true) {
    const draft = cloneApiDefinition(definition)
    const saved = cloneApiDefinition(definition)
    const requestId = `request:${definition.id}`

    set((state) => {
      const nextTab: OpenRequestTab = {
        entityType: 'request',
        requestId,
        entityId: definition.id,
        title: definition.name,
        method: definition.method,
        dirty: false,
        lastFocusedAt: Date.now(),
        editorTab: (state.openRequestTabs.find(tab => tab.requestId === requestId)?.editorTab as EditorPanelTab | undefined) ?? 'query',
      }
      const existing = state.openRequestTabs.some(tab => tab.requestId === requestId)
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
          ? state.openRequestTabs.map(tab => (tab.requestId === requestId ? nextTab : tab))
          : [...state.openRequestTabs, nextTab],
        activeRequestId: setActiveTab ? requestId : state.activeRequestId,
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
          tab.entityType === 'request' && tab.entityId === definition.id
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
      const remaining = state.openRequestTabs.filter(tab => !isTabRemoved(tab, removedRequestIds, removedCollectionIds))
      const activeRemoved = activeRequestId
        ? Boolean(state.openRequestTabs.find(tab => tab.requestId === activeRequestId && isTabRemoved(tab, removedRequestIds, removedCollectionIds)))
        : false
      const nextActive = activeRemoved
        ? remaining.slice().sort((left, right) => right.lastFocusedAt - left.lastFocusedAt)[0]
        : remaining.find(tab => tab.requestId === activeRequestId) ?? null

      const nextState: Partial<WorkbenchStore> = {
        openRequestTabs: remaining,
        activeEditorTab: nextActive?.editorTab ?? getDefaultTabEditorTab(nextActive?.entityType ?? 'request'),
        requestDrafts: Object.fromEntries(
          Object.entries(state.requestDrafts).filter(([id]) => !removedRequestIds.has(id)),
        ),
        savedRequests: Object.fromEntries(
          Object.entries(state.savedRequests).filter(([id]) => !removedRequestIds.has(id)),
        ),
        collectionDrafts: Object.fromEntries(
          Object.entries(state.collectionDrafts).filter(([id]) => !removedCollectionIds.has(id)),
        ),
        savedCollections: Object.fromEntries(
          Object.entries(state.savedCollections).filter(([id]) => !removedCollectionIds.has(id)),
        ),
        projectDrafts: state.projectDrafts,
        savedProjects: state.savedProjects,
        requestResponses: Object.fromEntries(
          Object.entries(state.requestResponses).filter(([id]) => !removedRequestIds.has(id)),
        ),
        loadingRequestIds: state.loadingRequestIds.filter(requestId => !removedRequestIds.has(requestId)),
        dirtyRequestIds: new Set(
          [...state.dirtyRequestIds].filter(id => !removedRequestIds.has(id) && !removedCollectionIds.has(id)),
        ),
        pendingCloseRequestId: state.pendingCloseRequestId && remaining.every(tab => tab.requestId !== state.pendingCloseRequestId)
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
        nextState.selectedTreeNode = buildTreeSelectionForTab(nextProject, nextActive)
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
