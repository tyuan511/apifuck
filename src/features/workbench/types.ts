import type {
  ApiDefinition,
  CollectionTreeNode,
  ResponseHeader,
  ResponseType,
  WorkspaceSnapshot,
} from '@/lib/workspace'

export type EditorPanelTab = 'query' | 'headers' | 'body' | 'preRequestScript' | 'postRequestScript'

export type TreeSelection
  = | { type: 'collection', id: string, parentCollectionId: string | null }
    | { type: 'api', id: string, parentCollectionId: string | null }

export interface RequestEditorDraft extends ApiDefinition {}

export interface OpenRequestTab {
  requestId: string
  title: string
  method: string
  dirty: boolean
  lastFocusedAt: number
}

export interface PendingCollectionDeletion {
  id: string
  name: string
  apiIds: string[]
  collectionIds: string[]
}

export interface PendingRequestDeletion {
  id: string
  name: string
}

export interface ResponseState {
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

export interface WorkbenchBootPayload {
  workspacePath: string
  workspaceSnapshot: WorkspaceSnapshot
}

export const methodOptions = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS']

export const editorTabs: Array<{ value: EditorPanelTab, label: string }> = [
  { value: 'query', label: '查询参数' },
  { value: 'headers', label: '请求头' },
  { value: 'body', label: '请求体' },
  { value: 'preRequestScript', label: '请求脚本' },
  { value: 'postRequestScript', label: '响应脚本' },
]

export const projectMonogramSeparatorPattern = /[-_\s]+/g
export const isMacOSDesktop = typeof navigator !== 'undefined' && /mac/i.test(navigator.userAgent)
export const macOSWindowChromeHeightClassName = 'h-[52px]'
export const canUseTauriWindowDrag = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

export type CollectionSubtree = Pick<PendingCollectionDeletion, 'apiIds' | 'collectionIds'>

export type DeleteCollectionRequest = PendingCollectionDeletion | null

export type ActiveProjectLike = WorkspaceSnapshot['projects'][number] | null

export type CollectionNodeLike = CollectionTreeNode
