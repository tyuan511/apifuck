import type { ReactNode, RefObject } from 'react'
import type {
  ApiDefinition,
  ApiSummary,
  CreateApiInput,
  KeyValue,
  ResponseHeader,
  ResponseType,
  SendRequestResponse,
  TreeNode,
  WorkspaceSnapshot,
} from '@/lib/workspace'
import {
  CheckIcon,
  ChevronDownIcon,
  ChevronRightIcon,
  EllipsisIcon,
  FilePlusIcon,
  FolderIcon,
  FolderOpenIcon,
  FolderPlusIcon,
  Loader2Icon,
  PlusIcon,
  SaveIcon,
  SendHorizonalIcon,
  Settings2Icon,
  XIcon,
} from 'lucide-react'
import { useTheme } from 'next-themes'
import { startTransition, useEffect, useMemo, useRef, useState } from 'react'
import { toast } from 'sonner'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { Input } from '@/components/ui/input'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Switch } from '@/components/ui/switch'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Textarea } from '@/components/ui/textarea'
import { openStartupWorkspace } from '@/lib/app-config'
import { cn } from '@/lib/utils'
import {
  createApi,
  createCollection,
  createDefaultDocumentation,
  createDefaultMock,
  createDefaultRequest,
  createProject,
  openWorkspace,
  readApi,
  sendRequest,
  updateApi,
} from '@/lib/workspace'

type EditorPanelTab = 'query' | 'headers' | 'body' | 'docs' | 'mock'
type TreeSelection
  = | { type: 'collection', id: string, parentCollectionId: string | null }
    | { type: 'api', id: string, parentCollectionId: string | null }
interface RequestEditorDraft extends ApiDefinition {}

interface OpenRequestTab {
  requestId: string
  title: string
  method: string
  dirty: boolean
  lastFocusedAt: number
}

interface ResponseState {
  status: number | null
  headers: ResponseHeader[]
  durationMs: number
  sizeBytes: number
  contentType: string
  responseType: ResponseType | null
  body: string
  isLoading: boolean
  error: string | null
}

const methodOptions = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS']
const editorTabs: Array<{ value: EditorPanelTab, label: string }> = [
  { value: 'query', label: 'Query' },
  { value: 'headers', label: 'Headers' },
  { value: 'body', label: 'Body' },
  { value: 'docs', label: 'Docs' },
  { value: 'mock', label: 'Mock' },
]

export function App() {
  const { setTheme } = useTheme()
  const splitContainerRef = useRef<HTMLDivElement | null>(null)

  const [workspacePath, setWorkspacePath] = useState('')
  const [workspace, setWorkspace] = useState<WorkspaceSnapshot | null>(null)
  const [isBooting, setIsBooting] = useState(true)
  const [isBusy, setIsBusy] = useState(false)
  const [activeProjectId, setActiveProjectId] = useState<string | null>(null)
  const [selectedTreeNode, setSelectedTreeNode] = useState<TreeSelection | null>(null)
  const [collapsedCollectionIds, setCollapsedCollectionIds] = useState<string[]>([])
  const [openRequestTabs, setOpenRequestTabs] = useState<OpenRequestTab[]>([])
  const [activeRequestId, setActiveRequestId] = useState<string | null>(null)
  const [requestDrafts, setRequestDrafts] = useState<Record<string, RequestEditorDraft>>({})
  const [savedRequests, setSavedRequests] = useState<Record<string, ApiDefinition>>({})
  const [dirtyRequestIds, setDirtyRequestIds] = useState<Set<string>>(new Set())
  const [loadingRequestIds, setLoadingRequestIds] = useState<string[]>([])
  const [requestResponses, setRequestResponses] = useState<Record<string, ResponseState>>({})
  const [activeEditorTab, setActiveEditorTab] = useState<EditorPanelTab>('query')
  const [splitRatio, setSplitRatio] = useState(0.55)
  const [isDraggingSplit, setIsDraggingSplit] = useState(false)

  const [projectDialogOpen, setProjectDialogOpen] = useState(false)
  const [projectNameDraft, setProjectNameDraft] = useState('')
  const [projectDescriptionDraft, setProjectDescriptionDraft] = useState('')
  const [collectionDialogOpen, setCollectionDialogOpen] = useState(false)
  const [collectionNameDraft, setCollectionNameDraft] = useState('')
  const [requestDialogOpen, setRequestDialogOpen] = useState(false)
  const [requestDialogParentCollectionId, setRequestDialogParentCollectionId] = useState<string | null>(null)
  const [requestNameDraft, setRequestNameDraft] = useState('')
  const [requestMethodDraft, setRequestMethodDraft] = useState('GET')
  const [requestUrlDraft, setRequestUrlDraft] = useState('https://example.com')
  const [pendingCloseRequestId, setPendingCloseRequestId] = useState<string | null>(null)

  const activeProject = getActiveProject(workspace, activeProjectId)
  const activeDraft = activeRequestId ? requestDrafts[activeRequestId] ?? null : null
  const activeResponse = activeRequestId ? requestResponses[activeRequestId] ?? null : null
  const activeRequestIsLoading = Boolean(
    activeRequestId && loadingRequestIds.includes(activeRequestId) && !activeDraft,
  )
  const currentProjectCollectionIds = activeProject ? collectCollectionIds(activeProject.children) : []
  const currentProjectCollectionNames = useMemo(() => new Map(
    currentProjectCollectionIds.map(id => [id, findCollectionName(activeProject?.children ?? [], id)]),
  ), [activeProject?.children, currentProjectCollectionIds])

  useEffect(() => {
    let cancelled = false

    async function bootstrapWorkbench() {
      setIsBooting(true)
      try {
        const startup = await openStartupWorkspace()
        if (cancelled) {
          return
        }

        startTransition(() => {
          setWorkspacePath(startup.workspacePath)
          setWorkspace(startup.workspaceSnapshot)
          setActiveProjectId(startup.workspaceSnapshot.lastOpenedProjectId)
          setSelectedTreeNode(null)
          setOpenRequestTabs([])
          setActiveRequestId(null)
          setRequestDrafts({})
          setSavedRequests({})
          setDirtyRequestIds(new Set())
          setRequestResponses({})
          setLoadingRequestIds([])
        })
        setTheme(startup.appConfig.theme)
      }
      catch (error) {
        if (!cancelled) {
          toast.error(error instanceof Error ? error.message : String(error))
        }
      }
      finally {
        if (!cancelled) {
          setIsBooting(false)
        }
      }
    }

    void bootstrapWorkbench()

    return () => {
      cancelled = true
    }
  }, [setTheme])

  useEffect(() => {
    if (!isDraggingSplit) {
      return
    }

    function handlePointerMove(event: MouseEvent) {
      const bounds = splitContainerRef.current?.getBoundingClientRect()
      if (!bounds) {
        return
      }

      const nextRatio = clamp((event.clientY - bounds.top) / bounds.height, 0.28, 0.78)
      setSplitRatio(nextRatio)
    }

    function stopDragging() {
      setIsDraggingSplit(false)
    }

    window.addEventListener('mousemove', handlePointerMove)
    window.addEventListener('mouseup', stopDragging)

    return () => {
      window.removeEventListener('mousemove', handlePointerMove)
      window.removeEventListener('mouseup', stopDragging)
    }
  }, [isDraggingSplit])

  useEffect(() => {
    if (!workspace) {
      return
    }

    const activeExists = activeProjectId && workspace.projects.some(project => project.metadata.id === activeProjectId)
    if (!activeExists) {
      setActiveProjectId(workspace.lastOpenedProjectId || workspace.defaultProjectId)
    }
  }, [activeProjectId, workspace])

  useEffect(() => {
    if (!activeProject || !selectedTreeNode) {
      return
    }

    if (selectedTreeNode.type === 'collection') {
      if (!currentProjectCollectionIds.includes(selectedTreeNode.id)) {
        setSelectedTreeNode(null)
      }
      return
    }

    const location = findApiLocationInProject(activeProject.children, selectedTreeNode.id)
    if (!location) {
      setSelectedTreeNode(null)
    }
  }, [activeProject, currentProjectCollectionIds, selectedTreeNode])

  async function runTask<T>(task: () => Promise<T>, successMessage?: string) {
    setIsBusy(true)
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
      setIsBusy(false)
    }
  }

  async function refreshWorkspace() {
    if (!workspacePath) {
      return null
    }

    const snapshot = await openWorkspace(workspacePath)
    setWorkspace(snapshot)
    return snapshot
  }

  function toggleCollection(collectionId: string) {
    setCollapsedCollectionIds((current) => {
      if (current.includes(collectionId)) {
        return current.filter(id => id !== collectionId)
      }
      return [...current, collectionId]
    })
  }

  function setNodeSelection(selection: TreeSelection | null) {
    setSelectedTreeNode(selection)
  }

  function focusRequestTab(requestId: string) {
    setOpenRequestTabs(current =>
      current.map(tab => (tab.requestId === requestId ? { ...tab, lastFocusedAt: Date.now() } : tab)),
    )
    setActiveRequestId(requestId)

    const location = workspace ? findApiLocation(workspace, requestId) : null
    if (location) {
      setActiveProjectId(location.projectId)
      setSelectedTreeNode({
        type: 'api',
        id: requestId,
        parentCollectionId: location.parentCollectionId,
      })
    }
  }

  function setRequestLoaded(definition: ApiDefinition) {
    const draft = cloneApiDefinition(definition)
    const saved = cloneApiDefinition(definition)

    setRequestDrafts(current => ({ ...current, [definition.id]: draft }))
    setSavedRequests(current => ({ ...current, [definition.id]: saved }))
    setRequestResponses((current) => {
      if (current[definition.id]) {
        return current
      }
      return {
        ...current,
        [definition.id]: createEmptyResponseState(),
      }
    })
    setDirtyRequestIds((current) => {
      const next = new Set(current)
      next.delete(definition.id)
      return next
    })
    setOpenRequestTabs((current) => {
      const nextTab: OpenRequestTab = {
        requestId: definition.id,
        title: definition.name,
        method: definition.method,
        dirty: false,
        lastFocusedAt: Date.now(),
      }
      const existing = current.some(tab => tab.requestId === definition.id)
      if (existing) {
        return current.map(tab => (tab.requestId === definition.id ? nextTab : tab))
      }
      return [...current, nextTab]
    })
    setActiveRequestId(definition.id)
  }

  async function openRequestFromSummary(summary: ApiSummary, parentCollectionId: string | null) {
    const projectLocation = workspace ? findApiLocation(workspace, summary.id) : null
    if (projectLocation) {
      setActiveProjectId(projectLocation.projectId)
    }
    setSelectedTreeNode({ type: 'api', id: summary.id, parentCollectionId })

    if (requestDrafts[summary.id]) {
      focusRequestTab(summary.id)
      return
    }

    setLoadingRequestIds(current => (current.includes(summary.id) ? current : [...current, summary.id]))
    try {
      const definition = await readApi(summary.id)
      setRequestLoaded(definition)
    }
    catch (error) {
      toast.error(error instanceof Error ? error.message : String(error))
    }
    finally {
      setLoadingRequestIds(current => current.filter(id => id !== summary.id))
    }
  }

  function updateRequestDraft(updater: (draft: RequestEditorDraft) => RequestEditorDraft) {
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

    setRequestDrafts(current => ({ ...current, [activeRequestId]: nextDraft }))
    setDirtyRequestIds((current) => {
      const next = new Set(current)
      if (dirty) {
        next.add(activeRequestId)
      }
      else {
        next.delete(activeRequestId)
      }
      return next
    })
    setOpenRequestTabs(current =>
      current.map(tab => (tab.requestId === activeRequestId
        ? {
            ...tab,
            title: nextDraft.name,
            method: nextDraft.method,
            dirty,
          }
        : tab)),
    )
  }

  async function handleSaveRequest() {
    if (!activeRequestId || !activeDraft) {
      return
    }

    const draft = cloneApiDefinition(activeDraft)
    await runTask(async () => {
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
      setRequestLoaded(saved)
      await refreshWorkspace()
    }, '请求已保存')
  }

  async function handleSendRequest() {
    if (!activeRequestId || !activeDraft) {
      return
    }

    const draft = cloneApiDefinition(activeDraft)
    setRequestResponses(current => ({
      ...current,
      [activeRequestId]: {
        ...(current[activeRequestId] ?? createEmptyResponseState()),
        isLoading: true,
        error: null,
      },
    }))

    try {
      const response = await sendRequest({
        method: draft.method,
        url: draft.url,
        request: draft.request,
      })
      setRequestResponses(current => ({
        ...current,
        [activeRequestId]: mapSendResponseToState(response),
      }))
    }
    catch (error) {
      setRequestResponses(current => ({
        ...current,
        [activeRequestId]: {
          ...(current[activeRequestId] ?? createEmptyResponseState()),
          isLoading: false,
          error: error instanceof Error ? error.message : String(error),
        },
      }))
      toast.error(error instanceof Error ? error.message : String(error))
    }
  }

  function requestCloseRequestTab(requestId: string) {
    if (dirtyRequestIds.has(requestId)) {
      setPendingCloseRequestId(requestId)
      return
    }
    closeRequestTab(requestId)
  }

  function closeRequestTab(requestId: string) {
    setOpenRequestTabs((current) => {
      const remaining = current.filter(tab => tab.requestId !== requestId)
      const nextActive = remaining
        .slice()
        .sort((left, right) => right.lastFocusedAt - left.lastFocusedAt)[0]

      setActiveRequestId(nextActive?.requestId ?? null)

      if (nextActive && workspace) {
        const location = findApiLocation(workspace, nextActive.requestId)
        if (location) {
          setActiveProjectId(location.projectId)
          setSelectedTreeNode({
            type: 'api',
            id: nextActive.requestId,
            parentCollectionId: location.parentCollectionId,
          })
        }
      }
      else if (!nextActive && selectedTreeNode?.type === 'api' && selectedTreeNode.id === requestId) {
        setSelectedTreeNode(null)
      }

      return remaining
    })

    setRequestDrafts((current) => {
      const next = { ...current }
      delete next[requestId]
      return next
    })
    setSavedRequests((current) => {
      const next = { ...current }
      delete next[requestId]
      return next
    })
    setRequestResponses((current) => {
      const next = { ...current }
      delete next[requestId]
      return next
    })
    setDirtyRequestIds((current) => {
      const next = new Set(current)
      next.delete(requestId)
      return next
    })
  }

  async function handleCreateProject() {
    const name = projectNameDraft.trim()
    if (!name) {
      toast.error('项目名称不能为空')
      return
    }

    await runTask(async () => {
      const project = await createProject({
        name,
        description: projectDescriptionDraft.trim(),
      })
      const snapshot = await refreshWorkspace()
      if (snapshot) {
        setActiveProjectId(project.id)
      }
      setProjectDialogOpen(false)
      setProjectNameDraft('')
      setProjectDescriptionDraft('')
    }, '项目已创建')
  }

  async function handleCreateCollection() {
    const name = collectionNameDraft.trim()
    if (!name || !activeProject) {
      toast.error('请先选择项目并填写集合名称')
      return
    }

    const parentCollectionId = null
    await runTask(async () => {
      const collection = await createCollection({
        projectId: activeProject.metadata.id,
        parentCollectionId: parentCollectionId ?? undefined,
        name,
        description: '',
      })
      await refreshWorkspace()
      setSelectedTreeNode({
        type: 'collection',
        id: collection.id,
        parentCollectionId,
      })
      setCollapsedCollectionIds(current => current.filter(id => id !== collection.id))
      setCollectionDialogOpen(false)
      setCollectionNameDraft('')
    }, '集合已创建')
  }

  async function handleCreateRequest() {
    const name = requestNameDraft.trim()
    const url = requestUrlDraft.trim()
    if (!name || !url || !activeProject) {
      toast.error('请填写请求名称和 URL')
      return
    }

    const parentCollectionId = requestDialogParentCollectionId
    const input: CreateApiInput = {
      projectId: activeProject.metadata.id,
      parentCollectionId: parentCollectionId ?? undefined,
      name,
      method: requestMethodDraft,
      url,
      description: '',
      tags: [],
      request: createDefaultRequest(),
      documentation: createDefaultDocumentation(
        parentCollectionId ? currentProjectCollectionNames.get(parentCollectionId) ?? '' : '',
      ),
      mock: createDefaultMock(),
    }

    await runTask(async () => {
      const definition = await createApi(input)
      await refreshWorkspace()
      setRequestLoaded(definition)
      setSelectedTreeNode({
        type: 'api',
        id: definition.id,
        parentCollectionId,
      })
      setRequestDialogOpen(false)
      setRequestDialogParentCollectionId(null)
      setRequestNameDraft('')
      setRequestMethodDraft('GET')
      setRequestUrlDraft('https://example.com')
    }, '请求已创建')
  }

  const shellClassName = 'h-screen bg-background text-foreground'
  const shellColumns = {
    gridTemplateColumns: 'clamp(248px, 23vw, 308px) minmax(0, 1fr)',
  } as const

  if (isBooting) {
    return (
      <main className={shellClassName}>
        <div className="grid h-full place-items-center">
          <div className="flex items-center gap-3 text-sm text-muted-foreground">
            <Loader2Icon className="size-4 animate-spin" />
            正在加载工作台...
          </div>
        </div>
      </main>
    )
  }

  return (
    <main className={shellClassName}>
      <div className="grid h-full min-h-0" style={shellColumns}>
        <ProjectSidebar
          activeProjectId={activeProjectId}
          collapsedCollectionIds={collapsedCollectionIds}
          openRequestTabs={openRequestTabs}
          projects={workspace?.projects ?? []}
          selectedTreeNode={selectedTreeNode}
          onCreateCollection={() => {
            setCollectionDialogOpen(true)
          }}
          onCreateProject={() => setProjectDialogOpen(true)}
          onCreateRequest={(parentCollectionId) => {
            setRequestDialogParentCollectionId(parentCollectionId)
            setRequestDialogOpen(true)
          }}
          onOpenRequest={openRequestFromSummary}
          onProjectChange={(projectId) => {
            setActiveProjectId(projectId)
            setSelectedTreeNode(null)
          }}
          onToggleCollection={toggleCollection}
          onTreeSelectionChange={setNodeSelection}
        />

        <RequestWorkspace
          activeDraft={activeDraft}
          activeEditorTab={activeEditorTab}
          activeRequestId={activeRequestId}
          activeRequestIsLoading={activeRequestIsLoading}
          activeResponse={activeResponse}
          dirtyRequestIds={dirtyRequestIds}
          isBusy={isBusy}
          isDraggingSplit={isDraggingSplit}
          openRequestTabs={openRequestTabs}
          pendingCloseRequestId={pendingCloseRequestId}
          splitContainerRef={splitContainerRef}
          splitRatio={splitRatio}
          onActiveEditorTabChange={setActiveEditorTab}
          onChangeDraft={updateRequestDraft}
          onCloseRequestDialogChange={setPendingCloseRequestId}
          onCloseRequestTab={requestCloseRequestTab}
          onConfirmCloseRequestTab={() => {
            if (pendingCloseRequestId) {
              closeRequestTab(pendingCloseRequestId)
              setPendingCloseRequestId(null)
            }
          }}
          onFocusRequestTab={focusRequestTab}
          onSaveRequest={() => {
            void handleSaveRequest()
          }}
          onSendRequest={() => {
            void handleSendRequest()
          }}
          onStartDraggingSplit={() => setIsDraggingSplit(true)}
        />
      </div>

      <CreateProjectDialog
        description={projectDescriptionDraft}
        name={projectNameDraft}
        open={projectDialogOpen}
        onDescriptionChange={setProjectDescriptionDraft}
        onNameChange={setProjectNameDraft}
        onOpenChange={setProjectDialogOpen}
        onSubmit={() => {
          void handleCreateProject()
        }}
      />

      <CreateCollectionDialog
        name={collectionNameDraft}
        open={collectionDialogOpen}
        parentLabel="project root"
        onNameChange={setCollectionNameDraft}
        onOpenChange={setCollectionDialogOpen}
        onSubmit={() => {
          void handleCreateCollection()
        }}
      />

      <CreateRequestDialog
        method={requestMethodDraft}
        name={requestNameDraft}
        open={requestDialogOpen}
        parentLabel={describeCollectionParent(requestDialogParentCollectionId, currentProjectCollectionNames)}
        url={requestUrlDraft}
        onMethodChange={setRequestMethodDraft}
        onNameChange={setRequestNameDraft}
        onOpenChange={(open) => {
          setRequestDialogOpen(open)
          if (!open) {
            setRequestDialogParentCollectionId(null)
          }
        }}
        onSubmit={() => {
          void handleCreateRequest()
        }}
        onUrlChange={setRequestUrlDraft}
      />
    </main>
  )
}

interface ProjectSidebarProps {
  activeProjectId: string | null
  collapsedCollectionIds: string[]
  openRequestTabs: OpenRequestTab[]
  projects: WorkspaceSnapshot['projects']
  selectedTreeNode: TreeSelection | null
  onCreateCollection: () => void
  onCreateProject: () => void
  onCreateRequest: (parentCollectionId: string | null) => void
  onOpenRequest: (summary: ApiSummary, parentCollectionId: string | null) => void
  onProjectChange: (projectId: string) => void
  onToggleCollection: (collectionId: string) => void
  onTreeSelectionChange: (selection: TreeSelection | null) => void
}

function ProjectSidebar(props: ProjectSidebarProps) {
  const activeProject = props.projects.find(project => project.metadata.id === props.activeProjectId) ?? props.projects[0] ?? null

  return (
    <aside className="flex min-h-0 flex-col border-r border-border/70 bg-sidebar/50">
      <div className="border-b border-border/70 px-3 py-3">
        <div className="flex items-center justify-between gap-2">
          <div className="min-w-0">
            <p className="truncate text-base font-semibold tracking-tight">
              {activeProject?.metadata.name ?? 'No project selected'}
            </p>
          </div>

          <DropdownMenu>
            <DropdownMenuTrigger
              render={(
                <Button size="icon-sm" variant="ghost" />
              )}
            >
              <EllipsisIcon />
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" className="w-72">
              <div className="flex items-center justify-between px-2 py-1.5">
                <span className="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
                  Projects
                </span>
                <button
                  type="button"
                  onClick={props.onCreateProject}
                  className="rounded-md p-1 text-muted-foreground transition hover:bg-accent hover:text-foreground"
                >
                  <PlusIcon className="size-4" />
                </button>
              </div>
              <DropdownMenuSeparator />
              {props.projects.map(project => (
                <DropdownMenuItem
                  key={project.metadata.id}
                  onClick={() => props.onProjectChange(project.metadata.id)}
                  className="justify-between"
                >
                  <span className="flex min-w-0 flex-col items-start gap-0">
                    <span className="truncate">{project.metadata.name}</span>
                    <span className="text-xs text-muted-foreground">{project.metadata.slug}</span>
                  </span>
                  {project.metadata.id === activeProject?.metadata.id && (
                    <CheckIcon className="size-4 text-emerald-500" />
                  )}
                </DropdownMenuItem>
              ))}
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      </div>

      <div className="min-h-0 flex-1 overflow-auto px-2.5 py-3">
        <div className="mb-2 flex items-center justify-between px-0.5">
          <span className="text-[11px] font-medium uppercase tracking-[0.22em] text-muted-foreground">
            Requests
          </span>
          <div className="flex items-center gap-0.5">
            <button
              type="button"
              onClick={props.onCreateCollection}
              className="rounded-md p-1.5 text-muted-foreground transition hover:bg-accent hover:text-foreground"
            >
              <FolderPlusIcon className="size-4" />
            </button>
            <button
              type="button"
              onClick={() => props.onCreateRequest(null)}
              className="rounded-md p-1.5 text-muted-foreground transition hover:bg-accent hover:text-foreground"
            >
              <FilePlusIcon className="size-4" />
            </button>
          </div>
        </div>
        {activeProject
          ? (
              <div className="space-y-1">
                {activeProject.children.length > 0
                  ? activeProject.children.map(node => (
                      <RequestTreeNode
                        key={node.id}
                        collapsedCollectionIds={props.collapsedCollectionIds}
                        node={node}
                        openRequestTabs={props.openRequestTabs}
                        selectedTreeNode={props.selectedTreeNode}
                        onCreateRequest={props.onCreateRequest}
                        onOpenRequest={props.onOpenRequest}
                        onToggleCollection={props.onToggleCollection}
                        onTreeSelectionChange={props.onTreeSelectionChange}
                      />
                    ))
                  : <SidebarEmptyState title="No requests yet" body="Create a collection or request to start building this project." />}
              </div>
            )
          : <SidebarEmptyState title="No projects" body="Create your first project to open the workbench." />}
      </div>

      <div className="border-t border-border/70 px-3 py-3">
        <button
          type="button"
          className="flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-left text-[13px] text-muted-foreground transition hover:bg-accent hover:text-foreground"
        >
          <Settings2Icon className="size-4" />
          设置
        </button>
      </div>
    </aside>
  )
}

interface RequestTreeNodeProps {
  collapsedCollectionIds: string[]
  node: TreeNode
  openRequestTabs: OpenRequestTab[]
  selectedTreeNode: TreeSelection | null
  onCreateRequest: (parentCollectionId: string | null) => void
  onOpenRequest: (summary: ApiSummary, parentCollectionId: string | null) => void
  onToggleCollection: (collectionId: string) => void
  onTreeSelectionChange: (selection: TreeSelection | null) => void
  parentCollectionId?: string | null
}

function RequestTreeNode(props: RequestTreeNodeProps) {
  const {
    collapsedCollectionIds,
    node,
    openRequestTabs,
    parentCollectionId = null,
    selectedTreeNode,
    onCreateRequest,
    onOpenRequest,
    onToggleCollection,
    onTreeSelectionChange,
  } = props

  if (node.entityType === 'collection') {
    const isCollapsed = collapsedCollectionIds.includes(node.id)
    const isSelected = selectedTreeNode?.type === 'collection' && selectedTreeNode.id === node.id
    return (
      <div>
        <button
          type="button"
          onClick={() => {
            onTreeSelectionChange({ type: 'collection', id: node.id, parentCollectionId })
          }}
          className={cn(
            'flex w-full items-center gap-1.5 rounded-lg px-1.5 py-1.5 text-left transition hover:bg-accent',
            isSelected && 'bg-accent text-foreground',
          )}
        >
          <span
            className="flex size-4 items-center justify-center rounded-md text-muted-foreground hover:text-foreground"
            onClick={(event) => {
              event.stopPropagation()
              onToggleCollection(node.id)
            }}
          >
            {isCollapsed ? <ChevronRightIcon className="size-4" /> : <ChevronDownIcon className="size-4" />}
          </span>
          {isCollapsed ? <FolderIcon className="size-4 text-muted-foreground" /> : <FolderOpenIcon className="size-4 text-muted-foreground" />}
          <span className="min-w-0 flex-1 truncate text-[13px] font-medium">{node.name}</span>
          <span className="text-[11px] text-muted-foreground">{node.children.length}</span>
          <span
            role="button"
            tabIndex={0}
            onClick={(event) => {
              event.stopPropagation()
              onCreateRequest(node.id)
            }}
            onKeyDown={(event) => {
              if (event.key === 'Enter' || event.key === ' ') {
                event.preventDefault()
                event.stopPropagation()
                onCreateRequest(node.id)
              }
            }}
            className="rounded-md p-1 text-muted-foreground transition hover:bg-background hover:text-foreground"
          >
            <FilePlusIcon className="size-4" />
          </span>
        </button>
        {!isCollapsed && node.children.length > 0 && (
          <div className="ml-4 border-l border-border/60 pl-1.5">
            {node.children.map(child => (
              <RequestTreeNode
                key={child.id}
                collapsedCollectionIds={collapsedCollectionIds}
                node={child}
                openRequestTabs={openRequestTabs}
                parentCollectionId={node.id}
                selectedTreeNode={selectedTreeNode}
                onCreateRequest={onCreateRequest}
                onOpenRequest={onOpenRequest}
                onToggleCollection={onToggleCollection}
                onTreeSelectionChange={onTreeSelectionChange}
              />
            ))}
          </div>
        )}
      </div>
    )
  }

  const isSelected = selectedTreeNode?.type === 'api' && selectedTreeNode.id === node.id
  const isOpen = openRequestTabs.some(tab => tab.requestId === node.id)
  return (
    <button
      type="button"
      onClick={() => {
        onTreeSelectionChange({ type: 'api', id: node.id, parentCollectionId })
        onOpenRequest(node, parentCollectionId)
      }}
      className={cn(
        'mt-0.5 flex w-full items-center gap-1.5 rounded-lg px-1.5 py-1.5 text-left transition hover:bg-accent',
        isSelected && 'bg-accent text-foreground',
      )}
    >
      <MethodBadge method={node.method} subtle />
      <span className="min-w-0 flex-1 truncate text-[13px]">{node.name}</span>
      {isOpen && <span className="size-2 rounded-full bg-primary" />}
    </button>
  )
}

interface RequestWorkspaceProps {
  activeDraft: RequestEditorDraft | null
  activeEditorTab: EditorPanelTab
  activeRequestId: string | null
  activeRequestIsLoading: boolean
  activeResponse: ResponseState | null
  dirtyRequestIds: Set<string>
  isBusy: boolean
  isDraggingSplit: boolean
  openRequestTabs: OpenRequestTab[]
  pendingCloseRequestId: string | null
  splitContainerRef: RefObject<HTMLDivElement | null>
  splitRatio: number
  onActiveEditorTabChange: (tab: EditorPanelTab) => void
  onChangeDraft: (updater: (draft: RequestEditorDraft) => RequestEditorDraft) => void
  onCloseRequestDialogChange: (requestId: string | null) => void
  onCloseRequestTab: (requestId: string) => void
  onConfirmCloseRequestTab: () => void
  onFocusRequestTab: (requestId: string) => void
  onSaveRequest: () => void
  onSendRequest: () => void
  onStartDraggingSplit: () => void
}

function RequestWorkspace(props: RequestWorkspaceProps) {
  const hasUnsavedChanges = Boolean(props.activeRequestId && props.dirtyRequestIds.has(props.activeRequestId))

  return (
    <section className="flex min-h-0 flex-col bg-background">
      <RequestTabsBar
        activeRequestId={props.activeRequestId}
        openRequestTabs={props.openRequestTabs}
        onCloseRequestTab={props.onCloseRequestTab}
        onFocusRequestTab={props.onFocusRequestTab}
      />

      {props.activeRequestIsLoading && !props.activeDraft
        ? (
            <div className="grid flex-1 place-items-center">
              <div className="flex items-center gap-3 text-sm text-muted-foreground">
                <Loader2Icon className="size-4 animate-spin" />
                正在读取请求详情...
              </div>
            </div>
          )
        : props.activeDraft
          ? (
              <>
                <RequestHeaderBar
                  draft={props.activeDraft}
                  hasUnsavedChanges={hasUnsavedChanges}
                  isBusy={props.isBusy}
                  onChangeDraft={props.onChangeDraft}
                  onSaveRequest={props.onSaveRequest}
                  onSendRequest={props.onSendRequest}
                />

                <div
                  ref={props.splitContainerRef}
                  className="grid min-h-0 flex-1"
                  style={{
                    gridTemplateRows: `minmax(250px, ${props.splitRatio}fr) 8px minmax(200px, ${1 - props.splitRatio}fr)`,
                  }}
                >
                  <RequestEditorTabs
                    activeTab={props.activeEditorTab}
                    draft={props.activeDraft}
                    onActiveTabChange={props.onActiveEditorTabChange}
                    onChangeDraft={props.onChangeDraft}
                  />

                  <button
                    type="button"
                    aria-label="Resize request and response panes"
                    onMouseDown={props.onStartDraggingSplit}
                    className={cn(
                      'group relative mx-3 my-0.5 rounded-full bg-transparent transition',
                      props.isDraggingSplit ? 'cursor-row-resize' : 'cursor-row-resize hover:bg-accent/50',
                    )}
                  >
                    <span className="absolute inset-x-0 top-1/2 h-px -translate-y-1/2 bg-border" />
                    <span className="absolute left-1/2 top-1/2 h-1.5 w-14 -translate-x-1/2 -translate-y-1/2 rounded-full bg-border transition group-hover:bg-primary/40" />
                  </button>

                  <ResponsePane response={props.activeResponse} />
                </div>
              </>
            )
          : (
              <div className="grid flex-1 place-items-center">
                <div className="max-w-md text-center">
                  <p className="text-base font-medium">打开一个请求开始工作</p>
                  <p className="mt-1.5 text-sm leading-6 text-muted-foreground">
                    左侧点击请求节点会在这里打开一个持久化标签页。编辑后的草稿会保留在本地内存，保存后再写回工作空间文件。
                  </p>
                </div>
              </div>
            )}

      <AlertDialog open={Boolean(props.pendingCloseRequestId)} onOpenChange={open => !open && props.onCloseRequestDialogChange(null)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>关闭未保存的请求？</AlertDialogTitle>
            <AlertDialogDescription>
              这个请求标签还有未保存的修改。确认关闭后，本次草稿会被丢弃。
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel onClick={() => props.onCloseRequestDialogChange(null)}>
              继续编辑
            </AlertDialogCancel>
            <AlertDialogAction variant="destructive" onClick={props.onConfirmCloseRequestTab}>
              关闭标签
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </section>
  )
}

interface RequestTabsBarProps {
  activeRequestId: string | null
  openRequestTabs: OpenRequestTab[]
  onCloseRequestTab: (requestId: string) => void
  onFocusRequestTab: (requestId: string) => void
}

function RequestTabsBar(props: RequestTabsBarProps) {
  return (
    <div className="flex min-h-10 items-center gap-1.5 overflow-x-auto border-b border-border/70 px-3 py-1.5">
      {props.openRequestTabs.length > 0
        ? props.openRequestTabs.map(tab => (
            <div
              key={tab.requestId}
              className={cn(
                'group flex min-w-[168px] items-center gap-1.5 rounded-lg border px-2.5 py-1.5 transition',
                props.activeRequestId === tab.requestId
                  ? 'border-primary/40 bg-primary/5'
                  : 'border-transparent bg-muted/50 hover:border-border/80 hover:bg-accent',
              )}
            >
              <button
                type="button"
                onClick={() => props.onFocusRequestTab(tab.requestId)}
                className="flex min-w-0 flex-1 items-center gap-1.5 text-left"
              >
                <MethodBadge method={tab.method} subtle />
                <span className="min-w-0 truncate text-[13px] font-medium">{tab.title}</span>
                {tab.dirty && <span className="size-2 rounded-full bg-amber-500" />}
              </button>
              <button
                type="button"
                onClick={() => props.onCloseRequestTab(tab.requestId)}
                className="rounded-md p-1 text-muted-foreground opacity-0 transition group-hover:opacity-100 hover:bg-background hover:text-foreground"
              >
                <XIcon className="size-4" />
              </button>
            </div>
          ))
        : <span className="text-[13px] text-muted-foreground">No request tabs open</span>}
    </div>
  )
}

interface RequestHeaderBarProps {
  draft: RequestEditorDraft
  hasUnsavedChanges: boolean
  isBusy: boolean
  onChangeDraft: (updater: (draft: RequestEditorDraft) => RequestEditorDraft) => void
  onSaveRequest: () => void
  onSendRequest: () => void
}

function RequestHeaderBar(props: RequestHeaderBarProps) {
  return (
    <div className="border-b border-border/70 px-3 py-3">
      <div className="mb-2 flex items-center justify-between gap-3">
        <div>
          <p className="text-base font-semibold tracking-tight">{props.draft.name}</p>
          <p className="mt-0.5 text-xs text-muted-foreground">{props.draft.slug}</p>
        </div>
        <span
          className={cn(
            'rounded-full px-2 py-0.5 text-[11px] font-medium',
            props.hasUnsavedChanges
              ? 'bg-amber-500/10 text-amber-600 dark:text-amber-300'
              : 'bg-emerald-500/10 text-emerald-600 dark:text-emerald-300',
          )}
        >
          {props.hasUnsavedChanges ? 'Unsaved' : 'Saved'}
        </span>
      </div>

      <div className="flex flex-wrap items-center gap-2 xl:flex-nowrap">
        <Select
          value={props.draft.method}
          onValueChange={(value) => {
            if (value) {
              props.onChangeDraft(draft => ({ ...draft, method: value }))
            }
          }}
        >
          <SelectTrigger size="sm" className="w-[112px]">
            <SelectValue placeholder="Method" />
          </SelectTrigger>
          <SelectContent>
            {methodOptions.map(method => (
              <SelectItem key={method} value={method}>
                {method}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>

        <Input
          className="h-8 min-w-[280px] flex-1 text-sm"
          value={props.draft.url}
          onChange={event => props.onChangeDraft(draft => ({ ...draft, url: event.target.value }))}
          placeholder="https://api.example.com/v1/users"
        />

        <Button size="sm" disabled={props.isBusy} onClick={props.onSendRequest}>
          <SendHorizonalIcon />
          Send
        </Button>
        <Button size="sm" disabled={props.isBusy || !props.hasUnsavedChanges} variant="outline" onClick={props.onSaveRequest}>
          <SaveIcon />
          Save
        </Button>
      </div>
    </div>
  )
}

interface RequestEditorTabsProps {
  activeTab: EditorPanelTab
  draft: RequestEditorDraft
  onActiveTabChange: (tab: EditorPanelTab) => void
  onChangeDraft: (updater: (draft: RequestEditorDraft) => RequestEditorDraft) => void
}

function RequestEditorTabs(props: RequestEditorTabsProps) {
  return (
    <div className="min-h-0 overflow-hidden border-b border-border/70 px-3 py-3">
      <Tabs value={props.activeTab} onValueChange={value => props.onActiveTabChange(value as EditorPanelTab)} className="h-full">
        <TabsList variant="line" className="mb-3">
          {editorTabs.map(tab => (
            <TabsTrigger key={tab.value} value={tab.value} className="px-1.5 py-0.5 text-[13px]">
              {tab.label}
            </TabsTrigger>
          ))}
        </TabsList>

        <TabsContent value="query" className="min-h-0">
          <KeyValueTable
            emptyLabel="No query parameters"
            rows={props.draft.request.query}
            onAdd={() => props.onChangeDraft(draft => ({
              ...draft,
              request: {
                ...draft.request,
                query: [...draft.request.query, createKeyValueDraft()],
              },
            }))}
            onChange={(rows) => {
              props.onChangeDraft(draft => ({
                ...draft,
                request: { ...draft.request, query: rows },
              }))
            }}
          />
        </TabsContent>

        <TabsContent value="headers" className="min-h-0">
          <KeyValueTable
            emptyLabel="No request headers"
            rows={props.draft.request.headers}
            onAdd={() => props.onChangeDraft(draft => ({
              ...draft,
              request: {
                ...draft.request,
                headers: [...draft.request.headers, createKeyValueDraft()],
              },
            }))}
            onChange={(rows) => {
              props.onChangeDraft(draft => ({
                ...draft,
                request: { ...draft.request, headers: rows },
              }))
            }}
          />
        </TabsContent>

        <TabsContent value="body" className="min-h-0">
          <BodyEditor draft={props.draft} onChangeDraft={props.onChangeDraft} />
        </TabsContent>

        <TabsContent value="docs" className="min-h-0">
          <DocsEditor draft={props.draft} onChangeDraft={props.onChangeDraft} />
        </TabsContent>

        <TabsContent value="mock" className="min-h-0">
          <MockEditor draft={props.draft} onChangeDraft={props.onChangeDraft} />
        </TabsContent>
      </Tabs>
    </div>
  )
}

interface ResponsePaneProps {
  response: ResponseState | null
}

function ResponsePane(props: ResponsePaneProps) {
  if (!props.response) {
    return (
      <div className="grid min-h-0 place-items-center px-3 py-5">
        <div className="max-w-md text-center">
          <p className="text-sm font-medium">响应会在这里出现</p>
          <p className="mt-1.5 text-sm leading-6 text-muted-foreground">
            发送请求后，这里会展示状态码、耗时、响应大小和自动识别的内容视图。
          </p>
        </div>
      </div>
    )
  }

  return (
    <div className="flex min-h-0 flex-col">
      <div className="flex flex-wrap items-center gap-1.5 border-b border-border/70 px-3 py-2 text-[11px] text-muted-foreground">
        <ResponseMetaBadge label="Status" value={props.response.status !== null ? String(props.response.status) : 'Pending'} />
        <ResponseMetaBadge label="Duration" value={`${props.response.durationMs} ms`} />
        <ResponseMetaBadge label="Size" value={formatBytes(props.response.sizeBytes)} />
        <ResponseMetaBadge label="Content-Type" value={props.response.contentType || 'unknown'} />
        {props.response.responseType && <ResponseMetaBadge label="View" value={props.response.responseType.toUpperCase()} />}
      </div>

      <div className="min-h-0 flex-1 overflow-auto px-3 py-3">
        {props.response.isLoading
          ? (
              <div className="grid h-full place-items-center">
                <div className="flex items-center gap-3 text-sm text-muted-foreground">
                  <Loader2Icon className="size-4 animate-spin" />
                  请求发送中...
                </div>
              </div>
            )
          : props.response.error
            ? (
                <div className="rounded-xl border border-destructive/20 bg-destructive/5 p-3 text-sm text-destructive">
                  {props.response.error}
                </div>
              )
            : props.response.responseType === 'json'
              ? <JsonResponseView body={props.response.body} />
              : <TextResponseView body={props.response.body} />}
      </div>
    </div>
  )
}

function JsonResponseView(props: { body: string }) {
  return (
    <pre className="overflow-auto rounded-xl border border-border/70 bg-muted/40 p-3 font-mono text-xs leading-5">
      {props.body}
    </pre>
  )
}

function TextResponseView(props: { body: string }) {
  return (
    <pre className="overflow-auto rounded-xl border border-border/70 bg-muted/40 p-3 font-mono text-xs leading-5 whitespace-pre-wrap break-words">
      {props.body || 'No response body'}
    </pre>
  )
}

interface KeyValueTableProps {
  emptyLabel: string
  rows: KeyValue[]
  onAdd: () => void
  onChange: (rows: KeyValue[]) => void
}

function KeyValueTable(props: KeyValueTableProps) {
  function updateRow(rowId: string, updater: (row: KeyValue) => KeyValue) {
    props.onChange(props.rows.map(row => (row.id === rowId ? updater(row) : row)))
  }

  function removeRow(rowId: string) {
    props.onChange(props.rows.filter(row => row.id !== rowId))
  }

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="mb-2 flex items-center justify-between">
        <p className="text-[13px] text-muted-foreground">{props.emptyLabel}</p>
        <Button size="xs" variant="outline" onClick={props.onAdd}>
          <PlusIcon />
          Add row
        </Button>
      </div>

      <div className="min-h-0 space-y-1.5 overflow-auto pr-1">
        {props.rows.length > 0
          ? props.rows.map(row => (
              <div
                key={row.id}
                className="grid grid-cols-[28px_minmax(0,_1fr)_minmax(0,_1fr)_32px] items-center gap-1.5 rounded-xl border border-border/70 bg-muted/30 p-1.5"
              >
                <div className="flex items-center justify-center">
                  <Checkbox checked={row.enabled} onCheckedChange={checked => updateRow(row.id, current => ({ ...current, enabled: Boolean(checked) }))} />
                </div>
                <Input
                  value={row.key}
                  onChange={event => updateRow(row.id, current => ({ ...current, key: event.target.value }))}
                  placeholder="Key"
                />
                <Input
                  value={row.value}
                  onChange={event => updateRow(row.id, current => ({ ...current, value: event.target.value }))}
                  placeholder="Value"
                />
                <button
                  type="button"
                  onClick={() => removeRow(row.id)}
                  className="rounded-md p-1.5 text-muted-foreground transition hover:bg-background hover:text-foreground"
                >
                  <XIcon className="size-4" />
                </button>
              </div>
            ))
          : (
              <div className="grid min-h-[100px] place-items-center rounded-xl border border-dashed border-border/70 text-sm text-muted-foreground">
                {props.emptyLabel}
              </div>
            )}
      </div>
    </div>
  )
}

function BodyEditor(props: {
  draft: RequestEditorDraft
  onChangeDraft: (updater: (draft: RequestEditorDraft) => RequestEditorDraft) => void
}) {
  const body = props.draft.request.body

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <div className="flex items-center gap-2">
        <span className="text-[13px] text-muted-foreground">Body mode</span>
        <Select
          value={body.mode}
          onValueChange={(value) => {
            if (value) {
              props.onChangeDraft(draft => ({
                ...draft,
                request: {
                  ...draft.request,
                  body: { ...draft.request.body, mode: value as typeof body.mode },
                },
              }))
            }
          }}
        >
          <SelectTrigger size="sm" className="w-[164px]">
            <SelectValue placeholder="Choose mode" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="none">none</SelectItem>
            <SelectItem value="raw">raw</SelectItem>
            <SelectItem value="json">json</SelectItem>
          </SelectContent>
        </Select>
      </div>

      {body.mode === 'none' && (
        <div className="grid min-h-[140px] place-items-center rounded-xl border border-dashed border-border/70 text-sm text-muted-foreground">
          This request currently sends no body.
        </div>
      )}

      {body.mode === 'raw' && (
        <Textarea
          className="min-h-[220px] flex-1 font-mono text-xs"
          value={body.raw}
          onChange={event => props.onChangeDraft(draft => ({
            ...draft,
            request: {
              ...draft.request,
              body: { ...draft.request.body, raw: event.target.value },
            },
          }))}
          placeholder="Raw request body"
        />
      )}

      {body.mode === 'json' && (
        <Textarea
          className="min-h-[220px] flex-1 font-mono text-xs"
          value={body.json}
          onChange={event => props.onChangeDraft(draft => ({
            ...draft,
            request: {
              ...draft.request,
              body: { ...draft.request.body, json: event.target.value },
            },
          }))}
          placeholder={'{\n  "message": "hello"\n}'}
        />
      )}
    </div>
  )
}

function DocsEditor(props: {
  draft: RequestEditorDraft
  onChangeDraft: (updater: (draft: RequestEditorDraft) => RequestEditorDraft) => void
}) {
  const docs = props.draft.documentation

  return (
    <div className="grid h-full min-h-0 gap-3 lg:grid-cols-2">
      <div className="space-y-2.5">
        <LabelBlock label="Summary">
          <Input
            value={docs.summary}
            onChange={event => props.onChangeDraft(draft => ({
              ...draft,
              documentation: { ...draft.documentation, summary: event.target.value },
            }))}
            placeholder="What this endpoint does"
          />
        </LabelBlock>
        <LabelBlock label="Operation ID">
          <Input
            value={docs.operationId}
            onChange={event => props.onChangeDraft(draft => ({
              ...draft,
              documentation: { ...draft.documentation, operationId: event.target.value },
            }))}
            placeholder="getUserProfile"
          />
        </LabelBlock>
        <LabelBlock label="Group Name">
          <Input
            value={docs.groupName}
            onChange={event => props.onChangeDraft(draft => ({
              ...draft,
              documentation: { ...draft.documentation, groupName: event.target.value },
            }))}
            placeholder="User"
          />
        </LabelBlock>
        <div className="flex items-center justify-between rounded-xl border border-border/70 bg-muted/30 px-3 py-2.5">
          <div>
            <p className="text-[13px] font-medium">Deprecated</p>
            <p className="text-xs text-muted-foreground">Mark this request as legacy or scheduled for removal.</p>
          </div>
          <Switch
            checked={docs.deprecated}
            onCheckedChange={checked => props.onChangeDraft(draft => ({
              ...draft,
              documentation: { ...draft.documentation, deprecated: Boolean(checked) },
            }))}
          />
        </div>
      </div>
      <LabelBlock label="Description" className="h-full">
        <Textarea
          className="min-h-[220px] h-full font-mono text-xs"
          value={docs.description}
          onChange={event => props.onChangeDraft(draft => ({
            ...draft,
            documentation: { ...draft.documentation, description: event.target.value },
          }))}
          placeholder="Write endpoint usage notes, constraints, and examples."
        />
      </LabelBlock>
    </div>
  )
}

function MockEditor(props: {
  draft: RequestEditorDraft
  onChangeDraft: (updater: (draft: RequestEditorDraft) => RequestEditorDraft) => void
}) {
  const mock = props.draft.mock
  const mockBodyText = serializeMockBody(mock.body)

  return (
    <div className="grid h-full min-h-0 gap-3 lg:grid-cols-[300px_1fr]">
      <div className="space-y-3">
        <div className="flex items-center justify-between rounded-xl border border-border/70 bg-muted/30 px-3 py-2.5">
          <div>
            <p className="text-[13px] font-medium">Mock enabled</p>
            <p className="text-xs text-muted-foreground">Use this saved response for future mock features.</p>
          </div>
          <Switch
            checked={mock.enabled}
            onCheckedChange={checked => props.onChangeDraft(draft => ({
              ...draft,
              mock: { ...draft.mock, enabled: Boolean(checked) },
            }))}
          />
        </div>

        <LabelBlock label="Status">
          <Input
            type="number"
            value={String(mock.status)}
            onChange={event => props.onChangeDraft(draft => ({
              ...draft,
              mock: { ...draft.mock, status: Number(event.target.value || 200) },
            }))}
          />
        </LabelBlock>

        <LabelBlock label="Latency (ms)">
          <Input
            type="number"
            value={String(mock.latencyMs)}
            onChange={event => props.onChangeDraft(draft => ({
              ...draft,
              mock: { ...draft.mock, latencyMs: Number(event.target.value || 0) },
            }))}
          />
        </LabelBlock>

        <LabelBlock label="Content-Type">
          <Input
            value={mock.contentType}
            onChange={event => props.onChangeDraft(draft => ({
              ...draft,
              mock: { ...draft.mock, contentType: event.target.value },
            }))}
            placeholder="application/json"
          />
        </LabelBlock>
      </div>

      <LabelBlock label="Mock Body" className="h-full">
        <Textarea
          className="min-h-[220px] h-full font-mono text-xs"
          value={mockBodyText}
          onChange={event => props.onChangeDraft(draft => ({
            ...draft,
            mock: { ...draft.mock, body: parseMockBody(event.target.value) },
          }))}
          placeholder={'{\n  "ok": true\n}'}
        />
      </LabelBlock>
    </div>
  )
}

function CreateProjectDialog(props: {
  description: string
  name: string
  open: boolean
  onDescriptionChange: (value: string) => void
  onNameChange: (value: string) => void
  onOpenChange: (open: boolean) => void
  onSubmit: () => void
}) {
  return (
    <Dialog open={props.open} onOpenChange={props.onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Create Project</DialogTitle>
          <DialogDescription>Projects appear in the left selector and own their own request tree.</DialogDescription>
        </DialogHeader>
        <div className="space-y-3">
          <LabelBlock label="Project name">
            <Input value={props.name} onChange={event => props.onNameChange(event.target.value)} placeholder="Core APIs" />
          </LabelBlock>
          <LabelBlock label="Description">
            <Textarea value={props.description} onChange={event => props.onDescriptionChange(event.target.value)} placeholder="Optional notes for this project." />
          </LabelBlock>
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={() => props.onOpenChange(false)}>Cancel</Button>
          <Button onClick={props.onSubmit}>Create Project</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

function CreateCollectionDialog(props: {
  name: string
  open: boolean
  parentLabel: string
  onNameChange: (value: string) => void
  onOpenChange: (open: boolean) => void
  onSubmit: () => void
}) {
  return (
    <Dialog open={props.open} onOpenChange={props.onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Create Collection</DialogTitle>
          <DialogDescription>
            The new collection will be created under
            {' '}
            <span className="font-medium text-foreground">{props.parentLabel}</span>
            .
          </DialogDescription>
        </DialogHeader>
        <LabelBlock label="Collection name">
          <Input value={props.name} onChange={event => props.onNameChange(event.target.value)} placeholder="Auth" />
        </LabelBlock>
        <DialogFooter>
          <Button variant="outline" onClick={() => props.onOpenChange(false)}>Cancel</Button>
          <Button onClick={props.onSubmit}>Create Collection</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

function CreateRequestDialog(props: {
  method: string
  name: string
  open: boolean
  parentLabel: string
  url: string
  onMethodChange: (value: string) => void
  onNameChange: (value: string) => void
  onOpenChange: (open: boolean) => void
  onSubmit: () => void
  onUrlChange: (value: string) => void
}) {
  return (
    <Dialog open={props.open} onOpenChange={props.onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Create Request</DialogTitle>
          <DialogDescription>
            The request will be created under
            {' '}
            <span className="font-medium text-foreground">{props.parentLabel}</span>
            .
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-3">
          <LabelBlock label="Request name">
            <Input value={props.name} onChange={event => props.onNameChange(event.target.value)} placeholder="Health Check" />
          </LabelBlock>
          <div className="grid gap-3 sm:grid-cols-[128px_1fr]">
            <LabelBlock label="Method">
              <Select
                value={props.method}
                onValueChange={(value) => {
                  if (value) {
                    props.onMethodChange(value)
                  }
                }}
              >
                <SelectTrigger className="w-full">
                  <SelectValue placeholder="Method" />
                </SelectTrigger>
                <SelectContent>
                  {methodOptions.map(method => (
                    <SelectItem key={method} value={method}>
                      {method}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </LabelBlock>
            <LabelBlock label="URL">
              <Input value={props.url} onChange={event => props.onUrlChange(event.target.value)} placeholder="https://api.example.com/health" />
            </LabelBlock>
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={() => props.onOpenChange(false)}>Cancel</Button>
          <Button onClick={props.onSubmit}>Create Request</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

function MethodBadge(props: { method: string, subtle?: boolean }) {
  return (
    <span
      className={cn(
        'inline-flex h-5 items-center rounded-full px-1.5 text-[10px] font-semibold uppercase tracking-[0.14em]',
        props.subtle ? 'bg-primary/10 text-primary' : 'bg-primary text-primary-foreground',
      )}
    >
      {props.method}
    </span>
  )
}

function ResponseMetaBadge(props: { label: string, value: string }) {
  return (
    <span className="inline-flex items-center gap-1 rounded-full border border-border/70 bg-muted/40 px-2 py-0.5">
      <span className="text-muted-foreground">
        {props.label}
        :
      </span>
      <span className="font-medium text-foreground">{props.value}</span>
    </span>
  )
}

function LabelBlock(props: { children: ReactNode, className?: string, label: string }) {
  return (
    <div className={cn('space-y-1.5', props.className)}>
      <label className="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
        {props.label}
      </label>
      {props.children}
    </div>
  )
}

function SidebarEmptyState(props: { title: string, body: string }) {
  return (
    <div className="rounded-xl border border-dashed border-border/70 px-3 py-5 text-center">
      <p className="font-medium">{props.title}</p>
      <p className="mt-1.5 text-sm leading-6 text-muted-foreground">{props.body}</p>
    </div>
  )
}

function getActiveProject(workspace: WorkspaceSnapshot | null, activeProjectId: string | null) {
  if (!workspace) {
    return null
  }

  return workspace.projects.find(project => project.metadata.id === activeProjectId)
    ?? workspace.projects.find(project => project.metadata.id === workspace.lastOpenedProjectId)
    ?? workspace.projects[0]
    ?? null
}

function collectCollectionIds(nodes: TreeNode[]): string[] {
  return nodes.flatMap((node) => {
    if (node.entityType !== 'collection') {
      return []
    }
    return [node.id, ...collectCollectionIds(node.children)]
  })
}

function findCollectionName(nodes: TreeNode[], collectionId: string): string {
  for (const node of nodes) {
    if (node.entityType !== 'collection') {
      continue
    }
    if (node.id === collectionId) {
      return node.name
    }
    const nested = findCollectionName(node.children, collectionId)
    if (nested) {
      return nested
    }
  }
  return ''
}

function describeCollectionParent(parentCollectionId: string | null, collectionNames: Map<string, string>) {
  if (!parentCollectionId) {
    return 'project root'
  }
  return collectionNames.get(parentCollectionId) ?? 'selected collection'
}

function cloneApiDefinition(api: ApiDefinition): ApiDefinition {
  return JSON.parse(JSON.stringify(api)) as ApiDefinition
}

function areApiDefinitionsEqual(left: ApiDefinition, right: ApiDefinition) {
  return JSON.stringify(left) === JSON.stringify(right)
}

function createKeyValueDraft(): KeyValue {
  return {
    id: globalThis.crypto?.randomUUID?.() ?? `kv-${Date.now()}-${Math.random().toString(16).slice(2)}`,
    key: '',
    value: '',
    enabled: true,
    description: '',
  }
}

function createEmptyResponseState(): ResponseState {
  return {
    status: null,
    headers: [],
    durationMs: 0,
    sizeBytes: 0,
    contentType: '',
    responseType: null,
    body: '',
    isLoading: false,
    error: null,
  }
}

function mapSendResponseToState(response: SendRequestResponse): ResponseState {
  return {
    status: response.status,
    headers: response.headers,
    durationMs: response.durationMs,
    sizeBytes: response.sizeBytes,
    contentType: response.contentType,
    responseType: response.responseType,
    body: response.body,
    isLoading: false,
    error: null,
  }
}

function serializeMockBody(body: unknown) {
  if (typeof body === 'string') {
    return body
  }
  if (body === null || body === undefined) {
    return ''
  }
  return JSON.stringify(body, null, 2)
}

function parseMockBody(value: string): unknown {
  const trimmed = value.trim()
  if (!trimmed) {
    return null
  }
  try {
    return JSON.parse(trimmed)
  }
  catch {
    return value
  }
}

function clamp(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value))
}

function formatBytes(sizeBytes: number) {
  if (sizeBytes < 1024) {
    return `${sizeBytes} B`
  }
  if (sizeBytes < 1024 * 1024) {
    return `${(sizeBytes / 1024).toFixed(1)} KB`
  }
  return `${(sizeBytes / (1024 * 1024)).toFixed(2)} MB`
}

function findApiLocation(
  workspace: WorkspaceSnapshot,
  apiId: string,
): { projectId: string, parentCollectionId: string | null, summary: ApiSummary } | null {
  for (const project of workspace.projects) {
    const location = findApiLocationInProject(project.children, apiId)
    if (location) {
      return {
        projectId: project.metadata.id,
        parentCollectionId: location.parentCollectionId,
        summary: location.summary,
      }
    }
  }
  return null
}

function findApiLocationInProject(
  nodes: TreeNode[],
  apiId: string,
  parentCollectionId: string | null = null,
): { parentCollectionId: string | null, summary: ApiSummary } | null {
  for (const node of nodes) {
    if (node.entityType === 'api') {
      if (node.id === apiId) {
        return {
          parentCollectionId,
          summary: node,
        }
      }
      continue
    }

    const nested = findApiLocationInProject(node.children, apiId, node.id)
    if (nested) {
      return nested
    }
  }
  return null
}
