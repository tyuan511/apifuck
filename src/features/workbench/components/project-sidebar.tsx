import type { OpenRequestTab, TreeSelection } from '../types'
import type { ApiSummary, CollectionTreeNode, Environment, ProjectSnapshot, TreeNode } from '@/lib/project'
import { PointerActivationConstraints } from '@dnd-kit/dom'
import {
  DragDropProvider,
  DragOverlay,
  KeyboardSensor,
  PointerSensor,
  useDraggable,
  useDroppable,
} from '@dnd-kit/react'
import {
  CheckIcon,
  ChevronDownIcon,
  ChevronRightIcon,
  CircleIcon,
  EllipsisIcon,
  FolderIcon,
  FolderOpenIcon,
  FolderPlusIcon,
  GlobeIcon,
  PencilLineIcon,
  PlusIcon,
  Settings2Icon,
  Trash2Icon,
} from 'lucide-react'
import { AnimatePresence, motion } from 'motion/react'
import * as React from 'react'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { readProjectSummary } from '@/lib/project'
import { cn } from '@/lib/utils'
import { macOSWindowChromeHeightClassName } from '../types'
import { countApis, getProjectAvatarStyle, getProjectDisplayName, getProjectDisplayNameFromPath, getProjectMonogram, startWindowDragging } from '../utils'
import { MethodBadge, SidebarEmptyState } from './shared'

interface ProjectSidebarProps {
  collapsedCollectionIds: string[]
  environments: Environment[]
  activeEnvironmentId: string | null
  isMacOSDesktop: boolean
  openRequestTabs: OpenRequestTab[]
  project: ProjectSnapshot | null
  projectPath: string
  recentProjectPaths: string[]
  selectedTreeNode: TreeSelection | null
  onCreateCollection: (parentCollectionId: string | null) => void
  onCreateEnvironment: () => void
  onCreateProject: () => void
  onEditProject: () => void
  onCreateRequest: (parentCollectionId: string | null) => void
  onDeleteCollection: (node: CollectionTreeNode) => void
  onDeleteEnvironment: (environmentId: string) => void
  onDeleteRequest: (summary: ApiSummary) => void
  onEditCollection: (node: CollectionTreeNode) => void
  onEditEnvironment: (environment: Environment) => void
  onEditRequest: (summary: ApiSummary) => void
  onMoveTreeNode: (nodeId: string, targetParentCollectionId: string | null, position: number) => void | Promise<void>
  onOpenExistingProject: () => void | Promise<void>
  onRemoveRecentProject: (path: string, name: string) => void | Promise<void>
  onOpenSettings: () => void
  onSelectProject: (path: string) => void | Promise<void>
  onOpenRequest: (summary: ApiSummary, parentCollectionId: string | null) => void
  onSetActiveEnvironment: (environmentId: string | null) => void
  onToggleCollection: (collectionId: string) => void
  onTreeSelectionChange: (selection: TreeSelection | null) => void
}

const treeNodeDndType = 'workbench-tree-node'

type TreeDropKind = 'before' | 'after' | 'inside'

interface TreeDragItem {
  childCount?: number
  entityType: TreeNode['entityType']
  isCollapsed?: boolean
  isOpen?: boolean
  name: string
  nodeId: string
  parentCollectionId: string | null
  method?: string
  width?: number
}

interface TreeDropData {
  kind: TreeDropKind
  targetNodeId: string
}

interface TreeNodeRecord {
  ancestorCollectionIds: string[]
  index: number
  node: TreeNode
  parentCollectionId: string | null
}

interface TreeMoveRequest {
  nodeId: string
  position: number
  targetParentCollectionId: string | null
}

interface ResolvedTreeMove {
  position: number
  targetParentCollectionId: string | null
}

interface TreeDragPreview extends TreeDropData, ResolvedTreeMove {}

interface OptimisticTreeState {
  nodes: TreeNode[]
  requestId: number
}

type TreeDragMode = 'keyboard' | 'pointer'

type TreeRowVisualState = 'idle' | 'overlay' | 'preview' | 'selected'

interface VisibleTreeEntry {
  ancestorCollectionIds: string[]
  depth: number
  isCollapsed: boolean
  node: TreeNode
  parentCollectionId: string | null
}

interface TreeRowHitSnapshot {
  bottom: number
  left: number
  midY: number
  nodeId: string
  right: number
  top: number
}

interface TreeInsideHitSnapshot {
  collectionId: string
  depth: number
  intentBottom: number
  intentTop: number
  left: number
  right: number
  tailBottom: number
  tailTop: number
}

interface TreeHitSnapshot {
  insideCollections: TreeInsideHitSnapshot[]
  rows: TreeRowHitSnapshot[]
}

type ProviderDragStartEvent = Parameters<NonNullable<React.ComponentProps<typeof DragDropProvider>['onDragStart']>>[0]
type ProviderDragMoveEvent = Parameters<NonNullable<React.ComponentProps<typeof DragDropProvider>['onDragMove']>>[0]
type ProviderDragOverEvent = Parameters<NonNullable<React.ComponentProps<typeof DragDropProvider>['onDragOver']>>[0]
type ProviderDragEndEvent = Parameters<NonNullable<React.ComponentProps<typeof DragDropProvider>['onDragEnd']>>[0]

const treeLayoutTransition = {
  duration: 0.16,
  ease: [0.22, 1, 0.36, 1],
  type: 'tween',
} as const

const insideIntentEnterThreshold = 48
const insideIntentExitThreshold = 24

function buildTreeNodeRecordMap(
  nodes: TreeNode[],
  parentCollectionId: string | null = null,
  ancestorCollectionIds: string[] = [],
  map: Map<string, TreeNodeRecord> = new Map(),
) {
  nodes.forEach((node, index) => {
    map.set(node.id, {
      ancestorCollectionIds,
      index,
      node,
      parentCollectionId,
    })

    if (node.entityType === 'collection') {
      buildTreeNodeRecordMap(
        node.children,
        node.id,
        [...ancestorCollectionIds, node.id],
        map,
      )
    }
  })

  return map
}

function collectVisibleTreeEntries(
  nodes: TreeNode[],
  collapsedCollectionIds: Set<string>,
  parentCollectionId: string | null = null,
  ancestorCollectionIds: string[] = [],
  depth = 0,
  entries: VisibleTreeEntry[] = [],
) {
  for (const node of nodes) {
    const isCollapsed = node.entityType === 'collection' && collapsedCollectionIds.has(node.id)

    entries.push({
      ancestorCollectionIds,
      depth,
      isCollapsed,
      node,
      parentCollectionId,
    })

    if (node.entityType === 'collection' && !isCollapsed) {
      collectVisibleTreeEntries(
        node.children,
        collapsedCollectionIds,
        node.id,
        [...ancestorCollectionIds, node.id],
        depth + 1,
        entries,
      )
    }
  }

  return entries
}

function toScrollableContentY(rectTop: number, containerRect: DOMRect, scrollTop: number) {
  return rectTop - containerRect.top + scrollTop
}

function createTreeHitSnapshot(args: {
  childContainerElements: Map<string, HTMLDivElement>
  collapsedCollectionIds: Set<string>
  nodes: TreeNode[]
  rowElements: Map<string, HTMLDivElement>
  scrollElement: HTMLDivElement | null
}) {
  const { childContainerElements, collapsedCollectionIds, nodes, rowElements, scrollElement } = args
  if (!scrollElement) {
    return null
  }

  const containerRect = scrollElement.getBoundingClientRect()
  const scrollTop = scrollElement.scrollTop
  const visibleEntries = collectVisibleTreeEntries(nodes, collapsedCollectionIds)
  const lastVisibleDescendantRowByCollection = new Map<string, TreeRowHitSnapshot>()
  const rowSnapshotsByNodeId = new Map<string, TreeRowHitSnapshot>()
  const rows: TreeRowHitSnapshot[] = []
  const insideCollections: TreeInsideHitSnapshot[] = []

  for (const entry of visibleEntries) {
    const rowElement = rowElements.get(entry.node.id)
    if (!rowElement) {
      continue
    }

    const rowRect = rowElement.getBoundingClientRect()
    const top = toScrollableContentY(rowRect.top, containerRect, scrollTop)
    const bottom = toScrollableContentY(rowRect.bottom, containerRect, scrollTop)

    const rowSnapshot: TreeRowHitSnapshot = {
      bottom,
      left: rowRect.left,
      midY: top + ((bottom - top) / 2),
      nodeId: entry.node.id,
      right: rowRect.right,
      top,
    }

    rows.push(rowSnapshot)
    rowSnapshotsByNodeId.set(entry.node.id, rowSnapshot)

    for (const ancestorCollectionId of entry.ancestorCollectionIds) {
      lastVisibleDescendantRowByCollection.set(ancestorCollectionId, rowSnapshot)
    }
  }

  for (const entry of visibleEntries) {
    if (entry.node.entityType !== 'collection' || entry.isCollapsed) {
      continue
    }

    const collectionRow = rowSnapshotsByNodeId.get(entry.node.id)
    if (!collectionRow) {
      continue
    }

    const lastVisibleDescendantRow = lastVisibleDescendantRowByCollection.get(entry.node.id)
    const childContainerElement = childContainerElements.get(entry.node.id)
    const fallbackLeft = collectionRow.left + 18
    const fallbackRight = collectionRow.right

    if (childContainerElement) {
      const childRect = childContainerElement.getBoundingClientRect()
      const childTop = toScrollableContentY(childRect.top, containerRect, scrollTop)
      const childBottom = toScrollableContentY(childRect.bottom, containerRect, scrollTop)
      const isEmptyCollection = entry.node.children.length === 0
      const tailTop = isEmptyCollection
        ? Math.max(
            collectionRow.bottom + 8,
            childTop + ((childBottom - childTop) / 2),
          )
        : Math.max(
            lastVisibleDescendantRow?.midY ?? (childTop + ((childBottom - childTop) / 2)),
            childTop + 4,
          )
      const tailBottom = isEmptyCollection
        ? Math.max(childBottom + 10, tailTop + 18)
        : Math.max(childBottom + 10, tailTop + 18)
      const intentTop = isEmptyCollection
        ? Math.max(collectionRow.bottom + 2, childTop - 2)
        : collectionRow.midY
      const intentBottom = isEmptyCollection
        ? Math.max(childBottom + 18, intentTop + 32)
        : Math.max(childBottom + 10, intentTop + 24)

      insideCollections.push({
        collectionId: entry.node.id,
        depth: entry.depth,
        intentBottom,
        intentTop,
        left: childRect.left - 6,
        right: childRect.right,
        tailBottom,
        tailTop,
      })
      continue
    }

    const tailTop = lastVisibleDescendantRow?.midY ?? (collectionRow.bottom + 12)
    const tailBottom = Math.max((lastVisibleDescendantRow?.bottom ?? collectionRow.bottom) + 20, tailTop + 18)
    const intentTop = entry.node.children.length === 0
      ? collectionRow.bottom + 4
      : collectionRow.midY
    const intentBottom = Math.max((lastVisibleDescendantRow?.bottom ?? collectionRow.bottom) + 28, intentTop + 24)

    insideCollections.push({
      collectionId: entry.node.id,
      depth: entry.depth,
      intentBottom,
      intentTop,
      left: fallbackLeft,
      right: fallbackRight,
      tailBottom,
      tailTop,
    })
  }

  return rows.length > 0 ? { insideCollections, rows } satisfies TreeHitSnapshot : null
}

function insertAtPosition<T>(items: T[], item: T, position: number) {
  const nextItems = items.slice()
  const nextIndex = Math.max(0, Math.min(position, nextItems.length))
  nextItems.splice(nextIndex, 0, item)
  return nextItems
}

function removeTreeNode(nodes: TreeNode[], nodeId: string): { node: TreeNode | null, nodes: TreeNode[] } {
  for (let index = 0; index < nodes.length; index += 1) {
    const node = nodes[index]

    if (node.id === nodeId) {
      return {
        node,
        nodes: [...nodes.slice(0, index), ...nodes.slice(index + 1)],
      }
    }

    if (node.entityType !== 'collection') {
      continue
    }

    const nested = removeTreeNode(node.children, nodeId)
    if (!nested.node) {
      continue
    }

    const nextNodes = nodes.slice()
    nextNodes[index] = {
      ...node,
      children: nested.nodes,
    }

    return {
      node: nested.node,
      nodes: nextNodes,
    }
  }

  return { node: null, nodes }
}

function insertTreeNode(
  nodes: TreeNode[],
  parentCollectionId: string | null,
  position: number,
  node: TreeNode,
): { inserted: boolean, nodes: TreeNode[] } {
  if (!parentCollectionId) {
    return {
      inserted: true,
      nodes: insertAtPosition(nodes, node, position),
    }
  }

  for (let index = 0; index < nodes.length; index += 1) {
    const currentNode = nodes[index]

    if (currentNode.entityType !== 'collection') {
      continue
    }

    if (currentNode.id === parentCollectionId) {
      const nextNodes = nodes.slice()
      nextNodes[index] = {
        ...currentNode,
        children: insertAtPosition(currentNode.children, node, position),
      }

      return {
        inserted: true,
        nodes: nextNodes,
      }
    }

    const nested = insertTreeNode(currentNode.children, parentCollectionId, position, node)
    if (!nested.inserted) {
      continue
    }

    const nextNodes = nodes.slice()
    nextNodes[index] = {
      ...currentNode,
      children: nested.nodes,
    }

    return {
      inserted: true,
      nodes: nextNodes,
    }
  }

  return { inserted: false, nodes }
}

function applyTreeMove(nodes: TreeNode[], move: TreeMoveRequest) {
  const removed = removeTreeNode(nodes, move.nodeId)
  if (!removed.node) {
    return null
  }

  const inserted = insertTreeNode(
    removed.nodes,
    move.targetParentCollectionId,
    move.position,
    removed.node,
  )

  return inserted.inserted ? inserted.nodes : null
}

function getPreviewKey(preview: TreeDragPreview | null) {
  if (!preview) {
    return 'none'
  }

  return [
    preview.kind,
    preview.targetNodeId,
    preview.targetParentCollectionId ?? 'root',
    preview.position,
  ].join(':')
}

function getSiblingNodes(rootNodes: TreeNode[], records: Map<string, TreeNodeRecord>, parentCollectionId: string | null) {
  if (!parentCollectionId) {
    return rootNodes
  }

  const parent = records.get(parentCollectionId)?.node
  return parent?.entityType === 'collection' ? parent.children : []
}

function getDropData(value: unknown): TreeDropData | null {
  if (!value || typeof value !== 'object') {
    return null
  }

  const record = value as Partial<TreeDropData>
  if (
    (record.kind === 'before' || record.kind === 'after' || record.kind === 'inside')
    && typeof record.targetNodeId === 'string'
  ) {
    return {
      kind: record.kind,
      targetNodeId: record.targetNodeId,
    }
  }

  return null
}

function resolveTreeMove(
  draggedItem: TreeDragItem,
  dropData: TreeDropData,
  rootNodes: TreeNode[],
  records: Map<string, TreeNodeRecord>,
): ResolvedTreeMove | null {
  const sourceRecord = records.get(draggedItem.nodeId)
  const targetRecord = records.get(dropData.targetNodeId)

  if (!sourceRecord || !targetRecord || draggedItem.nodeId === dropData.targetNodeId) {
    return null
  }

  if (dropData.kind === 'inside') {
    if (targetRecord.node.entityType !== 'collection') {
      return null
    }

    if (
      draggedItem.entityType === 'collection'
      && targetRecord.ancestorCollectionIds.includes(draggedItem.nodeId)
    ) {
      return null
    }

    const siblings = targetRecord.node.children
    const position = siblings.length
    const sameParent = sourceRecord.parentCollectionId === targetRecord.node.id
    const normalizedPosition = sameParent && position > sourceRecord.index ? position - 1 : position

    if (sameParent && normalizedPosition === sourceRecord.index) {
      return null
    }

    return {
      position: normalizedPosition,
      targetParentCollectionId: targetRecord.node.id,
    }
  }

  const targetParentCollectionId = targetRecord.parentCollectionId
  const siblings = getSiblingNodes(rootNodes, records, targetParentCollectionId)
  const targetIndex = siblings.findIndex(node => node.id === dropData.targetNodeId)

  if (targetIndex < 0) {
    return null
  }

  const position = targetIndex + (dropData.kind === 'after' ? 1 : 0)
  const sameParent = sourceRecord.parentCollectionId === targetParentCollectionId
  const normalizedPosition = sameParent && position > sourceRecord.index ? position - 1 : position

  if (sameParent && normalizedPosition === sourceRecord.index) {
    return null
  }

  return {
    position: normalizedPosition,
    targetParentCollectionId,
  }
}

function getNodeDepth(record: TreeNodeRecord | null | undefined) {
  return record?.ancestorCollectionIds.length ?? 0
}

function getEffectiveDragDepth(
  records: Map<string, TreeNodeRecord>,
  draggedItem: TreeDragItem,
  activePreview: TreeDragPreview | null,
) {
  if (activePreview?.targetParentCollectionId) {
    const targetParentRecord = records.get(activePreview.targetParentCollectionId)
    return getNodeDepth(targetParentRecord) + 1
  }

  return getNodeDepth(records.get(draggedItem.nodeId))
}

function resolvePreviewFromPointer(args: {
  activePreview: TreeDragPreview | null
  clientX: number
  clientY: number
  draggedItem: TreeDragItem
  hitSnapshot: TreeHitSnapshot
  insideIntentActive: boolean
  records: Map<string, TreeNodeRecord>
  rootNodes: TreeNode[]
  scrollElement: HTMLDivElement | null
}) {
  const { activePreview, clientX, clientY, draggedItem, hitSnapshot, insideIntentActive, records, rootNodes, scrollElement } = args
  if (!scrollElement || hitSnapshot.rows.length === 0) {
    return null
  }

  const containerRect = scrollElement.getBoundingClientRect()
  const contentY = clientY - containerRect.top + scrollElement.scrollTop
  const effectiveDragDepth = getEffectiveDragDepth(records, draggedItem, activePreview)

  let bestIntentPreview: (TreeDragPreview & { _depth: number, _zoneTop: number }) | null = null
  let bestTailPreview: (TreeDragPreview & { _depth: number, _zoneTop: number }) | null = null

  for (const insideSnapshot of hitSnapshot.insideCollections) {
    if (
      clientX < insideSnapshot.left
      || clientX > insideSnapshot.right
    ) {
      continue
    }

    const insideDropData: TreeDropData = {
      kind: 'inside',
      targetNodeId: insideSnapshot.collectionId,
    }
    const insideMove = resolveTreeMove(draggedItem, insideDropData, rootNodes, records)

    if (!insideMove) {
      continue
    }

    const targetChildDepth = insideSnapshot.depth + 1

    if (
      insideIntentActive
      && targetChildDepth > effectiveDragDepth
      && contentY >= insideSnapshot.intentTop
      && contentY <= insideSnapshot.intentBottom
    ) {
      const preview = {
        ...insideDropData,
        ...insideMove,
        _depth: insideSnapshot.depth,
        _zoneTop: insideSnapshot.intentTop,
      }

      if (
        !bestIntentPreview
        || insideSnapshot.depth > bestIntentPreview._depth
        || (
          insideSnapshot.depth === bestIntentPreview._depth
          && insideSnapshot.intentTop > bestIntentPreview._zoneTop
        )
      ) {
        bestIntentPreview = preview
      }
    }

    if (
      contentY < insideSnapshot.tailTop
      || contentY > insideSnapshot.tailBottom
    ) {
      continue
    }

    const preview = {
      ...insideDropData,
      ...insideMove,
      _depth: insideSnapshot.depth,
      _zoneTop: insideSnapshot.tailTop,
    }

    if (
      !bestTailPreview
      || insideSnapshot.depth > bestTailPreview._depth
      || (
        insideSnapshot.depth === bestTailPreview._depth
        && insideSnapshot.tailTop > bestTailPreview._zoneTop
      )
    ) {
      bestTailPreview = preview
    }
  }

  if (bestIntentPreview) {
    const { _depth: _ignoredDepth, _zoneTop: _ignoredZoneTop, ...resolvedPreview } = bestIntentPreview
    return resolvedPreview
  }

  if (bestTailPreview) {
    const { _depth: _ignoredDepth, _zoneTop: _ignoredZoneTop, ...resolvedPreview } = bestTailPreview
    return resolvedPreview
  }

  const beforeTarget = hitSnapshot.rows.find(snapshot => contentY < snapshot.midY)
  const fallbackTarget = beforeTarget ?? hitSnapshot.rows.at(-1)

  if (!fallbackTarget) {
    return null
  }

  const dropData: TreeDropData = beforeTarget
    ? { kind: 'before', targetNodeId: beforeTarget.nodeId }
    : { kind: 'after', targetNodeId: fallbackTarget.nodeId }
  const move = resolveTreeMove(draggedItem, dropData, rootNodes, records)

  return move
    ? {
        ...dropData,
        ...move,
      }
    : null
}

export function ProjectSidebar(props: ProjectSidebarProps) {
  const activeProject = props.project
  const activeProjectApiCount = activeProject ? countApis(activeProject.children) : 0
  const activeProjectName = getProjectDisplayName(activeProject)
  const activeProjectAvatarSeed = activeProject ? props.projectPath : activeProjectName
  const projectPaths = React.useMemo(() => {
    const deduped = new Set<string>()
    const paths = [props.projectPath, ...props.recentProjectPaths]
      .map(path => path.trim())
      .filter(Boolean)
      .filter((path) => {
        if (deduped.has(path)) {
          return false
        }
        deduped.add(path)
        return true
      })

    return paths
  }, [props.projectPath, props.recentProjectPaths])
  const [projectNamesByPath, setProjectNamesByPath] = React.useState<Record<string, string>>({})

  React.useEffect(() => {
    let cancelled = false

    async function loadProjectNames() {
      const entries = await Promise.all(projectPaths.map(async (path) => {
        if (path === props.projectPath && activeProject) {
          return [path, getProjectDisplayName(activeProject)] as const
        }

        try {
          const metadata = await readProjectSummary(path)
          return [path, getProjectDisplayName({ metadata, children: [], environments: [] })] as const
        }
        catch {
          return [path, getProjectDisplayNameFromPath(path, activeProject, props.projectPath)] as const
        }
      }))

      if (!cancelled) {
        setProjectNamesByPath(Object.fromEntries(entries))
      }
    }

    void loadProjectNames()

    return () => {
      cancelled = true
    }
  }, [activeProject, projectPaths, props.projectPath])
  const collapsedCollectionIdSet = React.useMemo(
    () => new Set(props.collapsedCollectionIds),
    [props.collapsedCollectionIds],
  )
  const nodeRecords = React.useMemo(
    () => buildTreeNodeRecordMap(activeProject?.children ?? []),
    [activeProject],
  )
  const [activeDragItem, setActiveDragItem] = React.useState<TreeDragItem | null>(null)
  const [activeDragMode, setActiveDragMode] = React.useState<TreeDragMode | null>(null)
  const [activePreview, setActivePreview] = React.useState<TreeDragPreview | null>(null)
  const [optimisticTree, setOptimisticTree] = React.useState<OptimisticTreeState | null>(null)
  const childContainerElementsRef = React.useRef(new Map<string, HTMLDivElement>())
  const dragStartClientXRef = React.useRef<number | null>(null)
  const hitSnapshotRef = React.useRef<TreeHitSnapshot | null>(null)
  const insideIntentActiveRef = React.useRef(false)
  const optimisticRequestIdRef = React.useRef(0)
  const previewKeyRef = React.useRef(getPreviewKey(null))
  const rowElementsRef = React.useRef(new Map<string, HTMLDivElement>())
  const treeScrollRef = React.useRef<HTMLDivElement | null>(null)

  const setCollectionChildContainerElement = React.useCallback((collectionId: string, element: HTMLDivElement | null) => {
    const nextMap = childContainerElementsRef.current
    if (element) {
      nextMap.set(collectionId, element)
    }
    else {
      nextMap.delete(collectionId)
    }
  }, [])

  const setPreviewIfChanged = React.useCallback((nextPreview: TreeDragPreview | null) => {
    const nextKey = getPreviewKey(nextPreview)
    if (previewKeyRef.current === nextKey) {
      return
    }

    previewKeyRef.current = nextKey
    setActivePreview(nextPreview)
  }, [])

  const setRowElement = React.useCallback((nodeId: string, element: HTMLDivElement | null) => {
    const nextMap = rowElementsRef.current
    if (element) {
      nextMap.set(nodeId, element)
    }
    else {
      nextMap.delete(nodeId)
    }
  }, [])

  const clearDragState = React.useCallback(() => {
    dragStartClientXRef.current = null
    hitSnapshotRef.current = null
    insideIntentActiveRef.current = false
    previewKeyRef.current = getPreviewKey(null)
    setActiveDragItem(null)
    setActiveDragMode(null)
    setPreviewIfChanged(null)
  }, [setPreviewIfChanged])

  React.useEffect(() => {
    clearDragState()
    setOptimisticTree(null)
  }, [activeProject?.metadata.id, clearDragState])

  const previewNodes = React.useMemo(() => {
    if (!activeProject || !activeDragItem || !activePreview) {
      return null
    }

    return applyTreeMove(activeProject.children, {
      nodeId: activeDragItem.nodeId,
      position: activePreview.position,
      targetParentCollectionId: activePreview.targetParentCollectionId,
    })
  }, [activeDragItem, activePreview, activeProject])

  React.useLayoutEffect(() => {
    if (activeDragMode !== 'pointer' || !activeDragItem || !activeProject) {
      return
    }

    hitSnapshotRef.current = createTreeHitSnapshot({
      childContainerElements: childContainerElementsRef.current,
      collapsedCollectionIds: collapsedCollectionIdSet,
      nodes: activeProject.children,
      rowElements: rowElementsRef.current,
      scrollElement: treeScrollRef.current,
    })
  }, [activeDragItem, activeDragMode, activeProject, collapsedCollectionIdSet])

  React.useEffect(() => {
    const scrollElement = treeScrollRef.current
    if (!scrollElement || activeDragMode !== 'pointer' || !activeDragItem || !activeProject) {
      return
    }

    const handleScroll = () => {
      hitSnapshotRef.current = createTreeHitSnapshot({
        childContainerElements: childContainerElementsRef.current,
        collapsedCollectionIds: collapsedCollectionIdSet,
        nodes: previewNodes ?? activeProject.children,
        rowElements: rowElementsRef.current,
        scrollElement,
      })
    }

    scrollElement.addEventListener('scroll', handleScroll, { passive: true })
    return () => {
      scrollElement.removeEventListener('scroll', handleScroll)
    }
  }, [activeDragItem, activeDragMode, activeProject, collapsedCollectionIdSet, previewNodes])

  const visibleNodes = previewNodes ?? optimisticTree?.nodes ?? activeProject?.children ?? []

  const handleDragStart = React.useCallback((event: ProviderDragStartEvent) => {
    const sourceData = event.operation.source?.data ?? {}
    const sourceElement = event.operation.source?.element
    const nativeEvent = event.nativeEvent
    const isPointerDrag = nativeEvent instanceof MouseEvent || nativeEvent instanceof PointerEvent
    if (
      typeof sourceData.nodeId === 'string'
      && typeof sourceData.name === 'string'
      && (sourceData.entityType === 'api' || sourceData.entityType === 'collection')
    ) {
      hitSnapshotRef.current = createTreeHitSnapshot({
        childContainerElements: childContainerElementsRef.current,
        collapsedCollectionIds: collapsedCollectionIdSet,
        nodes: activeProject?.children ?? [],
        rowElements: rowElementsRef.current,
        scrollElement: treeScrollRef.current,
      })
      setOptimisticTree(null)
      dragStartClientXRef.current = isPointerDrag ? nativeEvent.clientX : null
      insideIntentActiveRef.current = false
      setActiveDragItem({
        childCount: typeof sourceData.childCount === 'number' ? sourceData.childCount : undefined,
        entityType: sourceData.entityType,
        isCollapsed: typeof sourceData.isCollapsed === 'boolean' ? sourceData.isCollapsed : undefined,
        isOpen: typeof sourceData.isOpen === 'boolean' ? sourceData.isOpen : undefined,
        method: typeof sourceData.method === 'string' ? sourceData.method : undefined,
        name: sourceData.name,
        nodeId: sourceData.nodeId,
        parentCollectionId: typeof sourceData.parentCollectionId === 'string' ? sourceData.parentCollectionId : null,
        width: sourceElement instanceof HTMLElement ? sourceElement.getBoundingClientRect().width : undefined,
      })
      setActiveDragMode(nativeEvent instanceof KeyboardEvent ? 'keyboard' : 'pointer')
      setPreviewIfChanged(null)
    }
  }, [activeProject?.children, collapsedCollectionIdSet, setPreviewIfChanged])

  const handleDragMove = React.useCallback((event: ProviderDragMoveEvent) => {
    if (activeDragMode !== 'pointer' || !activeDragItem || !activeProject) {
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

    const dragStartClientX = dragStartClientXRef.current
    if (typeof dragStartClientX === 'number') {
      const deltaX = nativeEvent.clientX - dragStartClientX
      if (!insideIntentActiveRef.current && deltaX >= insideIntentEnterThreshold) {
        insideIntentActiveRef.current = true
      }
      else if (insideIntentActiveRef.current && deltaX <= insideIntentExitThreshold) {
        insideIntentActiveRef.current = false
      }
    }

    setPreviewIfChanged(resolvePreviewFromPointer({
      activePreview,
      clientX: nativeEvent.clientX,
      clientY: nativeEvent.clientY,
      draggedItem: activeDragItem,
      hitSnapshot,
      insideIntentActive: insideIntentActiveRef.current,
      records: nodeRecords,
      rootNodes: activeProject.children,
      scrollElement: treeScrollRef.current,
    }))
  }, [activeDragItem, activeDragMode, activePreview, activeProject, nodeRecords, setPreviewIfChanged])

  const handleDragOver = React.useCallback((event: ProviderDragOverEvent) => {
    if (activeDragMode !== 'keyboard') {
      return
    }

    if (!activeDragItem || !activeProject) {
      setPreviewIfChanged(null)
      return
    }

    const dropData = getDropData(event.operation.target?.data)
    const nextMove = dropData
      ? resolveTreeMove(activeDragItem, dropData, activeProject.children, nodeRecords)
      : null

    setPreviewIfChanged(nextMove ? { ...dropData!, ...nextMove } : null)
  }, [activeDragItem, activeDragMode, activeProject, nodeRecords, setPreviewIfChanged])

  const handleDragEnd = React.useCallback((event: ProviderDragEndEvent) => {
    if (event.canceled || !activeDragItem || !activeProject) {
      clearDragState()
      return
    }

    const dropData = getDropData(event.operation.target?.data)
    const nextMove = activeDragMode === 'pointer'
      ? activePreview
      : (
          dropData
            ? resolveTreeMove(activeDragItem, dropData, activeProject.children, nodeRecords)
            : activePreview
        )

    if (!nextMove) {
      clearDragState()
      return
    }

    const nextNodes = applyTreeMove(activeProject.children, {
      nodeId: activeDragItem.nodeId,
      position: nextMove.position,
      targetParentCollectionId: nextMove.targetParentCollectionId,
    })

    if (!nextNodes) {
      clearDragState()
      return
    }

    const requestId = optimisticRequestIdRef.current + 1
    optimisticRequestIdRef.current = requestId

    setOptimisticTree({
      nodes: nextNodes,
      requestId,
    })
    clearDragState()

    const result = props.onMoveTreeNode(
      activeDragItem.nodeId,
      nextMove.targetParentCollectionId,
      nextMove.position,
    )

    if (result && typeof result.then === 'function') {
      void result.finally(() => {
        setOptimisticTree(current => (current?.requestId === requestId ? null : current))
      })
      return
    }

    setOptimisticTree(current => (current?.requestId === requestId ? null : current))
  }, [activeDragItem, activeDragMode, activePreview, activeProject, clearDragState, nodeRecords, props])

  return (
    <aside className="flex min-h-0 flex-col border-r border-border/70 bg-sidebar/50">
      {props.isMacOSDesktop && (
        <div
          onMouseDown={startWindowDragging}
          className={cn('shrink-0', macOSWindowChromeHeightClassName)}
        />
      )}
      <div className="px-2.5 pb-2">
        <DropdownMenu>
          <DropdownMenuTrigger
            render={(
              <Button
                variant="ghost"
                className="h-auto w-full justify-start rounded-xl border border-transparent bg-muted px-2.5 py-2.5 hover:bg-accent/80 data-[state=open]:border-border/70 data-[state=open]:bg-accent"
              />
            )}
          >
            <span className="flex min-w-0 flex-1 items-center gap-2.5">
              <span
                className="flex size-9 shrink-0 items-center justify-center rounded-lg text-sm font-semibold"
                style={getProjectAvatarStyle(activeProjectAvatarSeed)}
              >
                {getProjectMonogram(activeProject ? activeProjectName : '项目')}
              </span>
              <span className="min-w-0 flex-1 text-left">
                <span className="block truncate text-[14px] font-semibold tracking-tight text-foreground">
                  {activeProjectName}
                </span>
                <span className="block truncate text-[11px] text-muted-foreground">
                  {activeProject ? `${activeProjectApiCount} 个接口` : '未选择项目'}
                </span>
              </span>
              <ChevronDownIcon className="size-4 shrink-0 text-muted-foreground" />
            </span>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="start" sideOffset={6} className="w-76 rounded-xl">
            <div className="flex items-center justify-between px-2 py-1">
              <div className="text-[11px] font-medium text-muted-foreground">
                项目列表
              </div>
              <div className="flex items-center gap-1">
                <button
                  type="button"
                  onClick={props.onOpenExistingProject}
                  className="rounded-lg p-1.5 text-muted-foreground transition hover:bg-accent hover:text-foreground"
                  aria-label="打开已有项目"
                  title="打开已有项目"
                >
                  <FolderOpenIcon className="size-4" />
                </button>
                <button
                  type="button"
                  onClick={props.onCreateProject}
                  className="rounded-lg p-1.5 text-muted-foreground transition hover:bg-accent hover:text-foreground"
                  aria-label="新建项目"
                  title="新建项目"
                >
                  <PlusIcon className="size-4" />
                </button>
              </div>
            </div>
            <DropdownMenuSeparator className="my-1" />
            {projectPaths.length > 0
              ? projectPaths.map((path) => {
                  const canRemove = path !== props.projectPath
                  const projectName = projectNamesByPath[path] ?? getProjectDisplayNameFromPath(path, activeProject, props.projectPath)
                  return (
                    <DropdownMenuItem key={path} onClick={() => props.onSelectProject(path)} className="group rounded-lg p-1.5">
                      <span className="flex min-w-0 flex-1 items-center gap-2">
                        <span
                          className="flex size-8 shrink-0 items-center justify-center rounded-md text-xs font-semibold"
                          style={getProjectAvatarStyle(path)}
                        >
                          {getProjectMonogram(projectName)}
                        </span>
                        <span className="min-w-0 flex-1">
                          <span className="block truncate text-sm font-medium">
                            {projectName}
                          </span>
                          <span className="block truncate text-[11px] text-muted-foreground">
                            {path}
                          </span>
                        </span>
                        <div className="relative flex size-12 shrink-0 items-center justify-end">
                          {path === props.projectPath && (
                            <CheckIcon className="size-4 shrink-0 text-primary transition group-hover:opacity-0" />
                          )}
                          <div className="invisible absolute right-0 flex items-center gap-1 opacity-0 transition group-hover:visible group-hover:opacity-100">
                            {path === props.projectPath && (
                              <button
                                type="button"
                                className="flex size-6 items-center justify-center rounded-md text-muted-foreground hover:bg-accent hover:text-foreground"
                                aria-label="编辑项目"
                                title="编辑项目"
                                onClick={(event) => {
                                  event.preventDefault()
                                  event.stopPropagation()
                                  props.onEditProject()
                                }}
                              >
                                <PencilLineIcon className="size-3.5" />
                              </button>
                            )}
                            {canRemove && (
                              <button
                                type="button"
                                className="flex size-6 items-center justify-center rounded-md text-muted-foreground hover:bg-accent hover:text-destructive"
                                aria-label="删除项目"
                                title="删除项目"
                                onClick={(event) => {
                                  event.preventDefault()
                                  event.stopPropagation()
                                  void props.onRemoveRecentProject(path, projectName)
                                }}
                              >
                                <Trash2Icon className="size-3.5" />
                              </button>
                            )}
                          </div>
                        </div>
                      </span>
                    </DropdownMenuItem>
                  )
                })
              : (
                  <div className="px-2 py-3 text-sm text-muted-foreground">
                    暂无项目
                  </div>
                )}
          </DropdownMenuContent>
        </DropdownMenu>
      </div>

      {activeProject && (
        <EnvironmentSection
          environments={props.environments}
          activeEnvironmentId={props.activeEnvironmentId}
          onCreateEnvironment={props.onCreateEnvironment}
          onEditEnvironment={props.onEditEnvironment}
          onDeleteEnvironment={props.onDeleteEnvironment}
          onSetActiveEnvironment={props.onSetActiveEnvironment}
        />
      )}

      <DragDropProvider
        sensors={[
          PointerSensor.configure({
            activationConstraints: [new PointerActivationConstraints.Distance({ value: 6 })],
            preventActivation(event) {
              return event.target instanceof HTMLElement && Boolean(event.target.closest('[data-tree-action]'))
            },
          }),
          KeyboardSensor,
        ]}
        onDragEnd={handleDragEnd}
        onDragMove={handleDragMove}
        onDragOver={handleDragOver}
        onDragStart={handleDragStart}
      >
        <div className="min-h-0 flex flex-1 flex-col pt-3">
          <div className="group mb-2 flex items-center justify-between px-2.5">
            <span className="text-[11px] font-medium text-muted-foreground">
              请求
            </span>
            <DropdownMenu>
              <DropdownMenuTrigger
                render={(
                  <Button className="opacity-0 transition group-hover:opacity-100 group-focus-within:opacity-100" size="icon-xs" variant="ghost" />
                )}
              >
                <EllipsisIcon />
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end" className="w-36">
                <DropdownMenuItem onClick={() => props.onCreateRequest(null)}>
                  <PlusIcon />
                  新建请求
                </DropdownMenuItem>
                <DropdownMenuItem onClick={() => props.onCreateCollection(null)}>
                  <FolderPlusIcon />
                  新建集合
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          </div>
          <div ref={treeScrollRef} className="min-h-0 overflow-auto pb-2">
            {activeProject
              ? (
                  <div className="space-y-1 px-2.5">
                    {visibleNodes.length > 0
                      ? (
                          <TreeNodeList
                            activeDragItem={activeDragItem}
                            activePreview={activePreview}
                            collapsedCollectionIds={props.collapsedCollectionIds}
                            nodes={visibleNodes}
                            openRequestTabs={props.openRequestTabs}
                            selectedTreeNode={props.selectedTreeNode}
                            setCollectionChildContainerElement={setCollectionChildContainerElement}
                            setRowElement={setRowElement}
                            onCreateCollection={props.onCreateCollection}
                            onCreateRequest={props.onCreateRequest}
                            onDeleteCollection={props.onDeleteCollection}
                            onDeleteRequest={props.onDeleteRequest}
                            onEditCollection={props.onEditCollection}
                            onEditRequest={props.onEditRequest}
                            onOpenRequest={props.onOpenRequest}
                            onToggleCollection={props.onToggleCollection}
                            onTreeSelectionChange={props.onTreeSelectionChange}
                          />
                        )
                      : <SidebarEmptyState title="还没有请求" body="新建集合或请求" />}
                  </div>
                )
              : (
                  <div className="px-2.5">
                    <SidebarEmptyState title="还没有项目" body="先创建一个项目，再开始使用" />
                  </div>
                )}
          </div>
        </div>

        <DragOverlay
          dropAnimation={{
            duration: 180,
            easing: 'cubic-bezier(0.22, 1, 0.36, 1)',
          }}
        >
          {activeDragItem
            ? (
                <TreeDragOverlay item={activeDragItem} />
              )
            : null}
        </DragOverlay>
      </DragDropProvider>

      <div className="px-2.5 pt-3 pb-2.5">
        <button
          type="button"
          className="flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-left text-[13px] text-muted-foreground transition hover:bg-accent hover:text-foreground"
          onClick={props.onOpenSettings}
        >
          <Settings2Icon className="size-4" />
          设置
        </button>
      </div>
    </aside>
  )
}

interface RequestTreeNodeProps {
  activeDragItem: TreeDragItem | null
  activePreview: TreeDragPreview | null
  collapsedCollectionIds: string[]
  node: TreeNode
  openRequestTabs: OpenRequestTab[]
  selectedTreeNode: TreeSelection | null
  setCollectionChildContainerElement: (collectionId: string, element: HTMLDivElement | null) => void
  setRowElement: (nodeId: string, element: HTMLDivElement | null) => void
  onCreateCollection: (parentCollectionId: string | null) => void
  onCreateRequest: (parentCollectionId: string | null) => void
  onDeleteCollection: (node: CollectionTreeNode) => void
  onDeleteRequest: (summary: ApiSummary) => void
  onEditCollection: (node: CollectionTreeNode) => void
  onEditRequest: (summary: ApiSummary) => void
  onOpenRequest: (summary: ApiSummary, parentCollectionId: string | null) => void
  onToggleCollection: (collectionId: string) => void
  onTreeSelectionChange: (selection: TreeSelection | null) => void
  parentCollectionId?: string | null
}

interface TreeNodeListProps extends Omit<RequestTreeNodeProps, 'node'> {
  nodes: TreeNode[]
}

function TreeNodeList(props: TreeNodeListProps) {
  return (
    <AnimatePresence mode="popLayout">
      {props.nodes.map(node => (
        <motion.div
          key={node.id}
          layout="position"
          transition={treeLayoutTransition}
        >
          <RequestTreeNode
            {...props}
            node={node}
          />
        </motion.div>
      ))}
    </AnimatePresence>
  )
}

function TreeRowShell(props: {
  children: React.ReactNode
  className?: string
  interactive?: boolean
  rowRef?: ((element: HTMLDivElement | null) => void) | React.RefObject<HTMLDivElement | null>
  visualState: TreeRowVisualState
}) {
  return (
    <div
      ref={props.rowRef}
      className={cn(
        'group relative flex items-center gap-1 rounded-lg border border-transparent pr-1 transition',
        props.interactive && 'cursor-grab active:cursor-grabbing hover:bg-accent',
        props.visualState === 'selected' && 'bg-accent text-foreground',
        props.visualState === 'overlay' && 'bg-accent text-foreground shadow-sm',
        props.visualState === 'preview' && 'border-primary/70 bg-primary/[0.08] text-foreground opacity-75',
        props.className,
      )}
    >
      {props.children}
    </div>
  )
}

function CollectionTreeActions(props: {
  childCount: number
  node: CollectionTreeNode
  onCreateCollection: (parentCollectionId: string | null) => void
  onCreateRequest: (parentCollectionId: string | null) => void
  onDeleteCollection: (node: CollectionTreeNode) => void
  onEditCollection: (node: CollectionTreeNode) => void
}) {
  return (
    <>
      <span className="text-[11px] text-muted-foreground transition group-hover:opacity-0 group-focus-within:opacity-0">
        {props.childCount}
      </span>
      <DropdownMenu>
        <DropdownMenuTrigger
          render={(
            <Button
              data-tree-action
              className="absolute opacity-0 transition group-hover:opacity-100 group-focus-within:opacity-100"
              size="icon-xs"
              variant="ghost"
            />
          )}
        >
          <EllipsisIcon />
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" className="w-32">
          <DropdownMenuItem onClick={() => props.onCreateRequest(props.node.id)}>
            <PlusIcon />
            新建请求
          </DropdownMenuItem>
          <DropdownMenuItem onClick={() => props.onCreateCollection(props.node.id)}>
            <FolderPlusIcon />
            新建目录
          </DropdownMenuItem>
          <DropdownMenuItem variant="destructive" onClick={() => props.onDeleteCollection(props.node)}>
            <Trash2Icon />
            删除目录
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </>
  )
}

function ApiTreeActions(props: {
  isOpen: boolean
  node: ApiSummary
  onDeleteRequest: (summary: ApiSummary) => void
  onEditRequest: (summary: ApiSummary) => void
}) {
  return (
    <>
      {props.isOpen && (
        <span className="size-1 rounded-full bg-primary transition group-hover:opacity-0 group-focus-within:opacity-0" />
      )}
      <DropdownMenu>
        <DropdownMenuTrigger
          render={(
            <Button
              data-tree-action
              className={cn(
                'absolute opacity-0 transition group-hover:opacity-100 group-focus-within:opacity-100',
                props.isOpen && 'group-hover:opacity-100 group-focus-within:opacity-100',
              )}
              size="icon-xs"
              variant="ghost"
            />
          )}
        >
          <EllipsisIcon />
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" className="w-32">
          <DropdownMenuItem onClick={() => props.onEditRequest(props.node)}>
            <PencilLineIcon />
            编辑
          </DropdownMenuItem>
          <DropdownMenuItem variant="destructive" onClick={() => props.onDeleteRequest(props.node)}>
            <Trash2Icon />
            删除
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </>
  )
}

function CollectionTreeRow(props: {
  action?: React.ReactNode
  childCount: number
  interactive?: boolean
  isCollapsed: boolean
  name: string
  onChevronClick?: () => void
  onClick?: () => void
  rowRef?: ((element: HTMLDivElement | null) => void) | React.RefObject<HTMLDivElement | null>
  visualState: TreeRowVisualState
}) {
  const content = (
    <>
      <button
        type="button"
        onClick={(event) => {
          event.stopPropagation()
          props.onChevronClick?.()
        }}
        className="flex size-4 items-center justify-center rounded-md text-muted-foreground hover:text-foreground"
      >
        {props.isCollapsed ? <ChevronRightIcon className="size-4" /> : <ChevronDownIcon className="size-4" />}
      </button>
      {props.isCollapsed ? <FolderIcon className="size-4 text-muted-foreground" /> : <FolderOpenIcon className="size-4 text-muted-foreground" />}
      <span className="min-w-0 flex-1 truncate text-[13px] font-medium">{props.name}</span>
    </>
  )

  return (
    <TreeRowShell interactive={props.interactive} rowRef={props.rowRef} visualState={props.visualState}>
      {props.interactive && props.onClick
        ? (
            <div
              role="button"
              tabIndex={0}
              onClick={props.onClick}
              onKeyDown={(event) => {
                if (event.key === 'Enter' || event.key === ' ') {
                  event.preventDefault()
                  props.onClick?.()
                }
              }}
              className="flex min-w-0 flex-1 items-center gap-1.5 px-1.5 py-1.5 text-left"
            >
              {content}
            </div>
          )
        : (
            <div className="flex min-w-0 flex-1 items-center gap-1.5 px-1.5 py-1.5">
              {content}
            </div>
          )}
      <div className="relative flex w-6 items-center justify-center">
        {props.action ?? <span className="text-[11px] text-muted-foreground">{props.childCount}</span>}
      </div>
    </TreeRowShell>
  )
}

function ApiTreeRow(props: {
  action?: React.ReactNode
  className?: string
  interactive?: boolean
  isOpen: boolean
  method: string
  name: string
  onClick?: () => void
  rowRef?: ((element: HTMLDivElement | null) => void) | React.RefObject<HTMLDivElement | null>
  visualState: TreeRowVisualState
}) {
  const content = (
    <>
      <MethodBadge method={props.method} subtle />
      <span className="min-w-0 flex-1 truncate text-[13px]">{props.name}</span>
    </>
  )

  return (
    <TreeRowShell
      className={props.className}
      interactive={props.interactive}
      rowRef={props.rowRef}
      visualState={props.visualState}
    >
      {props.interactive && props.onClick
        ? (
            <div
              role="button"
              tabIndex={0}
              onClick={props.onClick}
              onKeyDown={(event) => {
                if (event.key === 'Enter' || event.key === ' ') {
                  event.preventDefault()
                  props.onClick?.()
                }
              }}
              className="flex min-w-0 flex-1 items-center gap-1.5 px-1.5 py-1.5 text-left"
            >
              {content}
            </div>
          )
        : (
            <div className="flex min-w-0 flex-1 items-center gap-1.5 px-1.5 py-1.5">
              {content}
            </div>
          )}
      <div className="relative flex w-6 items-center justify-center">
        {props.action ?? (props.isOpen ? <span className="size-1 rounded-full bg-primary" /> : null)}
      </div>
    </TreeRowShell>
  )
}

function RequestTreeNode(props: RequestTreeNodeProps) {
  const {
    activeDragItem,
    activePreview,
    collapsedCollectionIds,
    node,
    openRequestTabs,
    parentCollectionId = null,
    selectedTreeNode,
    setCollectionChildContainerElement,
    setRowElement,
    onCreateCollection,
    onCreateRequest,
    onDeleteCollection,
    onDeleteRequest,
    onEditCollection,
    onEditRequest,
    onOpenRequest,
    onToggleCollection,
    onTreeSelectionChange,
  } = props
  const requestTabId = node.entityType === 'api' ? `request:${node.id}` : null
  const isOpen = node.entityType === 'api'
    ? openRequestTabs.some(tab => tab.requestId === requestTabId)
    : false
  const isCollapsed = node.entityType === 'collection'
    ? collapsedCollectionIds.includes(node.id)
    : false
  const draggable = useDraggable({
    id: node.id,
    data: {
      childCount: node.entityType === 'collection' ? node.children.length : undefined,
      entityType: node.entityType,
      isCollapsed: node.entityType === 'collection' ? isCollapsed : undefined,
      isOpen: node.entityType === 'api' ? isOpen : undefined,
      method: node.entityType === 'api' ? node.method : undefined,
      name: node.name,
      nodeId: node.id,
      parentCollectionId,
    },
    type: treeNodeDndType,
  })
  const rowRef = React.useCallback((element: HTMLDivElement | null) => {
    draggable.ref(element)
    setRowElement(node.id, element)
  }, [draggable, node.id, setRowElement])
  const beforeDrop = useDroppable({
    id: `${node.id}:before`,
    accept: treeNodeDndType,
    data: { kind: 'before', targetNodeId: node.id },
  })
  const afterDrop = useDroppable({
    id: `${node.id}:after`,
    accept: treeNodeDndType,
    data: { kind: 'after', targetNodeId: node.id },
  })
  const insideDrop = useDroppable({
    id: `${node.id}:inside`,
    accept: treeNodeDndType,
    data: { kind: 'inside', targetNodeId: node.id },
  })

  const isDraggingSelf = activeDragItem?.nodeId === node.id
  const isPreviewNode = Boolean(activePreview) && isDraggingSelf

  if (node.entityType === 'collection') {
    const isSelected = selectedTreeNode?.type === 'collection' && selectedTreeNode.id === node.id
    const canDropIntoChildren = !isCollapsed && Boolean(activeDragItem)
    const visualState: TreeRowVisualState = isPreviewNode
      ? 'preview'
      : (isSelected || isDraggingSelf ? 'selected' : 'idle')

    return (
      <div className="relative">
        {!isDraggingSelf && <div ref={beforeDrop.ref} className="pointer-events-none absolute inset-x-0 top-0 h-1/2" />}
        {!isDraggingSelf && <div ref={afterDrop.ref} className="pointer-events-none absolute inset-x-0 bottom-0 h-1/2" />}

        <div className="relative">
          <CollectionTreeRow
            action={(
              <CollectionTreeActions
                childCount={node.children.length}
                node={node}
                onCreateCollection={onCreateCollection}
                onCreateRequest={onCreateRequest}
                onDeleteCollection={onDeleteCollection}
                onEditCollection={onEditCollection}
              />
            )}
            childCount={node.children.length}
            interactive
            isCollapsed={isCollapsed}
            name={node.name}
            onChevronClick={() => onToggleCollection(node.id)}
            onClick={() => {
              onTreeSelectionChange({ type: 'collection', id: node.id, parentCollectionId })
              onEditCollection(node)
            }}
            rowRef={rowRef}
            visualState={visualState}
          />

          {!isCollapsed && (node.children.length > 0 || canDropIntoChildren) && (
            <div
              ref={element => setCollectionChildContainerElement(node.id, element)}
              className="ml-4 border-l border-border/60 pl-1.5"
            >
              {node.children.length > 0 && (
                <TreeNodeList
                  activeDragItem={activeDragItem}
                  activePreview={activePreview}
                  collapsedCollectionIds={collapsedCollectionIds}
                  nodes={node.children}
                  openRequestTabs={openRequestTabs}
                  parentCollectionId={node.id}
                  selectedTreeNode={selectedTreeNode}
                  setCollectionChildContainerElement={setCollectionChildContainerElement}
                  setRowElement={setRowElement}
                  onCreateCollection={onCreateCollection}
                  onCreateRequest={onCreateRequest}
                  onDeleteCollection={onDeleteCollection}
                  onDeleteRequest={onDeleteRequest}
                  onEditCollection={onEditCollection}
                  onEditRequest={onEditRequest}
                  onOpenRequest={onOpenRequest}
                  onToggleCollection={onToggleCollection}
                  onTreeSelectionChange={onTreeSelectionChange}
                />
              )}
              {canDropIntoChildren && (
                <div
                  ref={insideDrop.ref}
                  className={cn(
                    'pointer-events-none',
                    node.children.length > 0 ? 'h-2' : 'mt-0.5 min-h-8',
                  )}
                />
              )}
            </div>
          )}
        </div>
      </div>
    )
  }

  const isSelected = selectedTreeNode?.type === 'api' && selectedTreeNode.id === node.id
  const visualState: TreeRowVisualState = isPreviewNode
    ? 'preview'
    : (isSelected || isDraggingSelf ? 'selected' : 'idle')

  return (
    <div className="relative">
      {!isDraggingSelf && <div ref={beforeDrop.ref} className="pointer-events-none absolute inset-x-0 top-0 h-1/2" />}
      {!isDraggingSelf && <div ref={afterDrop.ref} className="pointer-events-none absolute inset-x-0 bottom-0 h-1/2" />}

      <div className="relative">
        <ApiTreeRow
          action={(
            <ApiTreeActions
              isOpen={isOpen}
              node={node}
              onDeleteRequest={onDeleteRequest}
              onEditRequest={onEditRequest}
            />
          )}
          className="mt-0.5"
          interactive
          isOpen={isOpen}
          method={node.method}
          name={node.name}
          onClick={() => {
            onTreeSelectionChange({ type: 'api', id: node.id, parentCollectionId })
            onOpenRequest(node, parentCollectionId)
          }}
          rowRef={rowRef}
          visualState={visualState}
        />
      </div>
    </div>
  )
}

function TreeDragOverlay(props: { item: TreeDragItem }) {
  return (
    <div style={props.item.width ? { width: props.item.width } : undefined}>
      {props.item.entityType === 'collection'
        ? (
            <CollectionTreeRow
              childCount={props.item.childCount ?? 0}
              interactive={false}
              isCollapsed={props.item.isCollapsed ?? false}
              name={props.item.name}
              visualState="overlay"
            />
          )
        : (
            <ApiTreeRow
              interactive={false}
              isOpen={Boolean(props.item.isOpen)}
              method={props.item.method ?? 'GET'}
              name={props.item.name}
              visualState="overlay"
            />
          )}
    </div>
  )
}

function EnvironmentSection(props: {
  environments: Environment[]
  activeEnvironmentId: string | null
  onCreateEnvironment: () => void
  onEditEnvironment: (environment: Environment) => void
  onDeleteEnvironment: (environmentId: string) => void
  onSetActiveEnvironment: (environmentId: string | null) => void
}) {
  const [isExpanded, setIsExpanded] = React.useState(false)
  const activeEnvironment = props.environments.find(e => e.id === props.activeEnvironmentId)

  return (
    <div className="px-2.5 pb-2">
      <button
        type="button"
        onClick={() => setIsExpanded(!isExpanded)}
        className="mb-1 flex w-full items-center justify-between rounded-lg px-2 py-1 text-left text-[11px] font-medium text-muted-foreground transition hover:bg-accent hover:text-foreground"
      >
        <span className="flex items-center gap-1.5">
          <GlobeIcon className="size-3.5" />
          环境
        </span>
        <span className="flex items-center gap-1">
          {activeEnvironment && (
            <span className="rounded bg-primary/10 px-1 py-0.5 text-[9px] text-primary">
              {activeEnvironment.name}
            </span>
          )}
          {isExpanded ? <ChevronDownIcon className="size-3" /> : <ChevronRightIcon className="size-3" />}
        </span>
      </button>

      {isExpanded && (
        <div className="space-y-0.5">
          <div
            className={cn(
              'group flex cursor-pointer items-center gap-2 rounded-lg px-2 py-1.5 text-left text-[12px] transition',
              props.activeEnvironmentId === null
                ? 'bg-accent text-foreground'
                : 'text-foreground hover:bg-accent',
            )}
            onClick={() => {
              props.onSetActiveEnvironment(null)
            }}
          >
            <span className="flex size-4 items-center justify-center">
              {props.activeEnvironmentId === null
                ? (
                    <CheckIcon className="size-4 text-primary" />
                  )
                : (
                    <CircleIcon className="size-3 opacity-0 group-hover:opacity-50" />
                  )}
            </span>
            <span className="min-w-0 flex-1 truncate">无环境</span>
          </div>
          {props.environments.length === 0
            ? null
            : (
                props.environments.map(env => (
                  <div
                    key={env.id}
                    className={cn(
                      'group flex cursor-pointer items-center gap-2 rounded-lg px-2 py-1.5 text-left text-[12px] transition',
                      env.id === props.activeEnvironmentId
                        ? 'bg-accent text-foreground'
                        : 'text-foreground hover:bg-accent',
                    )}
                    onClick={() => {
                      if (env.id === props.activeEnvironmentId) {
                        props.onSetActiveEnvironment(null)
                      }
                      else {
                        props.onSetActiveEnvironment(env.id)
                      }
                    }}
                  >
                    <span className="flex size-4 items-center justify-center">
                      {env.id === props.activeEnvironmentId
                        ? (
                            <CheckIcon className="size-4 text-primary" />
                          )
                        : (
                            <CircleIcon className="size-3 opacity-0 group-hover:opacity-50" />
                          )}
                    </span>
                    <span className="min-w-0 flex-1 truncate">{env.name}</span>
                    <DropdownMenu>
                      <DropdownMenuTrigger
                        render={(
                          <Button
                            data-tree-action
                            className="size-5 opacity-0 transition group-hover:opacity-100"
                            size="icon-xs"
                            variant="ghost"
                            onClick={e => e.stopPropagation()}
                          >
                            <EllipsisIcon />
                          </Button>
                        )}
                      />
                      <DropdownMenuContent align="end" className="w-28">
                        <DropdownMenuItem
                          onClick={(e) => {
                            e.stopPropagation()
                            props.onEditEnvironment(env)
                          }}
                        >
                          <PencilLineIcon />
                          编辑
                        </DropdownMenuItem>
                        <DropdownMenuItem
                          variant="destructive"
                          onClick={(e) => {
                            e.stopPropagation()
                            props.onDeleteEnvironment(env.id)
                          }}
                        >
                          <Trash2Icon />
                          删除
                        </DropdownMenuItem>
                      </DropdownMenuContent>
                    </DropdownMenu>
                  </div>
                ))
              )}
          <button
            type="button"
            onClick={props.onCreateEnvironment}
            className="flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-left text-[12px] text-muted-foreground transition hover:bg-accent hover:text-foreground"
          >
            <PlusIcon className="size-3.5" />
            新建环境
          </button>
        </div>
      )}
    </div>
  )
}
