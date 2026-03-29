import type {
  EditorPanelTab,
  OpenRequestTab,
  RequestEditorDraft,
  ResponseState,
} from '../types'
import type { KeyValue } from '@/lib/workspace'
import { PointerActivationConstraints } from '@dnd-kit/dom'
import {
  DragDropProvider,
  DragOverlay,
  PointerSensor,
  useDraggable,
} from '@dnd-kit/react'
import { BracesIcon, CompassIcon, EllipsisIcon, Loader2Icon, PlusIcon, SaveIcon, SendHorizonalIcon, XIcon } from 'lucide-react'
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
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from '@/components/ui/resizable'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Textarea } from '@/components/ui/textarea'
import { cn } from '@/lib/utils'
import { useWorkbenchStore } from '../store/workbench-store'
import {
  editorTabs,
  macOSWindowChromeHeightClassName,
  methodOptions,
} from '../types'
import {
  buildUrlWithQuery,
  createKeyValueDraft,
  formatBytes,
  hasVisibleResponse,
  parseUrlQueryParams,
  startWindowDragging,
} from '../utils'
import { EnvironmentVariableInput } from './environment-variable-input'
import { MethodBadge, ResponseMetaBadge } from './shared'

interface RequestWorkspaceProps {
  activeDraft: RequestEditorDraft | null
  activeEditorTab: EditorPanelTab
  activeRequestId: string | null
  activeRequestIsLoading: boolean
  activeResponse: ResponseState | null
  dirtyRequestIds: Set<string>
  isBusy: boolean
  isMacOSDesktop: boolean
  openRequestTabs: OpenRequestTab[]
  pendingCloseRequestId: string | null
  splitRatio: number
  onActiveEditorTabChange: (tab: EditorPanelTab) => void
  onChangeDraft: (updater: (draft: RequestEditorDraft) => RequestEditorDraft) => void
  onCloseRequestDialogChange: (requestId: string | null) => void
  onCloseRequestTab: (requestId: string) => void
  onConfirmCloseRequestTab: () => void
  onFocusRequestTab: (requestId: string) => void
  onReorderRequestTabs: (tabs: OpenRequestTab[]) => void
  onSaveRequest: () => void
  onSendRequest: () => void
  onSplitRatioChange: (value: number) => void
}

export function RequestWorkspace(props: RequestWorkspaceProps) {
  const hasUnsavedChanges = Boolean(props.activeRequestId && props.dirtyRequestIds.has(props.activeRequestId))
  const shouldShowResponsePane = props.activeResponse ? hasVisibleResponse(props.activeResponse) : false
  const handleSaveShortcut = React.useEffectEvent(() => {
    if (!props.activeDraft || props.isBusy) {
      return
    }

    props.onSaveRequest()
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
                  onUrlChange={handleUrlChange}
                  onSaveRequest={props.onSaveRequest}
                  onSendRequest={props.onSendRequest}
                />

                {shouldShowResponsePane
                  ? (
                      <ResizablePanelGroup
                        id="request-workspace-panels"
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
                            activeTab={props.activeEditorTab}
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
                          activeTab={props.activeEditorTab}
                          draft={props.activeDraft}
                          onActiveTabChange={props.onActiveEditorTabChange}
                          onChangeDraft={props.onChangeDraft}
                          onQueryChange={handleQueryChange}
                        />
                      </div>
                    )}
              </>
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
        props.visualState === 'preview' && 'border-border/80 bg-accent',
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
          value={props.draft.url}
          onChange={event => props.onUrlChange(event.target.value)}
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
    </div>
  )
}

const requestBodyModeOptions = [
  { label: 'JSON', value: 'json' },
  { label: '原始文本', value: 'raw' },
  { label: '无', value: 'none' },
] as const

const requestBodyModeLabelMap = Object.fromEntries(
  requestBodyModeOptions.map(option => [option.value, option.label]),
) as Record<(typeof requestBodyModeOptions)[number]['value'], string>

interface RequestEditorTabsProps {
  activeTab: EditorPanelTab
  draft: RequestEditorDraft
  onActiveTabChange: (tab: EditorPanelTab) => void
  onChangeDraft: (updater: (draft: RequestEditorDraft) => RequestEditorDraft) => void
  onQueryChange: (query: KeyValue[]) => void
}

function RequestEditorTabs(props: RequestEditorTabsProps) {
  const bodyMode = props.draft.request.body.mode
  const bodyModeLabel = requestBodyModeLabelMap[bodyMode as keyof typeof requestBodyModeLabelMap] ?? bodyMode

  const activeProjectId = useWorkbenchStore(s => s.activeProjectId)
  const environments = useWorkbenchStore(s => s.environments)
  const activeEnvironmentId = useWorkbenchStore(s => s.activeEnvironmentId)

  const activeEnv = React.useMemo(() => {
    if (!activeProjectId)
      return null
    const projectEnvs = environments[activeProjectId]
    if (!projectEnvs)
      return null
    const envId = activeEnvironmentId[activeProjectId]
    if (!envId)
      return null
    return projectEnvs.find(e => e.id === envId) ?? null
  }, [activeProjectId, environments, activeEnvironmentId])

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
    if (props.activeTab === 'query') {
      props.onChangeDraft(draft => ({
        ...draft,
        request: {
          ...draft.request,
          query: [...draft.request.query, createKeyValueDraft()],
        },
      }))
      return
    }

    if (props.activeTab === 'headers') {
      props.onChangeDraft(draft => ({
        ...draft,
        request: {
          ...draft.request,
          headers: [...draft.request.headers, createKeyValueDraft()],
        },
      }))
    }
  }, [props.activeTab, props.onChangeDraft])

  return (
    <div className="flex h-full min-h-0 flex-col overflow-hidden border-b border-border/70 px-3 py-3">
      <Tabs value={props.activeTab} onValueChange={value => props.onActiveTabChange(value as EditorPanelTab)} className="h-full min-h-0">
        <div className="mb-3 flex shrink-0 items-center justify-between gap-3">
          <TabsList variant="line">
            {editorTabs.map(tab => (
              <TabsTrigger key={tab.value} value={tab.value} className="px-1.5 py-0.5 text-[13px]">
                {tab.label}
              </TabsTrigger>
            ))}
          </TabsList>

          <div className="flex shrink-0 items-center gap-2">
            {(props.activeTab === 'query' || props.activeTab === 'headers') && (
              <Button size="xs" variant="outline" onClick={handleAddKeyValueRow}>
                <PlusIcon className="size-4" />
                新增
              </Button>
            )}

            {props.activeTab === 'body' && (
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
                <SelectTrigger size="sm" className="w-[164px]">
                  <SelectValue>
                    {bodyModeLabel}
                  </SelectValue>
                </SelectTrigger>
                <SelectContent>
                  {requestBodyModeOptions.map(option => (
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

        <TabsContent value="body" className="min-h-0 overflow-auto">
          <BodyEditor draft={props.draft} onChangeDraft={props.onChangeDraft} />
        </TabsContent>

        <TabsContent value="preRequestScript" className="min-h-0">
          <PreRequestScriptEditor draft={props.draft} onChangeDraft={props.onChangeDraft} />
        </TabsContent>

        <TabsContent value="postRequestScript" className="min-h-0">
          <PostRequestScriptEditor draft={props.draft} onChangeDraft={props.onChangeDraft} />
        </TabsContent>
      </Tabs>
    </div>
  )
}

function PreRequestScriptEditor(props: {
  draft: RequestEditorDraft
  onChangeDraft: (updater: (draft: RequestEditorDraft) => RequestEditorDraft) => void
}) {
  return (
    <MonacoScriptEditor
      className="h-full min-h-[220px]"
      modelUri="file:///pre-request-script.js"
      language="javascript"
      value={props.draft.preRequestScript}
      onChange={(value) => {
        props.onChangeDraft(draft => ({ ...draft, preRequestScript: value }))
      }}
      placeholder="// 请求发送前执行，可通过 fuck.config 修改请求配置，通过 fuck.env 操作环境变量"
    />
  )
}

function PostRequestScriptEditor(props: {
  draft: RequestEditorDraft
  onChangeDraft: (updater: (draft: RequestEditorDraft) => RequestEditorDraft) => void
}) {
  return (
    <MonacoScriptEditor
      className="h-full min-h-[220px]"
      modelUri="file:///post-request-script.js"
      language="javascript"
      value={props.draft.postRequestScript}
      onChange={(value) => {
        props.onChangeDraft(draft => ({ ...draft, postRequestScript: value }))
      }}
      placeholder="// 请求响应后执行，可通过 fuck.response 访问响应内容"
    />
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

  return (
    <div className="flex h-full min-h-0 flex-col overflow-hidden">
      <div className="flex flex-wrap items-center gap-1.5 border-b border-border/70 px-3 py-2 text-[11px] text-muted-foreground">
        <ResponseMetaBadge label="状态" value={props.response.status !== null ? String(props.response.status) : '等待中'} />
        <ResponseMetaBadge label="耗时" value={`${props.response.durationMs} 毫秒`} />
        <ResponseMetaBadge label="大小" value={formatBytes(props.response.sizeBytes)} />
        <ResponseMetaBadge label="内容类型" value={props.response.contentType || '未知'} />
        {props.response.responseType && <ResponseMetaBadge label="视图" value={props.response.responseType === 'json' ? 'JSON' : '文本'} />}
      </div>

      <div className="flex min-h-0 flex-1 overflow-hidden px-3 py-3">
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
              : <TextResponseView body={props.response.body} contentType={props.response.contentType} />}
      </div>
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

interface KeyValueTableProps {
  emptyLabel: string
  rows: KeyValue[]
  environmentVariables: Array<{ key: string, value: string, description?: string }>
  environmentName: string
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

function BodyEditor(props: {
  draft: RequestEditorDraft
  onChangeDraft: (updater: (draft: RequestEditorDraft) => RequestEditorDraft) => void
}) {
  const body = props.draft.request.body

  const activeProjectId = useWorkbenchStore(state => state.activeProjectId)
  const environments = useWorkbenchStore(state => state.environments)
  const activeEnvironmentId = useWorkbenchStore(state => state.activeEnvironmentId)

  const activeEnv = React.useMemo(() => {
    if (!activeProjectId)
      return null
    const projectEnvs = environments[activeProjectId]
    if (!projectEnvs)
      return null
    const envId = activeEnvironmentId[activeProjectId]
    if (!envId)
      return null
    return projectEnvs.find(e => e.id === envId) ?? null
  }, [activeProjectId, environments, activeEnvironmentId])

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
    </div>
  )
}
