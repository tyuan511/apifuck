import { Channel, invoke } from '@tauri-apps/api/core'

export interface ProjectDocsConfig {
  enabled: boolean
  basePath: string
  info: Record<string, unknown>
}

export interface ProjectMockConfig {
  enabled: boolean
  baseUrl: string
}

export interface KeyValue {
  id: string
  key: string
  value: string
  enabled: boolean
  description: string
}

export interface Environment {
  id: string
  name: string
  baseUrl: string
  variables: KeyValue[]
}

export type AuthType = 'none' | 'basic' | 'bearer' | 'api-key'

export interface AuthConfig {
  inherit: boolean
  authType: AuthType
  basic: {
    username: string
    password: string
  }
  bearerToken: string
  apiKey: {
    key: string
    value: string
    addTo: string
  }
}

export interface RequestScopeConfig {
  baseUrl: string
  headers: KeyValue[]
  auth: AuthConfig
  preRequestScript: string
  postRequestScript: string
}

export interface ProjectMetadata {
  schemaVersion: number
  entityType: 'project'
  id: string
  createdAt: string
  updatedAt: string
  slug: string
  name: string
  description: string
  rootOrder: string[]
  docs: ProjectDocsConfig
  mock: ProjectMockConfig
  requestConfig: RequestScopeConfig
  activeEnvironmentId?: string
}

export interface CollectionMetadata {
  schemaVersion: number
  entityType: 'collection'
  id: string
  createdAt: string
  updatedAt: string
  slug: string
  name: string
  description: string
  order: string[]
  requestConfig: RequestScopeConfig
}

export type BodyMode = 'none' | 'raw' | 'json' | 'form-data' | 'x-www-form-urlencoded' | 'binary'

export interface BodySpec {
  mode: BodyMode
  raw: string
  json: string
  formData: KeyValue[]
  urlEncoded: KeyValue[]
  binary: {
    filePath?: string
  }
}

export interface RequestDefinition {
  headers: KeyValue[]
  query: KeyValue[]
  pathParams: KeyValue[]
  cookies: KeyValue[]
  auth: AuthConfig
  body: BodySpec
}

export interface ApiDocumentation {
  summary: string
  description: string
  deprecated: boolean
  operationId: string
  groupName: string
}

export interface ApiMock {
  enabled: boolean
  status: number
  latencyMs: number
  headers: KeyValue[]
  body: unknown
  contentType: string
}

export interface ApiDefinition {
  schemaVersion: number
  entityType: 'api'
  id: string
  createdAt: string
  updatedAt: string
  slug: string
  name: string
  method: string
  url: string
  description: string
  tags: string[]
  request: RequestDefinition
  documentation: ApiDocumentation
  mock: ApiMock
  preRequestScript: string
  postRequestScript: string
}

export interface ApiSummary {
  entityType: 'api'
  id: string
  slug: string
  name: string
  method: string
  url: string
  description: string
  tags: string[]
  createdAt: string
  updatedAt: string
}

export interface CollectionTreeNode {
  entityType: 'collection'
  id: string
  slug: string
  name: string
  description: string
  createdAt: string
  updatedAt: string
  requestConfig: RequestScopeConfig
  children: TreeNode[]
}

export type TreeNode = CollectionTreeNode | ApiSummary

export interface ProjectSnapshot {
  metadata: ProjectMetadata
  children: TreeNode[]
  environments: Environment[]
}

export interface BootstrapProjectInput {
  path: string
  project?: CreateProjectInput
}

export interface CreateProjectInput {
  name: string
  description: string
  slug?: string
}

export interface UpdateProjectInput {
  id: string
  name: string
  description: string
  docs: ProjectDocsConfig
  mock: ProjectMockConfig
  requestConfig: RequestScopeConfig
}

export interface CreateCollectionInput {
  projectId: string
  parentCollectionId?: string
  name: string
  description: string
  slug?: string
}

export interface UpdateCollectionInput {
  id: string
  name: string
  description: string
  requestConfig: RequestScopeConfig
}

export interface CreateApiInput {
  projectId: string
  parentCollectionId?: string
  name: string
  method: string
  url: string
  description: string
  tags: string[]
  request: RequestDefinition
  documentation: ApiDocumentation
  mock: ApiMock
  preRequestScript: string
  postRequestScript: string
  slug?: string
}

export interface UpdateApiInput {
  id: string
  name: string
  method: string
  url: string
  description: string
  tags: string[]
  request: RequestDefinition
  documentation: ApiDocumentation
  mock: ApiMock
  preRequestScript: string
  postRequestScript: string
}

export interface MoveNodeInput {
  nodeId: string
  targetProjectId: string
  targetCollectionId?: string
  position?: number
}

export interface ReorderChildrenInput {
  projectId?: string
  parentCollectionId?: string
  orderedIds: string[]
}

export type ResponseType = 'json' | 'text' | 'eventstream'

export interface SendRequestInput {
  requestId?: string
  method: string
  url: string
  request: RequestDefinition
}

export interface ResponseHeader {
  name: string
  value: string
}

export interface SendRequestResponse {
  status: number
  headers: ResponseHeader[]
  durationMs: number
  sizeBytes: number
  contentType: string
  responseType: ResponseType
  body: string
}

export interface SendRequestStreamStartedEvent {
  kind: 'started'
  requestId: string
  status: number
  headers: ResponseHeader[]
  contentType: string
}

export interface SendRequestStreamChunkEvent {
  kind: 'chunk'
  requestId: string
  chunk: string
  receivedBytes: number
}

export interface SendRequestStreamFinishedEvent {
  kind: 'finished'
  requestId: string
  durationMs: number
  sizeBytes: number
  responseType: ResponseType
  body: string
}

export type SendRequestStreamEvent
  = | SendRequestStreamStartedEvent
    | SendRequestStreamChunkEvent
    | SendRequestStreamFinishedEvent

interface SendRequestStreamEventWire {
  kind?: string
  requestId?: string
  request_id?: string
  status?: number
  headers?: ResponseHeader[]
  contentType?: string
  content_type?: string
  chunk?: string
  receivedBytes?: number
  received_bytes?: number
  durationMs?: number
  duration_ms?: number
  sizeBytes?: number
  size_bytes?: number
  responseType?: ResponseType
  response_type?: ResponseType
  body?: string
}

export function createDefaultRequest(): RequestDefinition {
  return {
    headers: [],
    query: [],
    pathParams: [],
    cookies: [],
    auth: {
      inherit: true,
      authType: 'none',
      basic: { username: '', password: '' },
      bearerToken: '',
      apiKey: { key: '', value: '', addTo: 'header' },
    },
    body: {
      mode: 'json',
      raw: '',
      json: '',
      formData: [],
      urlEncoded: [],
      binary: {},
    },
  }
}

export function createDefaultRequestScopeConfig(): RequestScopeConfig {
  return {
    baseUrl: '',
    headers: [],
    auth: {
      inherit: true,
      authType: 'none',
      basic: { username: '', password: '' },
      bearerToken: '',
      apiKey: { key: '', value: '', addTo: 'header' },
    },
    preRequestScript: '',
    postRequestScript: '',
  }
}

export function createDefaultDocumentation(groupName = ''): ApiDocumentation {
  return {
    summary: '',
    description: '',
    deprecated: false,
    operationId: '',
    groupName,
  }
}

export function createDefaultMock(): ApiMock {
  return {
    enabled: false,
    status: 200,
    latencyMs: 0,
    headers: [],
    body: null,
    contentType: 'application/json',
  }
}

export function bootstrapProject(path: string, project?: CreateProjectInput) {
  return invoke<ProjectSnapshot>('bootstrap_project', { path, input: project ?? null })
}

export function openProject(path: string) {
  return invoke<ProjectSnapshot>('open_project', { path })
}

export function readProjectSummary(path: string) {
  return invoke<ProjectMetadata>('read_project_summary', { path })
}

export function deleteProjectFiles(path: string) {
  return invoke<void>('delete_project_files', { path })
}

export function updateProject(input: UpdateProjectInput) {
  return invoke<ProjectMetadata>('update_project', { input })
}

export function createCollection(input: CreateCollectionInput) {
  return invoke<CollectionMetadata>('create_collection', { input })
}

export function updateCollection(input: UpdateCollectionInput) {
  return invoke<CollectionMetadata>('update_collection', { input })
}

export function createApi(input: CreateApiInput) {
  return invoke<ApiDefinition>('create_api', { input })
}

export function updateApi(input: UpdateApiInput) {
  return invoke<ApiDefinition>('update_api', { input })
}

export function deleteNode(id: string) {
  return invoke<void>('delete_node', { input: { id } })
}

export function moveNode(input: MoveNodeInput) {
  return invoke<void>('move_node', { input })
}

export function reorderChildren(input: ReorderChildrenInput) {
  return invoke<void>('reorder_children', { input })
}

export function readApi(id: string) {
  return invoke<ApiDefinition>('read_api', { id })
}

export function sendRequest(
  input: SendRequestInput,
  onStreamEvent?: (event: SendRequestStreamEvent) => void,
) {
  const onEvent = new Channel<SendRequestStreamEvent>()
  if (onStreamEvent) {
    onEvent.onmessage = (event) => {
      onStreamEvent(normalizeSendRequestStreamEvent(event as SendRequestStreamEventWire))
    }
  }

  return invoke<SendRequestResponse>('send_request', { input, onEvent })
}

function normalizeSendRequestStreamEvent(event: SendRequestStreamEventWire): SendRequestStreamEvent {
  const kind = event.kind
  const requestId = event.requestId ?? event.request_id ?? ''

  if (kind === 'started') {
    return {
      kind,
      requestId,
      status: event.status ?? 0,
      headers: event.headers ?? [],
      contentType: event.contentType ?? event.content_type ?? '',
    }
  }

  if (kind === 'chunk') {
    return {
      kind,
      requestId,
      chunk: event.chunk ?? '',
      receivedBytes: event.receivedBytes ?? event.received_bytes ?? 0,
    }
  }

  return {
    kind: 'finished',
    requestId,
    durationMs: event.durationMs ?? event.duration_ms ?? 0,
    sizeBytes: event.sizeBytes ?? event.size_bytes ?? 0,
    responseType: event.responseType ?? event.response_type ?? 'text',
    body: event.body ?? '',
  }
}

export interface CreateEnvironmentInput {
  projectId: string
  name: string
  baseUrl?: string
  variables?: KeyValue[]
}

export interface UpdateEnvironmentInput {
  id: string
  projectId: string
  name: string
  baseUrl?: string
  variables?: KeyValue[]
}

export interface DeleteEnvironmentInput {
  id: string
  projectId: string
}

export interface SetActiveEnvironmentInput {
  projectId: string
  environmentId: string | null
}

export function createEnvironment(input: CreateEnvironmentInput) {
  return invoke<Environment>('create_environment', { input })
}

export function updateEnvironment(input: UpdateEnvironmentInput) {
  return invoke<Environment>('update_environment', { input })
}

export function deleteEnvironment(input: DeleteEnvironmentInput) {
  return invoke<void>('delete_environment', { input })
}

export function listEnvironments(projectId: string) {
  return invoke<Environment[]>('list_environments', { projectId })
}

export function setActiveEnvironment(input: SetActiveEnvironmentInput) {
  return invoke<ProjectMetadata>('set_active_environment', { input })
}
