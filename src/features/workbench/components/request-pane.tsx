import type {
  CollectionEditorDraft,
  EditorPanelTab,
  EnvironmentEditorDraft,
  OpenRequestTab,
  ProjectEditorDraft,
  RequestEditorDraft,
  ResponseState,
  SettingsPanelTab,
  WebSocketSessionState,
  WorkbenchPanelTab,
} from '../types'
import type { AuthType, Environment, FormDataEntry, KeyValue, RequestScopeConfig, WebSocketMessageFormat } from '@/lib/project'
import { PointerActivationConstraints } from '@dnd-kit/dom'
import {
  DragDropProvider,
  DragOverlay,
  PointerSensor,
  useDraggable,
} from '@dnd-kit/react'
import { open } from '@tauri-apps/plugin-dialog'
import { BracesIcon, CompassIcon, EllipsisIcon, PlusIcon, SaveIcon, SendHorizonalIcon, XIcon } from 'lucide-react'
import { AnimatePresence, motion } from 'motion/react'
import * as React from 'react'
import { toast } from 'sonner'
import { MonacoCodeEditor, MonacoJsonEditor, MonacoScriptEditor } from '@/components/monaco-editor'
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
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuTrigger,
} from '@/components/ui/context-menu'
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from '@/components/ui/empty'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from '@/components/ui/resizable'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Table, TableBody, TableCell, TableRow } from '@/components/ui/table'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Textarea } from '@/components/ui/textarea'
import { cn } from '@/lib/utils'
import { useWorkbenchStore } from '../store/workbench-store'
import {
  editorTabs,
  macOSWindowChromeHeightClassName,
  methodOptions,
  settingsTabs,
} from '../types'
import {
  buildUrlWithQuery,
  createKeyValueDraft,
  formatBytes,
  hasVisibleResponse,
  isValidRequestUrl,
  isValidWebSocketUrl,
  normalizeRequestUrl,
  parseUrlQueryParams,
  startWindowDragging,
} from '../utils'
import { EnvironmentVariableInput } from './environment-variable-input'
import { MethodBadge, ResponseMetaBadge, Spinner } from './shared'

interface RequestPaneProps {
  activeDraft: RequestEditorDraft | null
  activeCollectionDraft: CollectionEditorDraft | null
  activeEnvironmentDraft: EnvironmentEditorDraft | null
  activeEnvironmentId: string | null
  activeProjectDraft: ProjectEditorDraft | null
  activeTabRecord: OpenRequestTab | null
  activeEditorTab: WorkbenchPanelTab
  activeRequestId: string | null
  activeRequestIsLoading: boolean
  activeResponse: ResponseState | null
  activeWebSocketSession: WebSocketSessionState | null
  dirtyRequestIds: Set<string>
  environmentDrafts: Record<string, EnvironmentEditorDraft>
  environments: Environment[]
  isBusy: boolean
  isMacOSDesktop: boolean
  openRequestTabs: OpenRequestTab[]
  pendingCloseRequestId: string | null
  splitRatio: number
  onActiveEditorTabChange: (tab: WorkbenchPanelTab) => void
  onChangeDraft: (updater: (draft: RequestEditorDraft) => RequestEditorDraft) => void
  onChangeCollectionDraft: (updater: (draft: CollectionEditorDraft) => CollectionEditorDraft) => void
  onChangeEnvironmentDraft: (updater: (draft: EnvironmentEditorDraft) => EnvironmentEditorDraft) => void
  onChangeProjectDraft: (updater: (draft: ProjectEditorDraft) => ProjectEditorDraft) => void
  onCloseRequestDialogChange: (requestId: string | null) => void
  onCloseRequestTab: (requestId: string) => void
  onConfirmCloseRequestTab: () => void
  onFocusRequestTab: (requestId: string) => void
  onReorderRequestTabs: (tabs: OpenRequestTab[]) => void
  onSaveRequest: () => void
  onSaveCollection: () => void
  onSaveEnvironment: () => void
  onSaveProject: () => void
  onSendRequest: () => void
  onConnectWebSocket: () => void
  onDisconnectWebSocket: () => void
  onSendWebSocketMessage: () => void
  onWebSocketDraftMessageChange: (requestId: string, value: string) => void
  onWebSocketMessageFormatChange: (requestId: string, format: WebSocketMessageFormat) => void
  onDeleteEnvironment: (environmentId: string) => void
  onSetActiveEnvironment: (environmentId: string | null) => void
  onSelectEnvironmentForTab: (environmentId: string | null) => void
  onStartCreateEnvironment: () => void
  onSplitRatioChange: (value: number) => void
}

export function RequestPane(props: RequestPaneProps) {
  const hasUnsavedChanges = Boolean(props.activeTabRecord && props.dirtyRequestIds.has(props.activeTabRecord.entityId))
  const shouldShowResponsePane = props.activeResponse ? hasVisibleResponse(props.activeResponse) : false
  const handleSaveShortcut = React.useEffectEvent(() => {
    if (!props.activeTabRecord || props.isBusy || !hasUnsavedChanges) {
      return
    }

    if (props.activeTabRecord.entityType === 'request') {
      props.onSaveRequest()
      return
    }

    if (props.activeTabRecord.entityType === 'collection') {
      props.onSaveCollection()
      return
    }

    if (props.activeTabRecord.entityType === 'environment') {
      props.onSaveEnvironment()
      return
    }

    props.onSaveProject()
  })

  // Refs to prevent circular updates between URL and query params sync
  const isUpdatingFromUrlRef = React.useRef(false)
  const isUpdatingFromQueryRef = React.useRef(false)

  // Handler for URL changes that also syncs to query params
  const handleUrlChange = React.useCallback((newUrl: string) => {
    if (isUpdatingFromQueryRef.current) {
      // Skip - this update is coming from query params
      return
    }

    isUpdatingFromUrlRef.current = true
    try {
      props.onChangeDraft((draft) => {
        const newQueryParams = parseUrlQueryParams(newUrl, draft.request.query)
        return {
          ...draft,
          url: newUrl,
          request: {
            ...draft.request,
            query: newQueryParams,
          },
        }
      })
    }
    finally {
      isUpdatingFromUrlRef.current = false
    }
  }, [props.onChangeDraft])

  // Handler for query params changes that also syncs to URL
  const handleQueryChange = React.useCallback((newQuery: KeyValue[]) => {
    if (isUpdatingFromUrlRef.current) {
      // Skip - this update is coming from URL
      return
    }

    isUpdatingFromQueryRef.current = true
    try {
      props.onChangeDraft((draft) => {
        const newUrl = buildUrlWithQuery(draft.url, newQuery)
        return {
          ...draft,
          url: newUrl,
          request: {
            ...draft.request,
            query: newQuery,
          },
        }
      })
    }
    finally {
      isUpdatingFromQueryRef.current = false
    }
  }, [props.onChangeDraft])

  React.useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.repeat || event.altKey || event.shiftKey) {
        return
      }

      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 's') {
        event.preventDefault()
        handleSaveShortcut()
      }
    }

    window.addEventListener('keydown', handleKeyDown)

    return () => {
      window.removeEventListener('keydown', handleKeyDown)
    }
  }, [handleSaveShortcut])

  return (
    <section className="flex min-h-0 min-w-0 flex-col overflow-hidden bg-background">
      <RequestTabsBar
        activeRequestId={props.activeRequestId}
        isMacOSDesktop={props.isMacOSDesktop}
        openRequestTabs={props.openRequestTabs}
        onCloseRequestTab={props.onCloseRequestTab}
        onFocusRequestTab={props.onFocusRequestTab}
        onReorderRequestTabs={props.onReorderRequestTabs}
      />

      {props.activeRequestIsLoading && !props.activeDraft
        ? (
            <div className="grid flex-1 place-items-center">
              <div className="flex items-center gap-3 text-sm text-muted-foreground">
                <Spinner />
                正在读取请求详情...
              </div>
            </div>
          )
        : props.activeTabRecord?.entityType === 'request' && props.activeDraft
          ? (
              <>
                {props.activeDraft.protocol === 'websocket'
                  ? (
                      <>
                        <WebSocketHeaderBar
                          draft={props.activeDraft}
                          hasUnsavedChanges={hasUnsavedChanges}
                          isBusy={props.isBusy}
                          session={props.activeWebSocketSession}
                          onChangeDraft={props.onChangeDraft}
                          onUrlChange={handleUrlChange}
                          onSaveRequest={props.onSaveRequest}
                          onConnect={props.onConnectWebSocket}
                          onDisconnect={props.onDisconnectWebSocket}
                        />
                        <ResizablePanelGroup
                          id="websocket-pane-panels"
                          orientation="vertical"
                          className="min-h-0 min-w-0 flex-1 overflow-hidden"
                          onLayoutChanged={(layout) => {
                            const requestEditorSize = layout.requestEditorPanel
                            if (typeof requestEditorSize === 'number') {
                              props.onSplitRatioChange(requestEditorSize / 100)
                            }
                          }}
                        >
                          <ResizablePanel
                            id="requestEditorPanel"
                            className="flex min-h-0 min-w-0 flex-col overflow-hidden"
                            defaultSize={props.splitRatio * 100}
                            minSize={28}
                          >
                            <RequestEditorTabs
                              activeTab={props.activeEditorTab as EditorPanelTab}
                              draft={props.activeDraft}
                              onActiveTabChange={props.onActiveEditorTabChange}
                              onChangeDraft={props.onChangeDraft}
                              onQueryChange={handleQueryChange}
                            />
                          </ResizablePanel>

                          <ResizableHandle withHandle className="mx-3 my-0.5 bg-transparent transition hover:bg-accent/50" />

                          <ResizablePanel
                            id="requestResponsePanel"
                            className="flex min-h-0 min-w-0 flex-col overflow-hidden"
                            defaultSize={(1 - props.splitRatio) * 100}
                            minSize={22}
                          >
                            <WebSocketPane
                              requestId={props.activeDraft.id}
                              session={props.activeWebSocketSession}
                              onDraftMessageChange={props.onWebSocketDraftMessageChange}
                              onMessageFormatChange={props.onWebSocketMessageFormatChange}
                              onSendMessage={props.onSendWebSocketMessage}
                            />
                          </ResizablePanel>
                        </ResizablePanelGroup>
                      </>
                    )
                  : (
                      <>
                        <RequestHeaderBar
                          draft={props.activeDraft}
                          hasUnsavedChanges={hasUnsavedChanges}
                          isBusy={props.isBusy}
                          onChangeDraft={props.onChangeDraft}
                          onUrlChange={handleUrlChange}
                          onSaveRequest={props.onSaveRequest}
                          onSendRequest={props.onSendRequest}
                        />

                        {shouldShowResponsePane
                          ? (
                              <ResizablePanelGroup
                                id="request-pane-panels"
                                orientation="vertical"
                                className="min-h-0 min-w-0 flex-1 overflow-hidden"
                                onLayoutChanged={(layout) => {
                                  const requestEditorSize = layout.requestEditorPanel
                                  if (typeof requestEditorSize === 'number') {
                                    props.onSplitRatioChange(requestEditorSize / 100)
                                  }
                                }}
                              >
                                <ResizablePanel
                                  id="requestEditorPanel"
                                  className="flex min-h-0 min-w-0 flex-col overflow-hidden"
                                  defaultSize={props.splitRatio * 100}
                                  minSize={28}
                                >
                                  <RequestEditorTabs
                                    activeTab={props.activeEditorTab as EditorPanelTab}
                                    draft={props.activeDraft}
                                    onActiveTabChange={props.onActiveEditorTabChange}
                                    onChangeDraft={props.onChangeDraft}
                                    onQueryChange={handleQueryChange}
                                  />
                                </ResizablePanel>

                                <ResizableHandle withHandle className="mx-3 my-0.5 bg-transparent transition hover:bg-accent/50" />

                                <ResizablePanel
                                  id="requestResponsePanel"
                                  className="flex min-h-0 min-w-0 flex-col overflow-hidden"
                                  defaultSize={(1 - props.splitRatio) * 100}
                                  minSize={22}
                                >
                                  <ResponsePane response={props.activeResponse} />
                                </ResizablePanel>
                              </ResizablePanelGroup>
                            )
                          : (
                              <div className="min-h-0 flex-1 overflow-hidden">
                                <RequestEditorTabs
                                  activeTab={props.activeEditorTab as EditorPanelTab}
                                  draft={props.activeDraft}
                                  onActiveTabChange={props.onActiveEditorTabChange}
                                  onChangeDraft={props.onChangeDraft}
                                  onQueryChange={handleQueryChange}
                                />
                              </div>
                            )}
                      </>
                    )}
              </>
            )
          : props.activeTabRecord?.entityType === 'collection' && props.activeCollectionDraft
            ? (
                <CollectionSettingsPane
                  activeTab={props.activeEditorTab as SettingsPanelTab}
                  draft={props.activeCollectionDraft}
                  hasUnsavedChanges={hasUnsavedChanges}
                  isBusy={props.isBusy}
                  onChangeDraft={props.onChangeCollectionDraft}
                  onActiveTabChange={props.onActiveEditorTabChange}
                  onSave={props.onSaveCollection}
                />
              )
            : props.activeTabRecord?.entityType === 'environment' && props.activeEnvironmentDraft
              ? (
                  <EnvironmentSettingsPane
                    activeTab={props.activeEditorTab as SettingsPanelTab}
                    activeEnvironmentId={props.activeEnvironmentId}
                    environmentDrafts={props.environmentDrafts}
                    environments={props.environments}
                    draft={props.activeEnvironmentDraft}
                    hasUnsavedChanges={hasUnsavedChanges}
                    isBusy={props.isBusy}
                    onActiveTabChange={props.onActiveEditorTabChange}
                    onChangeDraft={props.onChangeEnvironmentDraft}
                    onDeleteEnvironment={props.onDeleteEnvironment}
                    onSave={props.onSaveEnvironment}
                    onSelectEnvironment={props.onSelectEnvironmentForTab}
                    onSetActiveEnvironment={props.onSetActiveEnvironment}
                    onStartCreateEnvironment={props.onStartCreateEnvironment}
                  />
                )
              : props.activeTabRecord?.entityType === 'project' && props.activeProjectDraft
                ? (
                    <ProjectSettingsPane
                      activeTab={props.activeEditorTab as SettingsPanelTab}
                      draft={props.activeProjectDraft}
                      hasUnsavedChanges={hasUnsavedChanges}
                      isBusy={props.isBusy}
                      onChangeDraft={props.onChangeProjectDraft}
                      onActiveTabChange={props.onActiveEditorTabChange}
                      onSave={props.onSaveProject}
                    />
                  )
                : (
                    <div className="grid flex-1 place-items-center">
                      <Empty className="max-w-lg border border-dashed border-border/70 bg-muted/20">
                        <EmptyHeader>
                          <EmptyMedia variant="icon">
                            <CompassIcon />
                          </EmptyMedia>
                          <EmptyTitle>还没打开任何请求</EmptyTitle>
                          <EmptyDescription>
                            左侧随便点一个请求，我们就开工。
                            草稿会先稳稳留在本地，放心改，不会一眨眼跑掉。
                          </EmptyDescription>
                        </EmptyHeader>
                        <EmptyContent className="gap-2 text-xs text-muted-foreground">
                          <div className="rounded-full border border-border/70 bg-background/80 px-3 py-1.5">
                            小提示：双击左侧多开几个请求，对照调试更顺手。
                          </div>
                        </EmptyContent>
                      </Empty>
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
  isMacOSDesktop: boolean
  openRequestTabs: OpenRequestTab[]
  onCloseRequestTab: (requestId: string) => void
  onFocusRequestTab: (requestId: string) => void
  onReorderRequestTabs: (tabs: OpenRequestTab[]) => void
}

const requestTabDefaultWidth = 176
const requestTabMinWidth = 148
const requestTabGap = 6
const requestTabOverflowTriggerWidth = 56
const requestTabsDragType = 'workbench-request-tab'
const requestTabsLayoutTransition = {
  duration: 0.16,
  ease: [0.22, 1, 0.36, 1],
  type: 'tween',
} as const

type RequestTabVisualState = 'idle' | 'selected' | 'preview' | 'overlay'
type RequestTabDropPosition = 'before' | 'after'

interface RequestTabDragItem {
  dirty: boolean
  method: string
  requestId: string
  title: string
  width: number
}

interface RequestTabDragPreview {
  position: RequestTabDropPosition
  targetRequestId: string
}

interface RequestTabHitSnapshot {
  left: number
  midX: number
  requestId: string
  right: number
}

type RequestTabsDragStartEvent = Parameters<NonNullable<React.ComponentProps<typeof DragDropProvider>['onDragStart']>>[0]
type RequestTabsDragMoveEvent = Parameters<NonNullable<React.ComponentProps<typeof DragDropProvider>['onDragMove']>>[0]
type RequestTabsDragEndEvent = Parameters<NonNullable<React.ComponentProps<typeof DragDropProvider>['onDragEnd']>>[0]

interface RequestTabsLayout {
  hiddenTabs: OpenRequestTab[]
  tabWidth: number
  visibleTabs: OpenRequestTab[]
}

type RequestTabChipData = Pick<OpenRequestTab, 'dirty' | 'method' | 'requestId' | 'title'>

function getRequestTabPreviewKey(preview: RequestTabDragPreview | null) {
  if (!preview) {
    return 'none'
  }

  return `${preview.targetRequestId}:${preview.position}`
}

function applyRequestTabMove(
  openRequestTabs: OpenRequestTab[],
  draggedRequestId: string,
  preview: RequestTabDragPreview,
) {
  const sourceIndex = openRequestTabs.findIndex(tab => tab.requestId === draggedRequestId)
  const targetIndex = openRequestTabs.findIndex(tab => tab.requestId === preview.targetRequestId)

  if (sourceIndex < 0 || targetIndex < 0 || draggedRequestId === preview.targetRequestId) {
    return null
  }

  const nextTabs = openRequestTabs.slice()
  const [draggedTab] = nextTabs.splice(sourceIndex, 1)
  if (!draggedTab) {
    return null
  }

  const rawInsertIndex = targetIndex + (preview.position === 'after' ? 1 : 0)
  const insertIndex = sourceIndex < rawInsertIndex ? rawInsertIndex - 1 : rawInsertIndex

  if (insertIndex === sourceIndex) {
    return null
  }

  nextTabs.splice(insertIndex, 0, draggedTab)
  return nextTabs
}

function createRequestTabHitSnapshot(
  tabs: OpenRequestTab[],
  tabElements: Map<string, HTMLDivElement>,
) {
  const snapshots = tabs.flatMap((tab) => {
    const element = tabElements.get(tab.requestId)
    if (!element) {
      return []
    }

    const rect = element.getBoundingClientRect()
    return [{
      left: rect.left,
      midX: rect.left + ((rect.right - rect.left) / 2),
      requestId: tab.requestId,
      right: rect.right,
    } satisfies RequestTabHitSnapshot]
  })

  return snapshots.length > 0 ? snapshots : null
}

function resolveRequestTabPreviewFromPointer(args: {
  clientX: number
  draggedRequestId: string
  hitSnapshot: RequestTabHitSnapshot[]
  openRequestTabs: OpenRequestTab[]
}) {
  const { clientX, draggedRequestId, hitSnapshot, openRequestTabs } = args
  const beforeTarget = hitSnapshot.find(snapshot => clientX < snapshot.midX)
  const fallbackTarget = beforeTarget ?? hitSnapshot.at(-1)

  if (!fallbackTarget) {
    return null
  }

  const preview: RequestTabDragPreview = beforeTarget
    ? { position: 'before', targetRequestId: beforeTarget.requestId }
    : { position: 'after', targetRequestId: fallbackTarget.requestId }

  return applyRequestTabMove(openRequestTabs, draggedRequestId, preview) ? preview : null
}

function getRequestTabsLayout(openRequestTabs: OpenRequestTab[], activeRequestId: string | null, availableWidth: number): RequestTabsLayout {
  if (openRequestTabs.length === 0 || availableWidth <= 0) {
    return {
      hiddenTabs: [],
      tabWidth: requestTabDefaultWidth,
      visibleTabs: openRequestTabs,
    }
  }

  const totalWidthAtDefault = openRequestTabs.length * requestTabDefaultWidth + (openRequestTabs.length - 1) * requestTabGap
  if (totalWidthAtDefault <= availableWidth) {
    return {
      hiddenTabs: [],
      tabWidth: requestTabDefaultWidth,
      visibleTabs: openRequestTabs,
    }
  }

  const totalWidthAtMin = openRequestTabs.length * requestTabMinWidth + (openRequestTabs.length - 1) * requestTabGap
  if (totalWidthAtMin <= availableWidth) {
    const tabWidth = Math.min(
      requestTabDefaultWidth,
      Math.max(
        requestTabMinWidth,
        Math.floor((availableWidth - (openRequestTabs.length - 1) * requestTabGap) / openRequestTabs.length),
      ),
    )

    return {
      hiddenTabs: [],
      tabWidth,
      visibleTabs: openRequestTabs,
    }
  }

  let visibleCount = Math.max(
    1,
    Math.floor((availableWidth - requestTabOverflowTriggerWidth) / (requestTabMinWidth + requestTabGap)),
  )

  visibleCount = Math.min(visibleCount, openRequestTabs.length - 1)

  while (
    visibleCount > 1
    && visibleCount * requestTabMinWidth + visibleCount * requestTabGap + requestTabOverflowTriggerWidth > availableWidth
  ) {
    visibleCount -= 1
  }

  const activeIndex = activeRequestId
    ? openRequestTabs.findIndex(tab => tab.requestId === activeRequestId)
    : -1

  const visibleIndices = new Set<number>()
  for (let index = 0; index < visibleCount; index += 1) {
    visibleIndices.add(index)
  }

  if (activeIndex >= visibleCount && activeIndex >= 0) {
    visibleIndices.delete(visibleCount - 1)
    visibleIndices.add(activeIndex)
  }

  const visibleTabs = openRequestTabs.filter((_, index) => visibleIndices.has(index))
  const hiddenTabs = openRequestTabs.filter((_, index) => !visibleIndices.has(index))
  const tabWidth = Math.min(
    requestTabDefaultWidth,
    Math.max(
      requestTabMinWidth,
      Math.floor((availableWidth - requestTabOverflowTriggerWidth - visibleTabs.length * requestTabGap) / visibleTabs.length),
    ),
  )

  return {
    hiddenTabs,
    tabWidth,
    visibleTabs,
  }
}

function RequestTabsBar(props: RequestTabsBarProps) {
  if (props.openRequestTabs.length === 0) {
    return null
  }

  const [activeDragTab, setActiveDragTab] = React.useState<RequestTabDragItem | null>(null)
  const [activePreview, setActivePreview] = React.useState<RequestTabDragPreview | null>(null)
  const containerRef = React.useRef<HTMLDivElement | null>(null)
  const dragVisibleRequestIdsRef = React.useRef<string[]>([])
  const hitSnapshotRef = React.useRef<RequestTabHitSnapshot[] | null>(null)
  const previewKeyRef = React.useRef(getRequestTabPreviewKey(null))
  const tabElementsRef = React.useRef(new Map<string, HTMLDivElement>())
  const [containerWidth, setContainerWidth] = React.useState(0)

  const updateContainerWidth = React.useCallback(() => {
    const container = containerRef.current
    if (!container) {
      return
    }

    const nextWidth = Math.floor(container.getBoundingClientRect().width)
    setContainerWidth(currentWidth => (currentWidth === nextWidth ? currentWidth : nextWidth))
  }, [])

  React.useLayoutEffect(() => {
    updateContainerWidth()

    const frameId = window.requestAnimationFrame(() => {
      updateContainerWidth()
    })

    const container = containerRef.current
    if (!container || typeof ResizeObserver === 'undefined') {
      return () => {
        window.cancelAnimationFrame(frameId)
      }
    }

    const observer = new ResizeObserver(() => {
      updateContainerWidth()
    })

    observer.observe(container)
    if (container.parentElement) {
      observer.observe(container.parentElement)
    }

    window.addEventListener('resize', updateContainerWidth)

    return () => {
      window.cancelAnimationFrame(frameId)
      window.removeEventListener('resize', updateContainerWidth)
      observer.disconnect()
    }
  }, [props.openRequestTabs.length, updateContainerWidth])

  const { hiddenTabs, tabWidth, visibleTabs } = React.useMemo(
    () => getRequestTabsLayout(props.openRequestTabs, props.activeRequestId, containerWidth),
    [containerWidth, props.activeRequestId, props.openRequestTabs],
  )

  const setPreviewIfChanged = React.useCallback((nextPreview: RequestTabDragPreview | null) => {
    const nextKey = getRequestTabPreviewKey(nextPreview)
    if (previewKeyRef.current === nextKey) {
      return
    }

    previewKeyRef.current = nextKey
    setActivePreview(nextPreview)
  }, [])

  const setTabElement = React.useCallback((requestId: string, element: HTMLDivElement | null) => {
    const nextMap = tabElementsRef.current
    if (element) {
      nextMap.set(requestId, element)
    }
    else {
      nextMap.delete(requestId)
    }
  }, [])

  const clearDragState = React.useCallback(() => {
    dragVisibleRequestIdsRef.current = []
    hitSnapshotRef.current = null
    previewKeyRef.current = getRequestTabPreviewKey(null)
    setActiveDragTab(null)
    setPreviewIfChanged(null)
  }, [setPreviewIfChanged])

  const previewTabs = React.useMemo(() => {
    if (!activeDragTab || !activePreview) {
      return null
    }

    return applyRequestTabMove(props.openRequestTabs, activeDragTab.requestId, activePreview)
  }, [activeDragTab, activePreview, props.openRequestTabs])

  const frozenVisibleRequestIds = activeDragTab && dragVisibleRequestIdsRef.current.length > 0
    ? dragVisibleRequestIdsRef.current
    : null
  const frozenVisibleRequestIdSet = frozenVisibleRequestIds
    ? new Set(frozenVisibleRequestIds)
    : null
  const tabsToRender = previewTabs ?? props.openRequestTabs
  const visibleTabsToRender = frozenVisibleRequestIdSet
    ? tabsToRender.filter(tab => frozenVisibleRequestIdSet.has(tab.requestId))
    : visibleTabs
  const hiddenTabsToRender = frozenVisibleRequestIdSet
    ? tabsToRender.filter(tab => !frozenVisibleRequestIdSet.has(tab.requestId))
    : hiddenTabs

  React.useEffect(() => {
    if (!activeDragTab) {
      return
    }

    const draggedTabStillExists = props.openRequestTabs.some(tab => tab.requestId === activeDragTab.requestId)
    if (!draggedTabStillExists) {
      clearDragState()
    }
  }, [activeDragTab, clearDragState, props.openRequestTabs])

  const handleDragStart = React.useCallback((event: RequestTabsDragStartEvent) => {
    const sourceData = event.operation.source?.data ?? {}
    const sourceElement = event.operation.source?.element
    if (typeof sourceData.requestId !== 'string') {
      return
    }

    const sourceTab = props.openRequestTabs.find(tab => tab.requestId === sourceData.requestId)
    if (!sourceTab) {
      return
    }

    dragVisibleRequestIdsRef.current = visibleTabs.map(tab => tab.requestId)
    hitSnapshotRef.current = createRequestTabHitSnapshot(visibleTabs, tabElementsRef.current)
    setActiveDragTab({
      dirty: sourceTab.dirty,
      method: sourceTab.method,
      requestId: sourceTab.requestId,
      title: sourceTab.title,
      width: sourceElement instanceof HTMLElement ? sourceElement.getBoundingClientRect().width : tabWidth,
    })
    setPreviewIfChanged(null)
  }, [props.openRequestTabs, setPreviewIfChanged, tabWidth, visibleTabs])

  const handleDragMove = React.useCallback((event: RequestTabsDragMoveEvent) => {
    if (!activeDragTab) {
      return
    }

    const nativeEvent = event.nativeEvent
    if (!(nativeEvent instanceof MouseEvent || nativeEvent instanceof PointerEvent)) {
      return
    }

    const hitSnapshot = hitSnapshotRef.current
    if (!hitSnapshot) {
      return
    }

    setPreviewIfChanged(resolveRequestTabPreviewFromPointer({
      clientX: nativeEvent.clientX,
      draggedRequestId: activeDragTab.requestId,
      hitSnapshot,
      openRequestTabs: props.openRequestTabs,
    }))
  }, [activeDragTab, props.openRequestTabs, setPreviewIfChanged])

  const handleDragEnd = React.useCallback((event: RequestTabsDragEndEvent) => {
    if (event.canceled || !activeDragTab) {
      clearDragState()
      return
    }

    if (!activePreview) {
      clearDragState()
      return
    }

    const nextTabs = applyRequestTabMove(props.openRequestTabs, activeDragTab.requestId, activePreview)
    if (!nextTabs) {
      clearDragState()
      return
    }

    props.onReorderRequestTabs(nextTabs)
    clearDragState()
  }, [activeDragTab, activePreview, clearDragState, props.onReorderRequestTabs, props.openRequestTabs])

  const closeTabGroup = React.useCallback((tabs: OpenRequestTab[], preservedRequestId?: string) => {
    if (tabs.length === 0) {
      return
    }

    if (preservedRequestId && preservedRequestId !== props.activeRequestId) {
      props.onFocusRequestTab(preservedRequestId)
    }

    const dirtyTabs = tabs.filter(tab => tab.dirty)
    const closableTabs = tabs.filter(tab => !tab.dirty)

    closableTabs.forEach((tab) => {
      props.onCloseRequestTab(tab.requestId)
    })

    if (dirtyTabs.length > 0 && closableTabs.length > 0) {
      toast(`已关闭 ${closableTabs.length} 个标签，保留 ${dirtyTabs.length} 个未保存标签。`)
    }
    else if (dirtyTabs.length > 0) {
      toast(`有 ${dirtyTabs.length} 个标签还没保存，暂时没有关闭。`)
    }
  }, [props.activeRequestId, props.onCloseRequestTab, props.onFocusRequestTab])
  const handleCloseCurrent = React.useCallback((requestId: string) => {
    props.onCloseRequestTab(requestId)
  }, [props.onCloseRequestTab])

  const handleCloseOthers = React.useCallback((requestId: string) => {
    closeTabGroup(
      props.openRequestTabs.filter(tab => tab.requestId !== requestId),
      requestId,
    )
  }, [closeTabGroup, props.openRequestTabs])

  const handleCloseRight = React.useCallback((requestId: string) => {
    const targetIndex = props.openRequestTabs.findIndex(tab => tab.requestId === requestId)
    if (targetIndex < 0) {
      return
    }

    closeTabGroup(
      props.openRequestTabs.slice(targetIndex + 1),
      requestId,
    )
  }, [closeTabGroup, props.openRequestTabs])

  const handleCloseAll = React.useCallback(() => {
    closeTabGroup(props.openRequestTabs)
  }, [closeTabGroup, props.openRequestTabs])

  return (
    <div
      className={cn('relative border-b border-border/70', props.isMacOSDesktop && macOSWindowChromeHeightClassName)}
      onMouseDown={props.isMacOSDesktop ? startWindowDragging : undefined}
    >
      <div
        className={cn(
          'relative z-10 px-3',
          props.isMacOSDesktop ? 'h-full min-h-full py-2' : 'min-h-10 py-1.5',
        )}
      >
        <DragDropProvider
          sensors={[
            PointerSensor.configure({
              activationConstraints: [new PointerActivationConstraints.Distance({ value: 6 })],
              preventActivation(event) {
                return event.target instanceof HTMLElement && Boolean(event.target.closest('[data-tab-action]'))
              },
            }),
          ]}
          onDragEnd={handleDragEnd}
          onDragMove={handleDragMove}
          onDragStart={handleDragStart}
        >
          <div ref={containerRef} className="flex w-full min-w-0 items-center gap-1.5">
            <div className="flex w-0 min-w-0 flex-1 items-center gap-1.5 overflow-hidden">
              <AnimatePresence mode="popLayout">
                {visibleTabsToRender.map(tab => (
                  <RequestTabListItem
                    key={tab.requestId}
                    activeDragTab={activeDragTab}
                    activePreview={activePreview}
                    activeRequestId={props.activeRequestId}
                    isCloseRightDisabled={props.openRequestTabs.findIndex(item => item.requestId === tab.requestId) >= props.openRequestTabs.length - 1}
                    isMacOSDesktop={props.isMacOSDesktop}
                    setTabElement={setTabElement}
                    tab={tab}
                    tabWidth={tabWidth}
                    onCloseAll={handleCloseAll}
                    onCloseCurrent={handleCloseCurrent}
                    onCloseOthers={handleCloseOthers}
                    onCloseRequestTab={props.onCloseRequestTab}
                    onCloseRight={handleCloseRight}
                    onFocusRequestTab={props.onFocusRequestTab}
                  />
                ))}
              </AnimatePresence>
            </div>

            {hiddenTabsToRender.length > 0 && (
              <RequestTabsOverflowMenu
                activeRequestId={props.activeRequestId}
                hiddenTabs={hiddenTabsToRender}
                isMacOSDesktop={props.isMacOSDesktop}
                onCloseRequestTab={props.onCloseRequestTab}
                onFocusRequestTab={props.onFocusRequestTab}
              />
            )}
          </div>

          <DragOverlay
            dropAnimation={{
              duration: 180,
              easing: 'cubic-bezier(0.22, 1, 0.36, 1)',
            }}
          >
            {activeDragTab
              ? (
                  <RequestTabDragOverlay
                    isMacOSDesktop={props.isMacOSDesktop}
                    item={activeDragTab}
                    tabWidth={activeDragTab.width ?? tabWidth}
                  />
                )
              : null}
          </DragOverlay>
        </DragDropProvider>
      </div>
    </div>
  )
}

interface RequestTabChipProps {
  chipRef?: React.Ref<HTMLDivElement>
  interactive: boolean
  isMacOSDesktop: boolean
  tab: RequestTabChipData
  tabWidth: number
  visualState: RequestTabVisualState
  onCloseRequestTab?: (requestId: string) => void
  onFocusRequestTab?: (requestId: string) => void
}

function RequestTabChip(props: RequestTabChipProps) {
  const content = (
    <>
      <MethodBadge method={props.tab.method} subtle />
      <span className="min-w-0 truncate text-[13px] font-medium">{props.tab.title}</span>
    </>
  )

  return (
    <div
      ref={props.chipRef}
      data-no-window-drag
      style={{
        flexBasis: props.tabWidth,
        maxWidth: props.tabWidth,
        minWidth: props.tabWidth,
        width: props.tabWidth,
      }}
      className={cn(
        'group flex min-w-0 shrink-0 items-center gap-1.5 rounded-lg border transition',
        props.interactive && 'cursor-grab active:cursor-grabbing',
        props.isMacOSDesktop ? 'h-8 p-1.5' : 'p-1.5',
        props.visualState === 'selected' && 'border-primary/40 bg-primary/5',
        props.visualState === 'preview' && 'border-border/80 bg-accent opacity-50',
        props.visualState === 'overlay' && 'border-border/80 bg-accent shadow-sm',
        props.visualState === 'idle' && 'border-transparent bg-muted/50 hover:border-border/80 hover:bg-accent',
      )}
    >
      {props.interactive && props.onFocusRequestTab
        ? (
            <button
              type="button"
              onClick={() => props.onFocusRequestTab?.(props.tab.requestId)}
              className="flex min-w-0 flex-1 items-center gap-1.5 text-left"
            >
              {content}
            </button>
          )
        : (
            <div className="flex min-w-0 flex-1 items-center gap-1.5">
              {content}
            </div>
          )}
      <div className="relative size-6 shrink-0">
        <span
          aria-hidden="true"
          className={cn(
            'pointer-events-none absolute inset-0 flex items-center justify-center text-muted-foreground transition group-hover:opacity-0 group-focus-within:opacity-0',
            props.tab.dirty ? 'text-amber-500' : 'text-muted-foreground/80',
          )}
        >
          {props.tab.dirty && <span className="size-2 rounded-full bg-current" />}
        </span>
        {props.interactive && props.onCloseRequestTab && (
          <button
            data-tab-action
            type="button"
            aria-label={`关闭 ${props.tab.title}`}
            onClick={() => props.onCloseRequestTab?.(props.tab.requestId)}
            className="absolute inset-0 rounded-md p-1 text-muted-foreground opacity-0 transition group-hover:opacity-100 group-focus-within:opacity-100 hover:bg-background hover:text-foreground"
          >
            <XIcon className="size-4" />
          </button>
        )}
      </div>
    </div>
  )
}

interface RequestTabListItemProps {
  activeDragTab: RequestTabDragItem | null
  activePreview: RequestTabDragPreview | null
  activeRequestId: string | null
  isCloseRightDisabled: boolean
  isMacOSDesktop: boolean
  setTabElement: (requestId: string, element: HTMLDivElement | null) => void
  tab: OpenRequestTab
  tabWidth: number
  onCloseAll: () => void
  onCloseCurrent: (requestId: string) => void
  onCloseOthers: (requestId: string) => void
  onCloseRequestTab: (requestId: string) => void
  onCloseRight: (requestId: string) => void
  onFocusRequestTab: (requestId: string) => void
}

function RequestTabListItem(props: RequestTabListItemProps) {
  const draggable = useDraggable({
    id: props.tab.requestId,
    data: {
      dirty: props.tab.dirty,
      method: props.tab.method,
      requestId: props.tab.requestId,
      title: props.tab.title,
    },
    type: requestTabsDragType,
  })

  const chipRef = React.useCallback((element: HTMLDivElement | null) => {
    draggable.ref(element)
    props.setTabElement(props.tab.requestId, element)
  }, [draggable, props.setTabElement, props.tab.requestId])

  const isDraggingSelf = props.activeDragTab?.requestId === props.tab.requestId
  const visualState: RequestTabVisualState = isDraggingSelf
    ? (props.activePreview ? 'preview' : 'selected')
    : (props.activeRequestId === props.tab.requestId ? 'selected' : 'idle')

  return (
    <ContextMenu>
      <ContextMenuTrigger className="shrink-0">
        <motion.div layout="position" transition={requestTabsLayoutTransition}>
          <RequestTabChip
            chipRef={chipRef}
            interactive
            isMacOSDesktop={props.isMacOSDesktop}
            tab={props.tab}
            tabWidth={props.tabWidth}
            visualState={visualState}
            onCloseRequestTab={props.onCloseRequestTab}
            onFocusRequestTab={props.onFocusRequestTab}
          />
        </motion.div>
      </ContextMenuTrigger>
      <ContextMenuContent className="w-44">
        <ContextMenuItem onClick={() => props.onCloseCurrent(props.tab.requestId)}>
          关闭当前
        </ContextMenuItem>
        <ContextMenuItem onClick={() => props.onCloseOthers(props.tab.requestId)}>
          关闭其他
        </ContextMenuItem>
        <ContextMenuItem
          disabled={props.isCloseRightDisabled}
          onClick={() => props.onCloseRight(props.tab.requestId)}
        >
          关闭右侧
        </ContextMenuItem>
        <ContextMenuItem onClick={props.onCloseAll}>
          关闭所有
        </ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  )
}

function RequestTabDragOverlay(props: {
  isMacOSDesktop: boolean
  item: RequestTabDragItem
  tabWidth: number
}) {
  return (
    <RequestTabChip
      interactive={false}
      isMacOSDesktop={props.isMacOSDesktop}
      tab={props.item}
      tabWidth={props.tabWidth}
      visualState="overlay"
    />
  )
}

interface RequestTabsOverflowMenuProps {
  activeRequestId: string | null
  hiddenTabs: OpenRequestTab[]
  isMacOSDesktop: boolean
  onCloseRequestTab: (requestId: string) => void
  onFocusRequestTab: (requestId: string) => void
}

function RequestTabsOverflowMenu(props: RequestTabsOverflowMenuProps) {
  return (
    <div data-no-window-drag className="group/overflow relative shrink-0">
      <button
        type="button"
        aria-label={`还有 ${props.hiddenTabs.length} 个请求标签`}
        className={cn(
          'flex h-8 w-14 items-center justify-center gap-1 rounded-lg border border-transparent bg-muted/50 text-muted-foreground transition hover:border-border/80 hover:bg-accent hover:text-foreground',
          props.isMacOSDesktop ? 'h-8' : 'h-[34px]',
        )}
      >
        <EllipsisIcon className="size-4" />
        <span className="text-[11px] font-semibold tabular-nums">{props.hiddenTabs.length}</span>
      </button>

      <div className="pointer-events-none absolute right-0 top-full z-30 pt-2 opacity-0 transition group-hover/overflow:pointer-events-auto group-hover/overflow:opacity-100 group-focus-within/overflow:pointer-events-auto group-focus-within/overflow:opacity-100">
        <div className="w-60 rounded-xl border border-border/80 bg-popover/95 p-1.5 text-popover-foreground shadow-lg backdrop-blur">
          <div className="flex items-center justify-between px-2 py-1">
            <span className="text-[11px] font-medium text-muted-foreground">
              更多
            </span>
            <span className="text-[11px] text-muted-foreground">
              {props.hiddenTabs.length}
              {' '}
              个
            </span>
          </div>

          <div className="max-h-80 overflow-auto pr-1">
            {props.hiddenTabs.map((tab) => {
              const isActive = props.activeRequestId === tab.requestId
              return (
                <div
                  key={tab.requestId}
                  className={cn(
                    'group/item flex items-center gap-1 rounded-lg border border-transparent pr-1 transition hover:border-border/70 hover:bg-accent',
                    isActive && 'border-primary/40 bg-primary/5',
                  )}
                >
                  <button
                    type="button"
                    onClick={() => props.onFocusRequestTab(tab.requestId)}
                    className="flex min-w-0 flex-1 items-center gap-1.5 p-1 text-left"
                  >
                    <MethodBadge method={tab.method} subtle />
                    <span className="min-w-0 flex-1 truncate text-[13px] font-medium">{tab.title}</span>
                  </button>
                  <div className="relative size-6 shrink-0">
                    <span
                      aria-hidden="true"
                      className={cn(
                        'pointer-events-none absolute inset-0 flex items-center justify-center text-muted-foreground transition group-hover/item:opacity-0 group-focus-within/item:opacity-0',
                        tab.dirty ? 'text-amber-500' : 'text-muted-foreground/80',
                      )}
                    >
                      {tab.dirty && <span className="size-2 rounded-full bg-current" />}
                    </span>
                    <button
                      type="button"
                      aria-label={`关闭 ${tab.title}`}
                      onClick={() => props.onCloseRequestTab(tab.requestId)}
                      className="absolute inset-0 rounded-md p-1 text-muted-foreground opacity-0 transition group-hover/item:opacity-100 group-focus-within/item:opacity-100 hover:bg-background hover:text-foreground"
                    >
                      <XIcon className="size-4" />
                    </button>
                  </div>
                </div>
              )
            })}
          </div>
        </div>
      </div>
    </div>
  )
}

interface RequestHeaderBarProps {
  draft: RequestEditorDraft
  hasUnsavedChanges: boolean
  isBusy: boolean
  onChangeDraft: (updater: (draft: RequestEditorDraft) => RequestEditorDraft) => void
  onUrlChange: (url: string) => void
  onSaveRequest: () => void
  onSendRequest: () => void
}

function RequestHeaderBar(props: RequestHeaderBarProps) {
  const normalizedUrl = React.useMemo(() => normalizeRequestUrl(props.draft.url), [props.draft.url])
  const hasUrl = normalizedUrl.length > 0
  const hasUrlError = hasUrl && !isValidRequestUrl(normalizedUrl)

  return (
    <div className="border-b border-border/70 px-3 py-3">
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
            <SelectValue placeholder="请求方法" />
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
          className="h-7 min-w-[280px] flex-1 text-sm focus-visible:border-input focus-visible:ring-0"
          aria-invalid={hasUrlError || undefined}
          value={props.draft.url}
          onChange={event => props.onUrlChange(event.target.value)}
          onBlur={(event) => {
            const nextUrl = normalizeRequestUrl(event.target.value)
            if (nextUrl !== props.draft.url) {
              props.onUrlChange(nextUrl)
            }
          }}
          placeholder="请输入请求地址"
        />

        <Button size="sm" disabled={props.isBusy} onClick={props.onSendRequest}>
          <SendHorizonalIcon />
          发送
        </Button>
        <Button size="sm" disabled={props.isBusy || !props.hasUnsavedChanges} variant="outline" onClick={props.onSaveRequest}>
          <SaveIcon />
          保存
        </Button>
      </div>
      {hasUrlError && (
        <p className="mt-2 text-xs text-destructive">
          URL 格式不正确，请输入完整地址，或使用 `/path`、`?query=1` 这类相对地址。
        </p>
      )}
    </div>
  )
}

function WebSocketHeaderBar(props: {
  draft: RequestEditorDraft
  hasUnsavedChanges: boolean
  isBusy: boolean
  session: WebSocketSessionState | null
  onChangeDraft: (updater: (draft: RequestEditorDraft) => RequestEditorDraft) => void
  onUrlChange: (url: string) => void
  onSaveRequest: () => void
  onConnect: () => void
  onDisconnect: () => void
}) {
  const normalizedUrl = React.useMemo(() => normalizeRequestUrl(props.draft.url), [props.draft.url])
  const hasUrl = normalizedUrl.length > 0
  const hasUrlError = hasUrl && !isValidWebSocketUrl(normalizedUrl)
  const statusLabel = props.session?.status ?? 'idle'

  return (
    <div className="border-b border-border/70 px-3 py-3">
      <div className="flex flex-wrap items-center gap-2 xl:flex-nowrap">
        <div className="inline-flex h-7 w-[112px] items-center justify-center rounded-md border border-border/70 bg-muted/40 text-xs font-semibold uppercase tracking-wide">
          WS
        </div>

        <Input
          className="h-7 min-w-[280px] flex-1 text-sm focus-visible:border-input focus-visible:ring-0"
          aria-invalid={hasUrlError || undefined}
          value={props.draft.url}
          onChange={event => props.onUrlChange(event.target.value)}
          placeholder="ws:// 或 wss:// 地址"
        />

        <Badge variant="outline" className="h-7 rounded-md px-2 text-xs capitalize">
          {statusLabel}
        </Badge>

        {props.session?.status === 'connected'
          ? (
              <Button size="sm" disabled={props.isBusy} onClick={props.onDisconnect}>
                断开
              </Button>
            )
          : (
              <Button size="sm" disabled={props.isBusy || hasUrlError} onClick={props.onConnect}>
                连接
              </Button>
            )}
        <Button size="sm" disabled={props.isBusy || !props.hasUnsavedChanges} variant="outline" onClick={props.onSaveRequest}>
          <SaveIcon />
          保存
        </Button>
      </div>
      {hasUrlError && (
        <p className="mt-2 text-xs text-destructive">
          WebSocket URL 必须以 `ws://` 或 `wss://` 开头。
        </p>
      )}
    </div>
  )
}

const requestBodyModeOptions = [
  { label: 'JSON', value: 'json' },
  { label: '原始文本', value: 'raw' },
  { label: 'form-data', value: 'form-data' },
  { label: 'x-www-form-urlencoded', value: 'x-www-form-urlencoded' },
  { label: '二进制文件', value: 'binary' },
  { label: '无', value: 'none' },
] as const

const authTypeOptions: Array<{ value: AuthType, label: string }> = [
  { value: 'none', label: '无认证' },
  { value: 'basic', label: 'Basic Auth' },
  { value: 'bearer', label: 'Bearer Token' },
  { value: 'api-key', label: 'API Key' },
] as const

type AuthModeValue = AuthType | 'inherit'

const inheritedAuthOption = { value: 'inherit' as const, label: '继承上一级' }
const authModeOptions = [inheritedAuthOption, ...authTypeOptions] as const

const apiKeyAddToOptions = [
  { value: 'header', label: 'Header' },
  { value: 'query', label: 'Query' },
] as const

const requestBodyModeLabelMap = Object.fromEntries(
  requestBodyModeOptions.map(option => [option.value, option.label]),
) as Record<(typeof requestBodyModeOptions)[number]['value'], string>

function getAuthModeValue(auth: { inherit: boolean, authType: AuthType }, allowInherit: boolean): AuthModeValue {
  if (allowInherit && auth.inherit) {
    return 'inherit'
  }

  return auth.authType
}

function applyAuthMode<T extends { inherit: boolean, authType: AuthType }>(auth: T, mode: AuthModeValue): T {
  if (mode === 'inherit') {
    return {
      ...auth,
      inherit: true,
      authType: 'none',
    }
  }

  return {
    ...auth,
    inherit: false,
    authType: mode,
  }
}

function normalizeScopeAuthMode(config: RequestScopeConfig, allowInherit: boolean): RequestScopeConfig {
  if (allowInherit) {
    return config
  }

  return {
    ...config,
    auth: {
      ...config.auth,
      inherit: false,
    },
  }
}

interface RequestEditorTabsProps {
  activeTab: EditorPanelTab
  draft: RequestEditorDraft
  onActiveTabChange: (tab: EditorPanelTab) => void
  onChangeDraft: (updater: (draft: RequestEditorDraft) => RequestEditorDraft) => void
  onQueryChange: (query: KeyValue[]) => void
}

function RequestEditorTabs(props: RequestEditorTabsProps) {
  const bodyMode = props.draft.request.body.mode
  const isWebSocket = props.draft.protocol === 'websocket'
  const visibleTabs = React.useMemo(
    () => (isWebSocket
      ? editorTabs.filter(tab => ['query', 'headers', 'auth'].includes(tab.value))
      : editorTabs),
    [isWebSocket],
  )
  const effectiveActiveTab = visibleTabs.some(tab => tab.value === props.activeTab)
    ? props.activeTab
    : 'query'
  const bodyModeLabel = requestBodyModeLabelMap[bodyMode as keyof typeof requestBodyModeLabelMap] ?? bodyMode
  const environments = useWorkbenchStore(s => s.environments)
  const activeEnvironmentId = useWorkbenchStore(s => s.activeEnvironmentId)

  const activeEnv = React.useMemo(() => {
    if (!activeEnvironmentId)
      return null
    return environments.find(e => e.id === activeEnvironmentId) ?? null
  }, [activeEnvironmentId, environments])

  const envVariables = React.useMemo(() => {
    if (!activeEnv)
      return []
    return activeEnv.variables.filter(v => v.enabled).map(v => ({
      key: v.key,
      value: v.value,
      description: v.description,
    }))
  }, [activeEnv])

  const envName = activeEnv?.name ?? 'default'

  const handleAddKeyValueRow = React.useCallback(() => {
    if (effectiveActiveTab === 'query') {
      props.onChangeDraft(draft => ({
        ...draft,
        request: {
          ...draft.request,
          query: [...draft.request.query, createKeyValueDraft()],
        },
      }))
      return
    }

    if (effectiveActiveTab === 'headers') {
      props.onChangeDraft(draft => ({
        ...draft,
        request: {
          ...draft.request,
          headers: [...draft.request.headers, createKeyValueDraft()],
        },
      }))
    }
  }, [effectiveActiveTab, props.onChangeDraft])

  React.useEffect(() => {
    if (props.activeTab !== effectiveActiveTab) {
      props.onActiveTabChange(effectiveActiveTab)
    }
  }, [effectiveActiveTab, props.activeTab, props.onActiveTabChange])

  return (
    <div className="flex h-full min-h-0 flex-col overflow-hidden border-b border-border/70 px-3 py-3">
      <Tabs value={effectiveActiveTab} onValueChange={value => props.onActiveTabChange(value as EditorPanelTab)} className="h-full min-h-0">
        <div className="mb-3 flex shrink-0 items-center justify-between gap-3">
          <TabsList>
            {visibleTabs.map(tab => (
              <TabsTrigger key={tab.value} value={tab.value} className="px-1.5 py-0.5 text-[13px]">
                {tab.label}
              </TabsTrigger>
            ))}
          </TabsList>

          <div className="flex shrink-0 items-center gap-2">
            {(effectiveActiveTab === 'query' || effectiveActiveTab === 'headers') && (
              <Button size="xs" variant="outline" onClick={handleAddKeyValueRow}>
                <PlusIcon className="size-4" />
                新增
              </Button>
            )}

            {effectiveActiveTab === 'body' && !isWebSocket && (
              <Select
                value={bodyMode}
                onValueChange={(value) => {
                  if (value) {
                    props.onChangeDraft(draft => ({
                      ...draft,
                      request: {
                        ...draft.request,
                        body: { ...draft.request.body, mode: value as typeof bodyMode },
                      },
                    }))
                  }
                }}
              >
                <SelectTrigger size="sm" className="w-[220px]">
                  <SelectValue>
                    {bodyModeLabel}
                  </SelectValue>
                </SelectTrigger>
                <SelectContent className="min-w-[220px]">
                  {requestBodyModeOptions.map(option => (
                    <SelectItem key={option.value} value={option.value}>
                      {option.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            )}

            {effectiveActiveTab === 'auth' && (
              <Select
                value={getAuthModeValue(props.draft.request.auth, true)}
                onValueChange={(value) => {
                  if (!value) {
                    return
                  }

                  props.onChangeDraft(draft => ({
                    ...draft,
                    request: {
                      ...draft.request,
                      auth: applyAuthMode(draft.request.auth, value as AuthModeValue),
                    },
                  }))
                }}
              >
                <SelectTrigger size="sm" className="w-[176px]">
                  <SelectValue>
                    {authModeOptions.find(option => option.value === getAuthModeValue(props.draft.request.auth, true))?.label ?? '认证方式'}
                  </SelectValue>
                </SelectTrigger>
                <SelectContent>
                  {authModeOptions.map(option => (
                    <SelectItem key={option.value} value={option.value}>
                      {option.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            )}

          </div>
        </div>

        <TabsContent value="query" className="min-h-0">
          <KeyValueTable
            emptyLabel="暂无查询参数"
            rows={props.draft.request.query}
            environmentVariables={envVariables}
            environmentName={envName}
            onChange={props.onQueryChange}
          />
        </TabsContent>

        <TabsContent value="headers" className="min-h-0">
          <KeyValueTable
            emptyLabel="暂无请求头"
            rows={props.draft.request.headers}
            environmentVariables={envVariables}
            environmentName={envName}
            onChange={rows => props.onChangeDraft(draft => ({
              ...draft,
              request: { ...draft.request, headers: rows },
            }))}
          />
        </TabsContent>

        <TabsContent value="auth" className="min-h-0 overflow-auto">
          <AuthEditor
            auth={props.draft.request.auth}
            allowInherit
            showModeSelector={false}
            environmentVariables={envVariables}
            environmentName={envName}
            onChange={auth => props.onChangeDraft(draft => ({
              ...draft,
              request: {
                ...draft.request,
                auth,
              },
            }))}
          />
        </TabsContent>

        {!isWebSocket && (
          <TabsContent value="body" className="min-h-0 overflow-auto">
            <BodyEditor draft={props.draft} onChangeDraft={props.onChangeDraft} />
          </TabsContent>
        )}

        {!isWebSocket && (
          <TabsContent value="preRequestScript" className="flex min-h-0 flex-1 overflow-hidden">
            <PreRequestScriptEditor
              draft={props.draft}
              title="请求前脚本"
              placeholder="// 请求发送前执行，可通过 fuck.config 修改请求配置，通过 fuck.env 操作环境变量"
              onChangeDraft={props.onChangeDraft}
            />
          </TabsContent>
        )}

        {!isWebSocket && (
          <TabsContent value="postRequestScript" className="flex min-h-0 flex-1 overflow-hidden">
            <PostRequestScriptEditor
              draft={props.draft}
              title="响应脚本"
              placeholder="// 请求响应后执行，可通过 fuck.response 访问响应内容"
              onChangeDraft={props.onChangeDraft}
            />
          </TabsContent>
        )}
      </Tabs>
    </div>
  )
}

export function PreRequestScriptEditor(props: {
  draft: RequestEditorDraft
  placeholder?: string
  title?: string
  onChangeDraft: (updater: (draft: RequestEditorDraft) => RequestEditorDraft) => void
}) {
  return (
    <MonacoScriptEditor
      className="min-h-0 flex-1"
      modelUri="file:///pre-request-script.js"
      language="javascript"
      value={props.draft.preRequestScript}
      onChange={(value) => {
        props.onChangeDraft(draft => ({ ...draft, preRequestScript: value }))
      }}
      placeholder={props.placeholder ?? '// 请求发送前执行，可通过 fuck.config 修改请求配置，通过 fuck.env 操作环境变量'}
    />
  )
}

export function PostRequestScriptEditor(props: {
  draft: RequestEditorDraft
  placeholder?: string
  title?: string
  onChangeDraft: (updater: (draft: RequestEditorDraft) => RequestEditorDraft) => void
}) {
  return (
    <MonacoScriptEditor
      className="min-h-0 flex-1"
      modelUri="file:///post-request-script.js"
      language="javascript"
      value={props.draft.postRequestScript}
      onChange={(value) => {
        props.onChangeDraft(draft => ({ ...draft, postRequestScript: value }))
      }}
      placeholder={props.placeholder ?? '// 请求响应后执行，可通过 fuck.response 访问响应内容'}
    />
  )
}

function WebSocketPane(props: {
  requestId: string
  session: WebSocketSessionState | null
  onDraftMessageChange: (requestId: string, value: string) => void
  onMessageFormatChange: (requestId: string, format: WebSocketMessageFormat) => void
  onSendMessage: () => void
}) {
  const session = props.session
  const timelineRef = React.useRef<HTMLDivElement | null>(null)
  const getMessageLabel = (direction: WebSocketSessionState['messages'][number]['direction']) => {
    if (direction === 'outbound') {
      return '请求'
    }
    if (direction === 'inbound') {
      return '响应'
    }
    if (direction === 'system') {
      return '系统'
    }
    return '错误'
  }

  React.useEffect(() => {
    const container = timelineRef.current
    if (!container) {
      return
    }
    container.scrollTop = container.scrollHeight
  }, [session?.messages.length])

  return (
    <div className="flex h-full min-h-0 flex-col overflow-hidden">
      <div className="border-b border-border/70 px-3 py-2">
        <div className="flex flex-wrap items-center gap-2">
          <Input
            value={session?.draftMessage ?? ''}
            onChange={event => props.onDraftMessageChange(props.requestId, event.target.value)}
            onKeyDown={(event) => {
              if (event.key === 'Enter' && !event.nativeEvent.isComposing) {
                event.preventDefault()
                props.onSendMessage()
              }
            }}
            placeholder="输入要发送的文本或 JSON 消息，按回车发送"
            className="h-8 min-w-[220px] flex-1"
          />
          <Select
            value={session?.messageFormat ?? 'text'}
            onValueChange={value => props.onMessageFormatChange(props.requestId, value as WebSocketMessageFormat)}
          >
            <SelectTrigger size="sm" className="w-[132px]">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="text">文本</SelectItem>
              <SelectItem value="json">JSON</SelectItem>
            </SelectContent>
          </Select>
        </div>
      </div>

      <div ref={timelineRef} className="min-h-0 flex-1 overflow-auto px-3 py-3">
        {session && session.messages.length > 0
          ? (
              <div className="space-y-2">
                {session.messages.map((message, index) => (
                  <div
                    key={`${message.timestamp}-${index}`}
                    className={cn(
                      'rounded-xl border px-3 py-2 text-sm',
                      message.direction === 'inbound' && 'border-emerald-200 bg-emerald-50/50 dark:border-emerald-900 dark:bg-emerald-950/20',
                      message.direction === 'outbound' && 'border-sky-200 bg-sky-50/50 dark:border-sky-900 dark:bg-sky-950/20',
                      message.direction === 'system' && 'border-border/70 bg-muted/30',
                      message.direction === 'error' && 'border-destructive/20 bg-destructive/5 text-destructive',
                    )}
                  >
                    <div className="mb-1 flex items-center justify-between gap-2 text-[11px] uppercase tracking-wide text-muted-foreground">
                      <span>{getMessageLabel(message.direction)}</span>
                      <span>{new Date(message.timestamp).toLocaleTimeString()}</span>
                    </div>
                    <pre className="overflow-auto whitespace-pre-wrap break-all font-mono text-xs">{message.message}</pre>
                  </div>
                ))}
              </div>
            )
          : (
              <div className="grid h-full place-items-center rounded-xl border border-dashed border-border/70 text-sm text-muted-foreground">
                建立连接后，WebSocket 收发消息会出现在这里。
              </div>
            )}
      </div>
      {session?.error && (
        <div className="border-t border-border/70 px-3 py-2">
          <p className="text-xs text-destructive">{session.error}</p>
        </div>
      )}
    </div>
  )
}

function ResponsePane(props: { response: ResponseState | null }) {
  if (!props.response) {
    return (
      <div className="grid h-full min-h-0 place-items-center px-3 py-5">
        <div className="max-w-md text-center">
          <p className="text-sm font-medium">响应会在这里出现</p>
          <p className="mt-1.5 text-sm leading-6 text-muted-foreground">
            发送请求后，这里会展示状态码、耗时、响应大小和自动识别的内容视图。
          </p>
        </div>
      </div>
    )
  }

  const responseTypeLabel = props.response.responseType === 'json'
    ? 'JSON'
    : props.response.responseType === 'eventstream'
      ? '事件流'
      : props.response.responseType === 'text'
        ? '文本'
        : '未知'

  const responseStatusLabel = props.response.status !== null
    ? String(props.response.status)
    : props.response.isLoading
      ? (props.response.responseType === 'eventstream' ? '流式接收中' : '请求中')
      : props.response.error
        ? '失败'
        : '等待中'

  return (
    <div className="flex h-full min-h-0 flex-col overflow-hidden">
      <Tabs defaultValue="body" className="flex h-full min-h-0 flex-col overflow-hidden">
        <div className="flex items-center justify-between gap-3 border-b border-border/70 px-3 py-2">
          <TabsList>
            <TabsTrigger value="body" className="px-1.5 py-0.5 text-[13px]">
              响应体
            </TabsTrigger>
            <TabsTrigger value="headers" className="px-1.5 py-0.5 text-[13px]">
              响应头
            </TabsTrigger>
          </TabsList>
          <div className="flex min-w-0 flex-wrap items-center justify-end gap-1.5 text-xs">
            <ResponseMetaBadge label="状态" value={responseStatusLabel} />
            <ResponseMetaBadge label="耗时" value={`${props.response.durationMs} ms`} />
            <ResponseMetaBadge label="类型" value={responseTypeLabel} />
            <ResponseMetaBadge label="大小" value={formatBytes(props.response.sizeBytes)} />
          </div>
        </div>

        <TabsContent value="body" className="min-h-0 flex-1 overflow-hidden px-3 py-3">
          <ResponseBodyView response={props.response} />
        </TabsContent>

        <TabsContent value="headers" className="min-h-0 flex-1 overflow-auto px-3 py-3">
          <ResponseHeadersView response={props.response} />
        </TabsContent>
      </Tabs>
    </div>
  )
}

function ResponseBodyView(props: { response: ResponseState }) {
  if (props.response.responseType === 'eventstream') {
    return <EventStreamResponseView body={props.response.body} isLoading={props.response.isLoading} />
  }

  if (props.response.isLoading) {
    return (
      <div className="grid h-full w-full place-items-center">
        <div className="flex items-center gap-3 text-sm text-muted-foreground">
          <Spinner />
          请求发送中...
        </div>
      </div>
    )
  }

  if (props.response.error) {
    return (
      <div className="rounded-xl border border-destructive/20 bg-destructive/5 p-3 text-sm text-destructive">
        {props.response.error}
      </div>
    )
  }

  return props.response.responseType === 'json'
    ? <JsonResponseView body={props.response.body} />
    : <TextResponseView body={props.response.body} contentType={props.response.contentType} />
}

function ResponseHeadersView(props: { response: ResponseState }) {
  if (props.response.isLoading && props.response.headers.length === 0) {
    return (
      <div className="grid h-full place-items-center rounded-xl border border-dashed border-border/70 text-sm text-muted-foreground">
        请求发送中...
      </div>
    )
  }

  if (props.response.headers.length === 0) {
    return (
      <div className="grid h-full place-items-center rounded-xl border border-dashed border-border/70 text-sm text-muted-foreground">
        暂无响应头
      </div>
    )
  }

  return (
    <div className="overflow-hidden rounded-xl border border-border/70 bg-background">
      <Table>
        <TableBody>
          {props.response.headers.map(header => (
            <TableRow key={`${header.name}:${header.value}`}>
              <TableCell className="px-3 py-2.5 align-top text-muted-foreground break-all whitespace-normal">
                {header.name}
              </TableCell>
              <TableCell className="px-3 py-2.5 font-medium text-foreground break-all whitespace-normal">
                {header.value}
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </div>
  )
}

function JsonResponseView(props: { body: string }) {
  return (
    <MonacoCodeEditor
      className="h-full"
      language="json"
      lineNumbers="on"
      modelUri="file:///response-body.json"
      readOnly
      value={props.body || '没有响应体'}
      wordWrap="on"
    />
  )
}

function TextResponseView(props: { body: string, contentType: string }) {
  const isHtml = props.contentType.toLowerCase().includes('html')

  if (isHtml) {
    return (
      <MonacoCodeEditor
        className="h-full"
        language="html"
        lineNumbers="on"
        modelUri="file:///response-body.html"
        readOnly
        value={props.body || '没有响应体'}
        wordWrap="on"
      />
    )
  }

  return (
    <MonacoCodeEditor
      className="h-full"
      language="plaintext"
      lineNumbers="on"
      modelUri="file:///response-body.txt"
      readOnly
      value={props.body || '没有响应体'}
      wordWrap="on"
    />
  )
}

interface EventStreamLine {
  key: string
  label: string
  value: string
  tone: 'comment' | 'event' | 'data' | 'id' | 'retry' | 'field'
}

function EventStreamResponseView(props: { body: string, isLoading: boolean }) {
  const items = React.useMemo(() => parseEventStreamDataItems(props.body), [props.body])
  const containerRef = React.useRef<HTMLDivElement | null>(null)

  React.useEffect(() => {
    if (!props.isLoading || !containerRef.current) {
      return
    }

    containerRef.current.scrollTop = containerRef.current.scrollHeight
  }, [items, props.isLoading])

  if (!props.body) {
    return (
      <div className="grid h-full place-items-center rounded-xl border border-dashed border-border/70 text-sm text-muted-foreground">
        {props.isLoading ? '等待事件流数据...' : '没有响应体'}
      </div>
    )
  }

  return (
    <div ref={containerRef} className="h-full min-w-0 space-y-3 overflow-x-hidden overflow-y-auto">
      <div className="sticky top-0 z-10 flex items-center justify-between rounded-xl border border-border/70 bg-background/90 px-3 py-2 backdrop-blur">
        <div className="text-sm font-medium">Event Stream</div>
        <Badge variant={props.isLoading ? 'default' : 'secondary'}>
          {props.isLoading ? 'Streaming' : 'Completed'}
        </Badge>
      </div>

      <div className="min-w-0 space-y-1">
        {items.map(item => (
          <div
            key={item.key}
            className="min-w-0 rounded-md px-3 py-2.5 text-sm transition-colors hover:bg-muted/70"
          >
            <div
              className="min-w-0 max-w-full whitespace-pre-wrap break-words font-mono text-[12.5px] leading-5 text-foreground"
            >
              {item.value || '(empty)'}
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}

function parseEventStreamItems(body: string): EventStreamLine[] {
  const normalized = body.replace(/\r\n/g, '\n')
  const lines = normalized
    .split('\n')
    .map(line => line.replace(/\r/g, ''))
    .filter(line => line.trim() !== '')

  if (lines.length === 0) {
    return []
  }

  return lines.map((line, index) => parseEventStreamLine(line, 0, index))
}

function parseEventStreamLine(line: string, blockIndex: number, lineIndex: number): EventStreamLine {
  if (line.startsWith(':')) {
    return {
      key: `${blockIndex}-${lineIndex}`,
      label: 'comment',
      value: line.slice(1).trim(),
      tone: 'comment',
    }
  }

  const separatorIndex = line.indexOf(':')
  if (separatorIndex === -1) {
    return {
      key: `${blockIndex}-${lineIndex}`,
      label: 'field',
      value: line,
      tone: 'field',
    }
  }

  const field = line.slice(0, separatorIndex).trim()
  const value = line.slice(separatorIndex + 1).trimStart()

  if (field === 'event') {
    return { key: `${blockIndex}-${lineIndex}`, label: 'event', value, tone: 'event' }
  }
  if (field === 'data') {
    return { key: `${blockIndex}-${lineIndex}`, label: 'data', value, tone: 'data' }
  }
  if (field === 'id') {
    return { key: `${blockIndex}-${lineIndex}`, label: 'id', value, tone: 'id' }
  }
  if (field === 'retry') {
    return { key: `${blockIndex}-${lineIndex}`, label: 'retry', value, tone: 'retry' }
  }

  return {
    key: `${blockIndex}-${lineIndex}`,
    label: field || 'field',
    value,
    tone: 'field',
  }
}

function parseEventStreamDataItems(body: string): EventStreamLine[] {
  return parseEventStreamItems(body).filter(item => item.tone === 'data')
}

export interface KeyValueTableProps {
  emptyLabel: string
  rows: KeyValue[]
  environmentVariables: Array<{ key: string, value: string, description?: string }>
  environmentName: string
  onChange: (rows: KeyValue[]) => void
}

export function KeyValueTable(props: KeyValueTableProps) {
  function updateRow(rowId: string, updater: (row: KeyValue) => KeyValue) {
    props.onChange(props.rows.map(row => (row.id === rowId ? updater(row) : row)))
  }

  function removeRow(rowId: string) {
    props.onChange(props.rows.filter(row => row.id !== rowId))
  }

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="min-h-0 flex-1 space-y-1.5 overflow-auto pr-1">
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
                  placeholder="键"
                />
                <EnvironmentVariableInput
                  value={row.value}
                  onChange={newValue => updateRow(row.id, current => ({ ...current, value: newValue }))}
                  variables={props.environmentVariables}
                  environmentName={props.environmentName}
                  placeholder="值"
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

export function AuthEditor(props: {
  auth: RequestEditorDraft['request']['auth']
  allowInherit?: boolean
  showModeSelector?: boolean
  showInheritHint?: boolean
  environmentVariables: Array<{ key: string, value: string, description?: string }>
  environmentName: string
  onChange: (auth: RequestEditorDraft['request']['auth']) => void
}) {
  const authMode = getAuthModeValue(props.auth, props.allowInherit ?? false)
  const options = props.allowInherit ? authModeOptions : authTypeOptions

  return (
    <div className="space-y-3">
      {props.showModeSelector !== false && (
        <div className="flex flex-wrap items-center gap-3">
          <div className="w-[220px] max-w-full">
            <Label className="mb-1.5 block text-sm">认证方式</Label>
            <Select
              value={authMode}
              onValueChange={(value) => {
                if (!value) {
                  return
                }

                props.onChange(applyAuthMode(props.auth, value as AuthModeValue))
              }}
            >
              <SelectTrigger className="w-full">
                <SelectValue>
                  {options.find(option => option.value === authMode)?.label ?? '认证方式'}
                </SelectValue>
              </SelectTrigger>
              <SelectContent>
                {options.map(option => (
                  <SelectItem key={option.value} value={option.value}>
                    {option.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          {props.showInheritHint !== false && authMode === 'inherit' && (
            <div className="rounded-lg border border-border/70 bg-muted/30 px-3 py-2 text-sm text-muted-foreground">
              当前层会直接沿用上一级认证配置。
            </div>
          )}
        </div>
      )}

      {props.showInheritHint !== false && props.showModeSelector === false && authMode === 'inherit' && (
        <div className="rounded-lg border border-border/70 bg-muted/30 px-3 py-2 text-sm text-muted-foreground">
          当前请求会直接沿用上一级认证配置。
        </div>
      )}

      {authMode === 'inherit' && null}
      {authMode === 'none' && (
        <div className="grid min-h-[140px] place-items-center rounded-xl border border-dashed border-border/70 text-sm text-muted-foreground">
          当前层未启用认证。
        </div>
      )}

      {authMode === 'basic' && (
        <div className="grid gap-3 md:grid-cols-2">
          <EnvironmentVariableInput
            value={props.auth.basic.username}
            onChange={value => props.onChange({
              ...props.auth,
              basic: {
                ...props.auth.basic,
                username: value,
              },
            })}
            variables={props.environmentVariables}
            environmentName={props.environmentName}
            placeholder="用户名"
          />
          <EnvironmentVariableInput
            value={props.auth.basic.password}
            onChange={value => props.onChange({
              ...props.auth,
              basic: {
                ...props.auth.basic,
                password: value,
              },
            })}
            variables={props.environmentVariables}
            environmentName={props.environmentName}
            placeholder="密码"
          />
        </div>
      )}

      {authMode === 'bearer' && (
        <EnvironmentVariableInput
          value={props.auth.bearerToken}
          onChange={value => props.onChange({
            ...props.auth,
            bearerToken: value,
          })}
          variables={props.environmentVariables}
          environmentName={props.environmentName}
          placeholder="Bearer Token"
        />
      )}

      {authMode === 'api-key' && (
        <div className="grid grid-cols-[140px_minmax(0,_1fr)_minmax(0,_1fr)] gap-3">
          <Select
            value={props.auth.apiKey.addTo}
            onValueChange={(value) => {
              if (!value) {
                return
              }
              props.onChange({
                ...props.auth,
                apiKey: {
                  ...props.auth.apiKey,
                  addTo: value,
                },
              })
            }}
          >
            <SelectTrigger className="w-full">
              <SelectValue>
                {apiKeyAddToOptions.find(option => option.value === props.auth.apiKey.addTo)?.label ?? '添加位置'}
              </SelectValue>
            </SelectTrigger>
            <SelectContent>
              {apiKeyAddToOptions.map(option => (
                <SelectItem key={option.value} value={option.value}>
                  {option.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <EnvironmentVariableInput
            value={props.auth.apiKey.key}
            onChange={value => props.onChange({
              ...props.auth,
              apiKey: {
                ...props.auth.apiKey,
                key: value,
              },
            })}
            variables={props.environmentVariables}
            environmentName={props.environmentName}
            placeholder="参数名"
          />
          <EnvironmentVariableInput
            value={props.auth.apiKey.value}
            onChange={value => props.onChange({
              ...props.auth,
              apiKey: {
                ...props.auth.apiKey,
                value,
              },
            })}
            variables={props.environmentVariables}
            environmentName={props.environmentName}
            placeholder="参数值"
          />
        </div>
      )}
    </div>
  )
}

export function RequestScopeConfigEditor(props: {
  config: RequestScopeConfig
  activeTab?: Exclude<SettingsPanelTab, 'info'>
  allowInherit?: boolean
  onChange: (value: RequestScopeConfig) => void
  title: string
  description: string
}) {
  const activeTab = props.activeTab ?? 'headers'
  const allowInherit = props.allowInherit ?? true
  const environments = useWorkbenchStore(state => state.environments)
  const activeEnvironmentId = useWorkbenchStore(state => state.activeEnvironmentId)

  const activeEnv = React.useMemo(() => {
    if (!activeEnvironmentId)
      return null
    return environments.find(environment => environment.id === activeEnvironmentId) ?? null
  }, [activeEnvironmentId, environments])

  const envVariables = React.useMemo(() => {
    if (!activeEnv)
      return []
    return activeEnv.variables.filter(variable => variable.enabled).map(variable => ({
      key: variable.key,
      value: variable.value,
      description: variable.description,
    }))
  }, [activeEnv])

  const envName = activeEnv?.name ?? 'default'

  const handleAddHeader = React.useCallback(() => {
    props.onChange({
      ...props.config,
      headers: [...props.config.headers, createKeyValueDraft()],
    })
  }, [props])

  return (
    <div className="flex h-full min-h-0 flex-1 flex-col gap-3 rounded-xl border border-border/70 bg-muted/20 p-3">
      <div className="flex items-start justify-between gap-3">
        <div className="space-y-1">
          <h3 className="text-sm font-medium">{props.title}</h3>
          <p className="text-xs leading-5 text-muted-foreground">{props.description}</p>
        </div>

        {activeTab === 'headers' && (
          <Button size="xs" variant="outline" onClick={handleAddHeader}>
            <PlusIcon className="size-4" />
            新增
          </Button>
        )}
      </div>

      {activeTab === 'headers' && (
        <KeyValueTable
          emptyLabel="暂无默认请求头"
          rows={props.config.headers}
          environmentVariables={envVariables}
          environmentName={envName}
          onChange={headers => props.onChange({ ...props.config, headers })}
        />
      )}

      {activeTab === 'auth' && (
        <div className="min-h-0 flex-1 space-y-3 overflow-auto pr-1">
          <AuthEditor
            auth={props.config.auth as RequestEditorDraft['request']['auth']}
            allowInherit={allowInherit}
            showInheritHint={false}
            environmentVariables={envVariables}
            environmentName={envName}
            onChange={auth => props.onChange({ ...props.config, auth })}
          />
        </div>
      )}

      {activeTab === 'preRequestScript' && (
        <MonacoScriptEditor
          className="min-h-0 flex-1"
          modelUri="file:///request-scope-pre-request-script.js"
          language="javascript"
          value={props.config.preRequestScript}
          onChange={value => props.onChange({ ...props.config, preRequestScript: value })}
          placeholder="// 默认请求脚本，会在更下层的脚本前执行"
        />
      )}

      {activeTab === 'postRequestScript' && (
        <MonacoScriptEditor
          className="min-h-0 flex-1"
          modelUri="file:///request-scope-post-request-script.js"
          language="javascript"
          value={props.config.postRequestScript}
          onChange={value => props.onChange({ ...props.config, postRequestScript: value })}
          placeholder="// 默认响应脚本，会在更下层的脚本前执行"
        />
      )}
    </div>
  )
}

function ScopeBaseUrlField(props: {
  label: string
  description: string
  value: string
  onChange: (value: string) => void
}) {
  const environments = useWorkbenchStore(state => state.environments)
  const activeEnvironmentId = useWorkbenchStore(state => state.activeEnvironmentId)

  const activeEnv = React.useMemo(() => {
    if (!activeEnvironmentId)
      return null
    return environments.find(environment => environment.id === activeEnvironmentId) ?? null
  }, [activeEnvironmentId, environments])

  const envVariables = React.useMemo(() => {
    if (!activeEnv)
      return []
    return activeEnv.variables.filter(variable => variable.enabled).map(variable => ({
      key: variable.key,
      value: variable.value,
      description: variable.description,
    }))
  }, [activeEnv])

  return (
    <div className="space-y-1">
      <Label>{props.label}</Label>
      <EnvironmentVariableInput
        value={props.value}
        onChange={props.onChange}
        variables={envVariables}
        environmentName={activeEnv?.name ?? 'default'}
        placeholder="https://api.example.com"
      />
      <p className="text-xs leading-5 text-muted-foreground">{props.description}</p>
    </div>
  )
}

function CollectionBaseUrlField(props: { value: string, onChange: (value: string) => void }) {
  return (
    <ScopeBaseUrlField
      label="目录 Base URL"
      description="当前目录下的请求会优先基于这里的 Base URL 解析相对地址。可使用环境变量。"
      value={props.value}
      onChange={props.onChange}
    />
  )
}

function ProjectBaseUrlField(props: { value: string, onChange: (value: string) => void }) {
  return (
    <ScopeBaseUrlField
      label="项目 Base URL"
      description="会作为整个项目的默认 Base URL，被目录和请求继续继承。可使用环境变量。"
      value={props.value}
      onChange={props.onChange}
    />
  )
}

function CollectionSettingsPane(props: {
  activeTab: SettingsPanelTab
  draft: CollectionEditorDraft
  hasUnsavedChanges: boolean
  isBusy: boolean
  onChangeDraft: (updater: (draft: CollectionEditorDraft) => CollectionEditorDraft) => void
  onActiveTabChange: (tab: WorkbenchPanelTab) => void
  onSave: () => void
}) {
  return (
    <div className="flex h-full min-h-0 flex-col overflow-hidden border-b border-border/70 px-3 py-3">
      <Tabs value={props.activeTab} onValueChange={value => props.onActiveTabChange(value as WorkbenchPanelTab)} className="h-full min-h-0">
        <div className="mb-3 flex shrink-0 items-center justify-between gap-3">
          <TabsList>
            {settingsTabs.map(tab => (
              <TabsTrigger key={tab.value} value={tab.value} className="px-1.5 py-0.5 text-[13px]">
                {tab.label}
              </TabsTrigger>
            ))}
          </TabsList>
          <Button size="sm" disabled={props.isBusy || !props.hasUnsavedChanges} variant="outline" onClick={props.onSave}>
            <SaveIcon />
            保存
          </Button>
        </div>

        <TabsContent value="info" className="min-h-0 overflow-auto">
          <div className="space-y-4 rounded-xl border border-border/70 bg-muted/20 p-4">
            <div className="grid gap-3">
              <div className="space-y-1">
                <Label>目录名称</Label>
                <Input value={props.draft.name} onChange={event => props.onChangeDraft(draft => ({ ...draft, name: event.target.value }))} />
              </div>
              <div className="space-y-1">
                <Label>描述</Label>
                <Textarea value={props.draft.description} onChange={event => props.onChangeDraft(draft => ({ ...draft, description: event.target.value }))} />
              </div>
              <CollectionBaseUrlField
                value={props.draft.requestConfig.baseUrl}
                onChange={value => props.onChangeDraft(draft => ({
                  ...draft,
                  requestConfig: {
                    ...draft.requestConfig,
                    baseUrl: value,
                  },
                }))}
              />
            </div>
          </div>
        </TabsContent>
        <TabsContent value="headers" className="min-h-0 overflow-hidden">
          <RequestScopeConfigEditor activeTab="headers" config={props.draft.requestConfig} onChange={requestConfig => props.onChangeDraft(draft => ({ ...draft, requestConfig }))} title="目录默认请求设置" description="会叠加项目级设置，并被当前目录下的请求继续继承。" />
        </TabsContent>
        <TabsContent value="auth" className="min-h-0 overflow-hidden">
          <RequestScopeConfigEditor activeTab="auth" allowInherit config={props.draft.requestConfig} onChange={requestConfig => props.onChangeDraft(draft => ({ ...draft, requestConfig: normalizeScopeAuthMode(requestConfig, true) }))} title="目录默认认证设置" description="可以继承项目级认证，也可以在当前目录单独指定认证方式。" />
        </TabsContent>
        <TabsContent value="preRequestScript" className="min-h-0 overflow-hidden">
          <RequestScopeConfigEditor activeTab="preRequestScript" config={props.draft.requestConfig} onChange={requestConfig => props.onChangeDraft(draft => ({ ...draft, requestConfig }))} title="目录默认请求脚本" description="会在当前目录下请求自身脚本之前执行。" />
        </TabsContent>
        <TabsContent value="postRequestScript" className="min-h-0 overflow-hidden">
          <RequestScopeConfigEditor activeTab="postRequestScript" config={props.draft.requestConfig} onChange={requestConfig => props.onChangeDraft(draft => ({ ...draft, requestConfig }))} title="目录默认响应脚本" description="会在当前目录下请求自身响应脚本之前执行。" />
        </TabsContent>
      </Tabs>
    </div>
  )
}

function ProjectSettingsPane(props: {
  activeTab: SettingsPanelTab
  draft: ProjectEditorDraft
  hasUnsavedChanges: boolean
  isBusy: boolean
  onChangeDraft: (updater: (draft: ProjectEditorDraft) => ProjectEditorDraft) => void
  onActiveTabChange: (tab: WorkbenchPanelTab) => void
  onSave: () => void
}) {
  return (
    <div className="flex h-full min-h-0 flex-col overflow-hidden border-b border-border/70 px-3 py-3">
      <Tabs value={props.activeTab} onValueChange={value => props.onActiveTabChange(value as WorkbenchPanelTab)} className="h-full min-h-0">
        <div className="mb-3 flex shrink-0 items-center justify-between gap-3">
          <TabsList>
            {settingsTabs.map(tab => (
              <TabsTrigger key={tab.value} value={tab.value} className="px-1.5 py-0.5 text-[13px]">
                {tab.label}
              </TabsTrigger>
            ))}
          </TabsList>
          <Button size="sm" disabled={props.isBusy || !props.hasUnsavedChanges} variant="outline" onClick={props.onSave}>
            <SaveIcon />
            保存
          </Button>
        </div>

        <TabsContent value="info" className="min-h-0 overflow-auto">
          <div className="space-y-4 rounded-xl border border-border/70 bg-muted/20 p-4">
            <div className="grid gap-3">
              <div className="space-y-1">
                <Label>项目名称</Label>
                <Input value={props.draft.name} onChange={event => props.onChangeDraft(draft => ({ ...draft, name: event.target.value }))} />
              </div>
              <div className="space-y-1">
                <Label>描述</Label>
                <Textarea value={props.draft.description} onChange={event => props.onChangeDraft(draft => ({ ...draft, description: event.target.value }))} />
              </div>
              <ProjectBaseUrlField
                value={props.draft.requestConfig.baseUrl}
                onChange={value => props.onChangeDraft(draft => ({
                  ...draft,
                  requestConfig: {
                    ...draft.requestConfig,
                    baseUrl: value,
                  },
                }))}
              />
            </div>
          </div>
        </TabsContent>
        <TabsContent value="headers" className="min-h-0 overflow-hidden">
          <RequestScopeConfigEditor activeTab="headers" config={props.draft.requestConfig} onChange={requestConfig => props.onChangeDraft(draft => ({ ...draft, requestConfig }))} title="项目默认请求头" description="会作为整个项目的默认请求头，被所有目录和请求继承。" />
        </TabsContent>
        <TabsContent value="auth" className="min-h-0 overflow-hidden">
          <RequestScopeConfigEditor activeTab="auth" allowInherit={false} config={normalizeScopeAuthMode(props.draft.requestConfig, false)} onChange={requestConfig => props.onChangeDraft(draft => ({ ...draft, requestConfig: normalizeScopeAuthMode(requestConfig, false) }))} title="项目默认认证设置" description="会作为整个项目的认证基线，被所有目录和请求继承。" />
        </TabsContent>
        <TabsContent value="preRequestScript" className="min-h-0 overflow-hidden">
          <RequestScopeConfigEditor activeTab="preRequestScript" config={props.draft.requestConfig} onChange={requestConfig => props.onChangeDraft(draft => ({ ...draft, requestConfig }))} title="项目默认请求脚本" description="会在项目内所有请求的前置脚本之前执行。" />
        </TabsContent>
        <TabsContent value="postRequestScript" className="min-h-0 overflow-hidden">
          <RequestScopeConfigEditor activeTab="postRequestScript" config={props.draft.requestConfig} onChange={requestConfig => props.onChangeDraft(draft => ({ ...draft, requestConfig }))} title="项目默认响应脚本" description="会在项目内所有请求的响应脚本之前执行。" />
        </TabsContent>
      </Tabs>
    </div>
  )
}

function EnvironmentSettingsPane(props: {
  activeTab: SettingsPanelTab
  activeEnvironmentId: string | null
  environmentDrafts: Record<string, EnvironmentEditorDraft>
  environments: Environment[]
  draft: EnvironmentEditorDraft
  hasUnsavedChanges: boolean
  isBusy: boolean
  onActiveTabChange: (tab: WorkbenchPanelTab) => void
  onChangeDraft: (updater: (draft: EnvironmentEditorDraft) => EnvironmentEditorDraft) => void
  onDeleteEnvironment: (environmentId: string) => void
  onSave: () => void
  onSelectEnvironment: (environmentId: string | null) => void
  onSetActiveEnvironment: (environmentId: string | null) => void
  onStartCreateEnvironment: () => void
}) {
  const environmentTabs = React.useMemo(() => {
    const items = props.environments.map(environment => ({
      id: environment.id,
      label: props.environmentDrafts[environment.id]?.name || environment.name || '未命名环境',
    }))

    if (props.draft.id === '__new__') {
      items.push({
        id: '__new__',
        label: props.draft.name.trim() || '新的环境',
      })
    }

    return items
  }, [props.draft.id, props.draft.name, props.environmentDrafts, props.environments])

  const canDeleteCurrent = props.draft.id !== '__new__'
  const isCurrentActive = props.draft.id !== '__new__' && props.activeEnvironmentId === props.draft.id
  const hasEnvironments = environmentTabs.length > 0

  return (
    <div className="flex h-full min-h-0 flex-col overflow-hidden border-b border-border/70 px-3 py-3">
      <div className="mb-3 flex shrink-0 items-center justify-between gap-3">
        <div className="min-w-0 flex-1 overflow-hidden">
          <div className="flex items-center gap-2 overflow-x-auto pb-1">
            {environmentTabs.map(environment => (
              <button
                key={environment.id}
                type="button"
                onClick={() => props.onSelectEnvironment(environment.id === '__new__' ? null : environment.id)}
                className={cn(
                  'shrink-0 rounded-lg border px-3 py-1.5 text-sm transition',
                  environment.id === props.draft.id
                    ? 'border-border bg-accent text-foreground'
                    : 'border-transparent bg-muted/40 text-muted-foreground hover:bg-accent hover:text-foreground',
                )}
              >
                {environment.label}
              </button>
            ))}
          </div>
        </div>
        <Button size="sm" variant="outline" onClick={props.onStartCreateEnvironment}>
          <PlusIcon />
          新建环境
        </Button>
      </div>

      <div className="min-h-0 flex-1 overflow-auto">
        {!hasEnvironments && (
          <div className="grid h-full place-items-center">
            <Empty className="max-w-lg border border-dashed border-border/70 bg-muted/20">
              <EmptyHeader>
                <EmptyMedia variant="icon">
                  <CompassIcon />
                </EmptyMedia>
                <EmptyTitle>还没有环境</EmptyTitle>
                <EmptyDescription>
                  新建一个环境后，就可以在这里集中管理 Base URL 和环境变量。
                </EmptyDescription>
              </EmptyHeader>
              <EmptyContent>
                <Button variant="outline" onClick={props.onStartCreateEnvironment}>
                  <PlusIcon />
                  新建环境
                </Button>
              </EmptyContent>
            </Empty>
          </div>
        )}

        {hasEnvironments && (
          <div className="space-y-4 rounded-xl border border-border/70 bg-muted/20 p-4">
            <div className="flex items-center justify-between gap-3">
              <div>
                <h2 className="text-sm font-medium text-foreground">
                  {props.draft.name.trim() || (props.draft.id === '__new__' ? '新的环境' : '未命名环境')}
                </h2>
                <p className="mt-1 text-xs text-muted-foreground">
                  在这里管理当前环境的 Base URL 和环境变量。
                </p>
              </div>
              <div className="flex flex-wrap items-center gap-2">
                {isCurrentActive && (
                  <span className="rounded bg-primary/10 px-2 py-1 text-xs font-medium text-primary">
                    当前激活
                  </span>
                )}
                <Button
                  size="sm"
                  variant="destructive"
                  disabled={!canDeleteCurrent}
                  onClick={() => {
                    if (canDeleteCurrent) {
                      props.onDeleteEnvironment(props.draft.id)
                    }
                  }}
                >
                  <XIcon />
                  删除环境
                </Button>
                <Button
                  size="sm"
                  variant="outline"
                  disabled={props.draft.id === '__new__'}
                  onClick={() => props.onSetActiveEnvironment(isCurrentActive ? null : props.draft.id)}
                >
                  {isCurrentActive ? '取消激活' : '激活环境'}
                </Button>
                <Button size="sm" disabled={props.isBusy || !props.hasUnsavedChanges} variant="outline" onClick={props.onSave}>
                  <SaveIcon />
                  保存
                </Button>
              </div>
            </div>

            <div className="grid gap-3">
              <div className="space-y-1">
                <Label>环境名称</Label>
                <Input
                  value={props.draft.name}
                  onChange={event => props.onChangeDraft(draft => ({ ...draft, name: event.target.value }))}
                  placeholder="新的环境"
                />
              </div>
              <div className="space-y-1">
                <Label>Base URL</Label>
                <Input
                  value={props.draft.baseUrl}
                  onChange={event => props.onChangeDraft(draft => ({ ...draft, baseUrl: event.target.value }))}
                  placeholder="https://api.example.com"
                />
              </div>
              <div className="space-y-2">
                <div className="flex items-center justify-between">
                  <Label>环境变量</Label>
                  <Button
                    size="xs"
                    variant="outline"
                    onClick={() => props.onChangeDraft(draft => ({
                      ...draft,
                      variables: [
                        ...draft.variables,
                        {
                          id: crypto.randomUUID(),
                          key: '',
                          value: '',
                          enabled: true,
                          description: '',
                        },
                      ],
                    }))}
                  >
                    <PlusIcon className="size-4" />
                    添加变量
                  </Button>
                </div>
                <div className="space-y-2">
                  {props.draft.variables.map((variable, index) => (
                    <div key={variable.id} className="flex items-center gap-2 rounded-lg border border-border/60 bg-background/80 p-2">
                      <Checkbox
                        checked={variable.enabled}
                        onCheckedChange={checked => props.onChangeDraft(draft => ({
                          ...draft,
                          variables: draft.variables.map((item, itemIndex) =>
                            itemIndex === index ? { ...item, enabled: checked === true } : item,
                          ),
                        }))}
                      />
                      <Input
                        value={variable.key}
                        onChange={event => props.onChangeDraft(draft => ({
                          ...draft,
                          variables: draft.variables.map((item, itemIndex) =>
                            itemIndex === index ? { ...item, key: event.target.value } : item,
                          ),
                        }))}
                        placeholder="变量名"
                        className="w-40"
                      />
                      <Input
                        value={variable.value}
                        onChange={event => props.onChangeDraft(draft => ({
                          ...draft,
                          variables: draft.variables.map((item, itemIndex) =>
                            itemIndex === index ? { ...item, value: event.target.value } : item,
                          ),
                        }))}
                        placeholder="变量值"
                        className="flex-1"
                      />
                      <Button
                        variant="ghost"
                        size="icon-sm"
                        onClick={() => props.onChangeDraft(draft => ({
                          ...draft,
                          variables: draft.variables.filter((_, itemIndex) => itemIndex !== index),
                        }))}
                      >
                        <XIcon />
                      </Button>
                    </div>
                  ))}
                  {props.draft.variables.length === 0 && (
                    <div className="rounded-lg border border-dashed border-border/70 px-3 py-6 text-center text-sm text-muted-foreground">
                      还没有环境变量，添加后可在请求里使用环境变量占位符。
                    </div>
                  )}
                </div>
              </div>
            </div>
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

  const environments = useWorkbenchStore(state => state.environments)
  const activeEnvironmentId = useWorkbenchStore(state => state.activeEnvironmentId)

  const activeEnv = React.useMemo(() => {
    if (!activeEnvironmentId)
      return null
    return environments.find(e => e.id === activeEnvironmentId) ?? null
  }, [activeEnvironmentId, environments])

  const envVariables = React.useMemo(() => {
    if (!activeEnv)
      return []
    return activeEnv.variables.filter(v => v.enabled).map(v => ({
      key: v.key,
      value: v.value,
      description: v.description,
    }))
  }, [activeEnv])

  const handleFormatJson = React.useCallback(() => {
    const source = body.json.trim()
    if (!source) {
      return
    }

    try {
      const formatted = JSON.stringify(JSON.parse(source), null, 2)
      props.onChangeDraft(draft => ({
        ...draft,
        request: {
          ...draft.request,
          body: { ...draft.request.body, json: formatted },
        },
      }))
    }
    catch {
      toast.error('当前内容不是合法 JSON，暂时无法格式化')
    }
  }, [body.json, props.onChangeDraft])

  const createFormDataEntry = React.useCallback((entryType: 'text' | 'file' = 'text'): FormDataEntry => ({
    ...createKeyValueDraft(),
    entryType,
    filePath: '',
    fileName: '',
    contentType: '',
  }), [])

  const handlePickBinaryFile = React.useCallback(async () => {
    const selectedPath = await open({
      multiple: false,
      directory: false,
      title: '选择二进制请求体文件',
    })

    if (typeof selectedPath !== 'string') {
      return
    }

    props.onChangeDraft(draft => ({
      ...draft,
      request: {
        ...draft.request,
        body: {
          ...draft.request.body,
          binary: { filePath: selectedPath },
        },
      },
    }))
  }, [props.onChangeDraft])

  const handlePickFormDataFile = React.useCallback(async (rowId: string) => {
    const selectedPath = await open({
      multiple: false,
      directory: false,
      title: '选择 form-data 文件',
    })

    if (typeof selectedPath !== 'string') {
      return
    }

    const fileName = selectedPath.split(/[/\\]/).pop() ?? selectedPath
    props.onChangeDraft(draft => ({
      ...draft,
      request: {
        ...draft.request,
        body: {
          ...draft.request.body,
          formData: draft.request.body.formData.map(item => item.id === rowId
            ? {
                ...item,
                entryType: 'file',
                filePath: selectedPath,
                fileName,
                value: fileName,
              }
            : item),
        },
      },
    }))
  }, [props.onChangeDraft])

  return (
    <div className="flex h-full min-h-0 flex-col">
      {body.mode === 'none' && (
        <div className="grid min-h-[140px] place-items-center rounded-xl border border-dashed border-border/70 text-sm text-muted-foreground">
          当前请求不会发送请求体。
        </div>
      )}

      {body.mode === 'raw' && (
        <Textarea
          className="min-h-[220px] flex-1 overflow-auto font-mono text-xs"
          value={body.raw}
          onChange={event => props.onChangeDraft(draft => ({
            ...draft,
            request: {
              ...draft.request,
              body: { ...draft.request.body, raw: event.target.value },
            },
          }))}
          placeholder="请输入原始请求体"
        />
      )}

      {body.mode === 'json' && (
        <div className="relative flex min-h-0 flex-1 overflow-hidden">
          <Button
            type="button"
            size="icon-sm"
            variant="outline"
            className="absolute top-2 right-2 z-20 bg-background/88 shadow-sm backdrop-blur"
            aria-label="格式化 JSON"
            title="格式化 JSON"
            onClick={handleFormatJson}
          >
            <BracesIcon />
          </Button>

          <React.Suspense
            fallback={(
              <Textarea
                className="min-h-[220px] flex-1 overflow-auto font-mono text-xs"
                value={body.json}
                onChange={event => props.onChangeDraft(draft => ({
                  ...draft,
                  request: {
                    ...draft.request,
                    body: { ...draft.request.body, json: event.target.value },
                  },
                }))}
              />
            )}
          >
            <MonacoJsonEditor
              className="h-full min-h-[220px]"
              value={body.json}
              environmentVariables={envVariables}
              environmentName={activeEnv?.name}
              onChange={value => props.onChangeDraft(draft => ({
                ...draft,
                request: {
                  ...draft.request,
                  body: { ...draft.request.body, json: value },
                },
              }))}
            />
          </React.Suspense>
        </div>
      )}

      {body.mode === 'form-data' && (
        <div className="flex h-full min-h-0 flex-col">
          <div className="mb-3 flex items-center justify-end">
            <Button
              size="xs"
              variant="outline"
              onClick={() => props.onChangeDraft(draft => ({
                ...draft,
                request: {
                  ...draft.request,
                  body: {
                    ...draft.request.body,
                    formData: [...draft.request.body.formData, createFormDataEntry()],
                  },
                },
              }))}
            >
              <PlusIcon className="size-4" />
              新增字段
            </Button>
          </div>

          <div className="min-h-0 flex-1 space-y-1.5 overflow-auto pr-1">
            {body.formData.length > 0
              ? body.formData.map((row) => {
                  const entryType = row.entryType ?? 'text'
                  return (
                    <div
                      key={row.id}
                      className="grid grid-cols-[28px_112px_minmax(0,_1fr)_minmax(0,_1fr)_32px] items-center gap-1.5 rounded-xl border border-border/70 bg-muted/30 p-1.5"
                    >
                      <div className="flex items-center justify-center">
                        <Checkbox
                          checked={row.enabled}
                          onCheckedChange={checked => props.onChangeDraft(draft => ({
                            ...draft,
                            request: {
                              ...draft.request,
                              body: {
                                ...draft.request.body,
                                formData: draft.request.body.formData.map(item => item.id === row.id
                                  ? { ...item, enabled: Boolean(checked) }
                                  : item),
                              },
                            },
                          }))}
                        />
                      </div>
                      <Select
                        value={entryType}
                        onValueChange={value => props.onChangeDraft(draft => ({
                          ...draft,
                          request: {
                            ...draft.request,
                            body: {
                              ...draft.request.body,
                              formData: draft.request.body.formData.map(item => item.id === row.id
                                ? {
                                    ...item,
                                    entryType: value as 'text' | 'file',
                                    filePath: value === 'file' ? item.filePath : '',
                                    fileName: value === 'file' ? item.fileName : '',
                                    contentType: value === 'file' ? item.contentType : '',
                                  }
                                : item),
                            },
                          },
                        }))}
                      >
                        <SelectTrigger size="sm" className="w-full">
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value="text">文本</SelectItem>
                          <SelectItem value="file">文件</SelectItem>
                        </SelectContent>
                      </Select>
                      <Input
                        value={row.key}
                        onChange={event => props.onChangeDraft(draft => ({
                          ...draft,
                          request: {
                            ...draft.request,
                            body: {
                              ...draft.request.body,
                              formData: draft.request.body.formData.map(item => item.id === row.id
                                ? { ...item, key: event.target.value }
                                : item),
                            },
                          },
                        }))}
                        placeholder="字段名"
                      />
                      {entryType === 'file'
                        ? (
                            <div className="flex min-w-0 items-center gap-1.5">
                              <Input
                                value={row.filePath ?? ''}
                                readOnly
                                placeholder="请选择文件"
                                className="min-w-0"
                              />
                              <Button size="sm" variant="outline" onClick={() => { void handlePickFormDataFile(row.id) }}>
                                选择文件
                              </Button>
                            </div>
                          )
                        : (
                            <EnvironmentVariableInput
                              value={row.value}
                              onChange={newValue => props.onChangeDraft(draft => ({
                                ...draft,
                                request: {
                                  ...draft.request,
                                  body: {
                                    ...draft.request.body,
                                    formData: draft.request.body.formData.map(item => item.id === row.id
                                      ? { ...item, value: newValue }
                                      : item),
                                  },
                                },
                              }))}
                              variables={envVariables}
                              environmentName={activeEnv?.name ?? 'default'}
                              placeholder="字段值"
                            />
                          )}
                      <button
                        type="button"
                        onClick={() => props.onChangeDraft(draft => ({
                          ...draft,
                          request: {
                            ...draft.request,
                            body: {
                              ...draft.request.body,
                              formData: draft.request.body.formData.filter(item => item.id !== row.id),
                            },
                          },
                        }))}
                        className="rounded-md p-1.5 text-muted-foreground transition hover:bg-background hover:text-foreground"
                      >
                        <XIcon className="size-4" />
                      </button>
                    </div>
                  )
                })
              : (
                  <div className="grid min-h-[140px] place-items-center rounded-xl border border-dashed border-border/70 text-sm text-muted-foreground">
                    暂无 form-data 字段
                  </div>
                )}
          </div>
        </div>
      )}

      {body.mode === 'x-www-form-urlencoded' && (
        <div className="flex h-full min-h-0 flex-col">
          <div className="mb-3 flex items-center justify-end">
            <Button
              size="xs"
              variant="outline"
              onClick={() => props.onChangeDraft(draft => ({
                ...draft,
                request: {
                  ...draft.request,
                  body: {
                    ...draft.request.body,
                    urlEncoded: [...draft.request.body.urlEncoded, createKeyValueDraft()],
                  },
                },
              }))}
            >
              <PlusIcon className="size-4" />
              新增字段
            </Button>
          </div>
          <KeyValueTable
            emptyLabel="暂无 x-www-form-urlencoded 字段"
            rows={body.urlEncoded}
            environmentVariables={envVariables}
            environmentName={activeEnv?.name ?? 'default'}
            onChange={rows => props.onChangeDraft(draft => ({
              ...draft,
              request: {
                ...draft.request,
                body: { ...draft.request.body, urlEncoded: rows },
              },
            }))}
          />
        </div>
      )}

      {body.mode === 'binary' && (
        <div className="space-y-3">
          <div className="flex items-center gap-2">
            <Input
              value={body.binary.filePath ?? ''}
              readOnly
              placeholder="请选择要发送的文件"
            />
            <Button variant="outline" onClick={() => { void handlePickBinaryFile() }}>
              选择文件
            </Button>
          </div>
          <p className="text-xs text-muted-foreground">
            会直接以二进制方式发送所选文件内容。
          </p>
        </div>
      )}
    </div>
  )
}
