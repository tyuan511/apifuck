import type { MouseEvent as ReactMouseEvent } from 'react'
import type { CollectionSubtree, ResponseState } from './types'
import type {
  ApiDefinition,
  ApiSummary,
  CollectionTreeNode,
  KeyValue,
  SendRequestResponse,
  TreeNode,
  WorkspaceSnapshot,
} from '@/lib/workspace'
import { getCurrentWindow } from '@tauri-apps/api/window'
import {
  canUseTauriWindowDrag,
  isMacOSDesktop,
  projectMonogramSeparatorPattern,
} from './types'

// Static regex patterns for URL parsing (avoid re-compilation on each call)
const protocolRegex = /^https?:\/\//i
const hostnameRegex = /^[^/]+\.[^/]/

export function getActiveProject(workspace: WorkspaceSnapshot | null, activeProjectId: string | null) {
  if (!workspace) {
    return null
  }

  return workspace.projects.find(project => project.metadata.id === activeProjectId)
    ?? workspace.projects.find(project => project.metadata.id === workspace.lastOpenedProjectId)
    ?? workspace.projects[0]
    ?? null
}

export function collectCollectionIds(nodes: TreeNode[]): string[] {
  return nodes.flatMap((node) => {
    if (node.entityType !== 'collection') {
      return []
    }
    return [node.id, ...collectCollectionIds(node.children)]
  })
}

export function countApis(nodes: TreeNode[]): number {
  return nodes.reduce((total, node) => {
    if (node.entityType === 'api') {
      return total + 1
    }
    return total + countApis(node.children)
  }, 0)
}

export function collectCollectionSubtree(node: CollectionTreeNode): CollectionSubtree {
  return node.children.reduce((accumulator, child) => {
    if (child.entityType === 'api') {
      accumulator.apiIds.push(child.id)
      return accumulator
    }

    const nested = collectCollectionSubtree(child)
    accumulator.collectionIds.push(...nested.collectionIds)
    accumulator.apiIds.push(...nested.apiIds)
    return accumulator
  }, {
    apiIds: [] as string[],
    collectionIds: [node.id],
  })
}

export function findCollectionName(nodes: TreeNode[], collectionId: string): string {
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

export function describeCollectionParent(parentCollectionId: string | null, collectionNames: Map<string, string>) {
  if (!parentCollectionId) {
    return '项目根目录'
  }
  return collectionNames.get(parentCollectionId) ?? '已选集合'
}

export function cloneApiDefinition(api: ApiDefinition): ApiDefinition {
  return JSON.parse(JSON.stringify(api)) as ApiDefinition
}

export function areApiDefinitionsEqual(left: ApiDefinition, right: ApiDefinition) {
  return JSON.stringify(left) === JSON.stringify(right)
}

export function createKeyValueDraft(): KeyValue {
  return {
    id: globalThis.crypto?.randomUUID?.() ?? `kv-${Date.now()}-${Math.random().toString(16).slice(2)}`,
    key: '',
    value: '',
    enabled: true,
    description: '',
  }
}

/**
 * Parse query parameters from a URL string into KeyValue array.
 * Handles absolute URLs, relative paths like /users?id=1, and hostnames like api.example.com.
 * Preserves existing row IDs if the same key exists.
 */
export function parseUrlQueryParams(url: string, existingParams: KeyValue[]): KeyValue[] {
  if (!url) {
    return []
  }

  try {
    // Handle relative paths like /users?id=1 or just ?id=1
    if (!protocolRegex.test(url) && !hostnameRegex.test(url)) {
      // This is a relative URL or just a query string
      const queryStart = url.indexOf('?')
      if (queryStart === -1) {
        return []
      }
      const queryString = url.slice(queryStart + 1)
      const searchParams = new URLSearchParams(queryString)
      return parseSearchParams(searchParams, existingParams)
    }

    // Handle URLs without protocol by prepending https://
    let urlToParse = url
    if (!protocolRegex.test(urlToParse) && hostnameRegex.test(urlToParse)) {
      urlToParse = `https://${urlToParse}`
    }

    const urlObj = new URL(urlToParse)
    return parseSearchParams(urlObj.searchParams, existingParams)
  }
  catch {
    return []
  }
}

/**
 * Parse search params to KeyValue array, preserving existing IDs.
 */
function parseSearchParams(searchParams: URLSearchParams, existingParams: KeyValue[]): KeyValue[] {
  const result: KeyValue[] = []

  // Build a map of existing keys for ID preservation
  const existingKeyToId = new Map<string, string>()
  for (const param of existingParams) {
    if (param.key) {
      existingKeyToId.set(param.key, param.id)
    }
  }

  searchParams.forEach((value, key) => {
    result.push({
      id: existingKeyToId.get(key) ?? globalThis.crypto?.randomUUID?.() ?? `kv-${Date.now()}-${Math.random().toString(16).slice(2)}`,
      key,
      value: decodeURIComponent(value),
      enabled: true,
      description: '',
    })
  })

  return result
}

/**
 * Build a URL string with query parameters appended.
 * Handles absolute URLs, relative paths like /users?id=1, and hostnames like api.example.com.
 * Preserves the base URL (path + domain) and replaces existing query params.
 * Only includes enabled params.
 */
export function buildUrlWithQuery(url: string, params: KeyValue[]): string {
  if (!url) {
    return url
  }

  try {
    // Handle relative paths like /users?id=1
    if (!protocolRegex.test(url) && !hostnameRegex.test(url)) {
      const queryStart = url.indexOf('?')
      const base = queryStart >= 0 ? url.slice(0, queryStart) : url
      // Preserve hash if present
      const hashStart = base.indexOf('#')
      const baseWithoutHash = hashStart >= 0 ? base.slice(0, hashStart) : base
      const hash = hashStart >= 0 ? base.slice(hashStart) : ''

      const searchParams = new URLSearchParams()
      for (const param of params) {
        if (param.enabled && param.key.trim()) {
          searchParams.append(param.key.trim(), param.value)
        }
      }

      const queryString = searchParams.toString()
      return queryString ? `${baseWithoutHash}?${queryString}${hash}` : `${baseWithoutHash}${hash}`
    }

    // Handle URLs without protocol
    let urlToParse = url
    if (!protocolRegex.test(urlToParse) && hostnameRegex.test(urlToParse)) {
      urlToParse = `https://${urlToParse}`
    }

    const urlObj = new URL(urlToParse)
    const searchParams = urlObj.searchParams

    // Clear existing query params
    searchParams.delete('dummy') // Workaround to clear all params

    // Append enabled params
    for (const param of params) {
      if (param.enabled && param.key.trim()) {
        searchParams.append(param.key.trim(), param.value)
      }
    }

    // Return full URL including hash if present
    return urlObj.toString()
  }
  catch {
    return url
  }
}

/**
 * Extract the base URL (without query parameters) from a URL string.
 */
export function getBaseUrl(url: string): string {
  try {
    let urlToParse = url
    if (urlToParse && !protocolRegex.test(urlToParse)) {
      if (hostnameRegex.test(urlToParse)) {
        urlToParse = `https://${urlToParse}`
      }
      else {
        return url
      }
    }

    const urlObj = new URL(urlToParse)
    // Remove query string and hash
    return `${urlObj.origin}${urlObj.pathname}${urlObj.hash}`
  }
  catch {
    return url
  }
}

export function createEmptyResponseState(): ResponseState {
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

export function mapSendResponseToState(response: SendRequestResponse): ResponseState {
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

export function clamp(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value))
}

export function formatBytes(sizeBytes: number) {
  if (sizeBytes < 1024) {
    return `${sizeBytes} B`
  }
  if (sizeBytes < 1024 * 1024) {
    return `${(sizeBytes / 1024).toFixed(1)} KB`
  }
  return `${(sizeBytes / (1024 * 1024)).toFixed(2)} MB`
}

export function startWindowDragging(event: ReactMouseEvent<HTMLElement>) {
  if (!isMacOSDesktop || !canUseTauriWindowDrag || event.button !== 0) {
    return
  }

  const target = event.target
  if (target instanceof HTMLElement && target.closest('[data-no-window-drag]')) {
    return
  }

  void getCurrentWindow().startDragging().catch(() => {})
}

export function getProjectMonogram(value: string) {
  const normalized = value.trim()
  if (!normalized) {
    return '项'
  }

  const compact = normalized.replace(projectMonogramSeparatorPattern, '')
  return compact.slice(0, 2).toUpperCase()
}

export function getProjectDisplayName(project: WorkspaceSnapshot['projects'][number] | null | undefined) {
  if (!project) {
    return '未选择项目'
  }

  const normalizedName = project.metadata.name.trim().toLowerCase()
  const normalizedSlug = project.metadata.slug.trim().toLowerCase()

  if (normalizedSlug === 'default' && ['default', '默认项目'].includes(normalizedName)) {
    return '默认项目'
  }

  return project.metadata.name
}

export function getMethodBadgeTone(method: string) {
  switch (method.toUpperCase()) {
    case 'GET':
      return {
        subtle: 'bg-emerald-500/12 text-emerald-700 dark:text-emerald-300',
        solid: 'bg-emerald-600 text-white',
      }
    case 'POST':
      return {
        subtle: 'bg-sky-500/12 text-sky-700 dark:text-sky-300',
        solid: 'bg-sky-600 text-white',
      }
    case 'PUT':
      return {
        subtle: 'bg-amber-500/12 text-amber-700 dark:text-amber-300',
        solid: 'bg-amber-500 text-amber-950',
      }
    case 'PATCH':
      return {
        subtle: 'bg-cyan-500/12 text-cyan-700 dark:text-cyan-300',
        solid: 'bg-cyan-600 text-white',
      }
    case 'DELETE':
      return {
        subtle: 'bg-rose-500/12 text-rose-700 dark:text-rose-300',
        solid: 'bg-rose-600 text-white',
      }
    case 'HEAD':
      return {
        subtle: 'bg-slate-500/12 text-slate-700 dark:text-slate-300',
        solid: 'bg-slate-600 text-white',
      }
    case 'OPTIONS':
      return {
        subtle: 'bg-indigo-500/12 text-indigo-700 dark:text-indigo-300',
        solid: 'bg-indigo-600 text-white',
      }
    default:
      return {
        subtle: 'bg-primary/10 text-primary',
        solid: 'bg-primary text-primary-foreground',
      }
  }
}

export function hasVisibleResponse(response: ResponseState) {
  return response.isLoading
    || Boolean(response.error)
    || response.status !== null
    || response.headers.length > 0
    || response.durationMs > 0
    || response.sizeBytes > 0
    || Boolean(response.contentType)
    || response.responseType !== null
    || Boolean(response.body)
}

export function findApiLocation(
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

export function findApiLocationInProject(
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
