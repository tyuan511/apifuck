import type { OpenRequestTab, TreeSelection } from '../types'
import type { ApiSummary, CollectionTreeNode, TreeNode, WorkspaceSnapshot } from '@/lib/workspace'
import {
  CheckIcon,
  ChevronDownIcon,
  ChevronRightIcon,
  EllipsisIcon,
  FilePlusIcon,
  FolderIcon,
  FolderOpenIcon,
  FolderPlusIcon,
  PencilLineIcon,
  PlusIcon,
  Settings2Icon,
  Trash2Icon,
} from 'lucide-react'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { cn } from '@/lib/utils'
import { macOSWindowChromeHeightClassName } from '../types'
import { countApis, getProjectMonogram, startWindowDragging } from '../utils'
import { MethodBadge, SidebarEmptyState } from './shared'

interface ProjectSidebarProps {
  activeProjectId: string | null
  collapsedCollectionIds: string[]
  isMacOSDesktop: boolean
  openRequestTabs: OpenRequestTab[]
  projects: WorkspaceSnapshot['projects']
  selectedTreeNode: TreeSelection | null
  onCreateCollection: (parentCollectionId: string | null) => void
  onCreateProject: () => void
  onCreateRequest: (parentCollectionId: string | null) => void
  onDeleteCollection: (node: CollectionTreeNode) => void
  onDeleteRequest: (summary: ApiSummary) => void
  onEditRequest: (summary: ApiSummary) => void
  onOpenRequest: (summary: ApiSummary, parentCollectionId: string | null) => void
  onProjectChange: (projectId: string) => void
  onToggleCollection: (collectionId: string) => void
  onTreeSelectionChange: (selection: TreeSelection | null) => void
}

export function ProjectSidebar(props: ProjectSidebarProps) {
  const activeProject = props.projects.find(project => project.metadata.id === props.activeProjectId) ?? props.projects[0] ?? null
  const activeProjectApiCount = activeProject ? countApis(activeProject.children) : 0

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
              <span className="flex size-9 shrink-0 items-center justify-center rounded-xl bg-foreground/4 text-sm font-semibold text-foreground/80">
                {getProjectMonogram(activeProject?.metadata.name ?? activeProject?.metadata.slug ?? '项目')}
              </span>
              <span className="min-w-0 flex-1 text-left">
                <span className="block truncate text-[14px] font-semibold tracking-tight text-foreground">
                  {activeProject?.metadata.name ?? '未选择项目'}
                </span>
                <span className="block truncate text-[11px] text-muted-foreground">
                  {activeProject ? `${activeProjectApiCount} 个接口` : '点击选择项目'}
                </span>
              </span>
              <ChevronDownIcon className="size-4 shrink-0 text-muted-foreground" />
            </span>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="start" sideOffset={6} className="w-72 rounded-xl p-1.5">
            <div className="flex items-center justify-between px-1 py-1.5">
              <div className="min-w-0">
                <p className="text-[11px] font-medium uppercase tracking-[0.18em] text-muted-foreground">
                  切换项目
                </p>
                <p className="text-[11px] text-muted-foreground">
                  当前项目共
                  {' '}
                  {activeProjectApiCount}
                  {' '}
                  个接口
                </p>
              </div>
              <button
                type="button"
                onClick={props.onCreateProject}
                className="rounded-lg p-1.5 text-muted-foreground transition hover:bg-accent hover:text-foreground"
                aria-label="新建项目"
              >
                <PlusIcon className="size-4" />
              </button>
            </div>
            <DropdownMenuSeparator className="my-1.5" />
            {props.projects.map(project => (
              <DropdownMenuItem
                key={project.metadata.id}
                onClick={() => props.onProjectChange(project.metadata.id)}
                className="rounded-lg px-2 py-2"
              >
                <span className="flex min-w-0 flex-1 items-center gap-2.5">
                  <span className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-muted text-[12px] font-semibold text-foreground/80">
                    {getProjectMonogram(project.metadata.name || project.metadata.slug)}
                  </span>
                  <span className="min-w-0 flex-1 truncate text-[13px] font-medium">
                    <span className="block truncate text-[13px] font-medium">{project.metadata.name}</span>
                  </span>
                </span>
                {project.metadata.id === activeProject?.metadata.id && (
                  <CheckIcon className="size-4 text-emerald-500" />
                )}
              </DropdownMenuItem>
            ))}
          </DropdownMenuContent>
        </DropdownMenu>
      </div>

      <div className="min-h-0 flex-1 overflow-auto px-2.5 py-3">
        <div className="group mb-2 flex items-center justify-between px-0.5">
          <span className="text-[11px] font-medium uppercase tracking-[0.22em] text-muted-foreground">
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
              <DropdownMenuItem onClick={() => props.onCreateCollection(null)}>
                <FolderPlusIcon />
                新建集合
              </DropdownMenuItem>
              <DropdownMenuItem onClick={() => props.onCreateRequest(null)}>
                <FilePlusIcon />
                新建请求
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
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
                        onCreateCollection={props.onCreateCollection}
                        onCreateRequest={props.onCreateRequest}
                        onDeleteCollection={props.onDeleteCollection}
                        onDeleteRequest={props.onDeleteRequest}
                        onEditRequest={props.onEditRequest}
                        onOpenRequest={props.onOpenRequest}
                        onToggleCollection={props.onToggleCollection}
                        onTreeSelectionChange={props.onTreeSelectionChange}
                      />
                    ))
                  : <SidebarEmptyState title="还没有请求" body="新建集合或请求，开始搭建当前项目。" />}
              </div>
            )
          : <SidebarEmptyState title="还没有项目" body="先创建一个项目，再开始使用工作台。" />}
      </div>

      <div className="px-2.5 pb-2.5">
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
  onCreateCollection: (parentCollectionId: string | null) => void
  onCreateRequest: (parentCollectionId: string | null) => void
  onDeleteCollection: (node: CollectionTreeNode) => void
  onDeleteRequest: (summary: ApiSummary) => void
  onEditRequest: (summary: ApiSummary) => void
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
    onCreateCollection,
    onCreateRequest,
    onDeleteCollection,
    onDeleteRequest,
    onEditRequest,
    onOpenRequest,
    onToggleCollection,
    onTreeSelectionChange,
  } = props

  if (node.entityType === 'collection') {
    const isCollapsed = collapsedCollectionIds.includes(node.id)
    const isSelected = selectedTreeNode?.type === 'collection' && selectedTreeNode.id === node.id
    return (
      <div>
        <div
          className={cn(
            'group flex items-center gap-1 rounded-lg pr-1 transition hover:bg-accent',
            isSelected && 'bg-accent text-foreground',
          )}
        >
          <button
            type="button"
            onClick={() => {
              onTreeSelectionChange({ type: 'collection', id: node.id, parentCollectionId })
              onToggleCollection(node.id)
            }}
            className="flex min-w-0 flex-1 items-center gap-1.5 px-1.5 py-1.5 text-left"
          >
            <span className="flex size-4 items-center justify-center rounded-md text-muted-foreground hover:text-foreground">
              {isCollapsed ? <ChevronRightIcon className="size-4" /> : <ChevronDownIcon className="size-4" />}
            </span>
            {isCollapsed ? <FolderIcon className="size-4 text-muted-foreground" /> : <FolderOpenIcon className="size-4 text-muted-foreground" />}
            <span className="min-w-0 flex-1 truncate text-[13px] font-medium">{node.name}</span>
          </button>

          <div className="relative flex w-6 items-center justify-center">
            <span className="text-[11px] text-muted-foreground transition group-hover:opacity-0 group-focus-within:opacity-0">
              {node.children.length}
            </span>
            <DropdownMenu>
              <DropdownMenuTrigger
                render={(
                  <Button className="absolute opacity-0 transition group-hover:opacity-100 group-focus-within:opacity-100" size="icon-xs" variant="ghost" />
                )}
              >
                <EllipsisIcon />
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end" className="w-32">
                <DropdownMenuItem onClick={() => onCreateCollection(node.id)}>
                  <FolderPlusIcon />
                  新建目录
                </DropdownMenuItem>
                <DropdownMenuItem onClick={() => onCreateRequest(node.id)}>
                  <FilePlusIcon />
                  新建请求
                </DropdownMenuItem>
                <DropdownMenuItem variant="destructive" onClick={() => onDeleteCollection(node)}>
                  <Trash2Icon />
                  删除目录
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          </div>
        </div>
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
                onCreateCollection={onCreateCollection}
                onCreateRequest={onCreateRequest}
                onDeleteCollection={onDeleteCollection}
                onDeleteRequest={onDeleteRequest}
                onEditRequest={onEditRequest}
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
    <div
      className={cn(
        'group mt-0.5 flex items-center gap-1 rounded-lg pr-1 transition hover:bg-accent',
        isSelected && 'bg-accent text-foreground',
      )}
    >
      <button
        type="button"
        onClick={() => {
          onTreeSelectionChange({ type: 'api', id: node.id, parentCollectionId })
          onOpenRequest(node, parentCollectionId)
        }}
        className="flex min-w-0 flex-1 items-center gap-1.5 px-1.5 py-1.5 text-left"
      >
        <MethodBadge method={node.method} subtle />
        <span className="min-w-0 flex-1 truncate text-[13px]">{node.name}</span>
      </button>
      <div className="relative flex w-6 items-center justify-center">
        {isOpen && (
          <span className="size-2 rounded-full bg-primary transition group-hover:opacity-0 group-focus-within:opacity-0" />
        )}
        <DropdownMenu>
          <DropdownMenuTrigger
            render={(
              <Button
                className={cn(
                  'absolute opacity-0 transition group-hover:opacity-100 group-focus-within:opacity-100',
                  isOpen && 'group-hover:opacity-100 group-focus-within:opacity-100',
                )}
                size="icon-xs"
                variant="ghost"
              />
            )}
          >
            <EllipsisIcon />
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" className="w-32">
            <DropdownMenuItem onClick={() => onEditRequest(node)}>
              <PencilLineIcon />
              编辑
            </DropdownMenuItem>
            <DropdownMenuItem variant="destructive" onClick={() => onDeleteRequest(node)}>
              <Trash2Icon />
              删除
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
    </div>
  )
}
