import type { ApiDefinition, Environment, KeyValue, ProjectSnapshot, RequestDefinition } from '@/lib/project'
import { createDefaultRequestScopeConfig } from '@/lib/project'
import {
  findApiLocation,
  findCollectionPath,
  mergeKeyValueEntries,
  normalizeRequestUrl,
  resolveInheritedAuth,
} from './utils'

export type RequestExportLanguage = 'curl' | 'javascript' | 'python' | 'go'

export interface RequestExportSnippet {
  code: string
  language: 'shellscript' | 'javascript' | 'python' | 'go'
  title: string
}

interface ResolvedRequestHeader {
  key: string
  value: string
}

interface ResolvedRequestBody {
  binaryFilePath?: string
  formData: Array<
    | { type: 'text', key: string, value: string }
    | { type: 'file', key: string, filePath: string, fileName?: string, contentType?: string }
  >
  json: string
  mode: RequestDefinition['body']['mode']
  raw: string
  urlEncoded: Array<{ key: string, value: string }>
}

interface ResolvedRequestExport {
  body: ResolvedRequestBody
  headers: ResolvedRequestHeader[]
  method: string
  url: string
}

export function buildRequestExportSnippets(args: {
  activeEnvironmentId: string | null
  environments: Environment[]
  project: ProjectSnapshot | null
  request: ApiDefinition
}): Record<RequestExportLanguage, RequestExportSnippet> {
  const resolved = resolveRequestForExport(args)

  return {
    curl: {
      code: buildCurlSnippet(resolved),
      language: 'shellscript',
      title: 'cURL',
    },
    javascript: {
      code: buildJavaScriptSnippet(resolved),
      language: 'javascript',
      title: 'JavaScript',
    },
    python: {
      code: buildPythonSnippet(resolved),
      language: 'python',
      title: 'Python',
    },
    go: {
      code: buildGoSnippet(resolved),
      language: 'go',
      title: 'Go',
    },
  }
}

function resolveRequestForExport(args: {
  activeEnvironmentId: string | null
  environments: Environment[]
  project: ProjectSnapshot | null
  request: ApiDefinition
}): ResolvedRequestExport {
  const { activeEnvironmentId, environments, project, request } = args
  const activeEnvironment = activeEnvironmentId
    ? environments.find(environment => environment.id === activeEnvironmentId) ?? null
    : null
  const variables = activeEnvironment?.variables.filter(variable => variable.enabled) ?? []
  const requestLocation = project ? findApiLocation(project, request.id) : null
  const collectionConfigs = project && requestLocation?.parentCollectionId
    ? findCollectionPath(project.children, requestLocation.parentCollectionId)
        .map(collection => collection.requestConfig ?? createDefaultRequestScopeConfig())
    : []
  const projectConfig = project?.metadata.requestConfig ?? createDefaultRequestScopeConfig()
  const mergedHeaders = mergeKeyValueEntries(
    projectConfig.headers,
    ...collectionConfigs.map(config => config.headers),
    request.request.headers,
  )
  const mergedAuth = resolveInheritedAuth(
    projectConfig.auth,
    ...collectionConfigs.map(config => config.auth),
    request.request.auth,
  )
  const scopeBaseUrl = [
    projectConfig.baseUrl,
    ...collectionConfigs.map(config => config.baseUrl),
  ].find(baseUrl => baseUrl.trim()) || activeEnvironment?.baseUrl || ''

  let finalUrl = resolveUrl(request.url, scopeBaseUrl)
  finalUrl = applyPathParams(finalUrl, request.request.pathParams, variables)
  finalUrl = appendQueryParams(finalUrl, request.request.query, variables)

  const resolvedHeaders = mergedHeaders
    .filter(header => header.enabled && header.key.trim())
    .map(header => ({
      key: resolveEnvironmentVariables(header.key.trim(), variables),
      value: resolveEnvironmentVariables(header.value, variables),
    }))

  const resolvedCookies = request.request.cookies
    .filter(cookie => cookie.enabled && cookie.key.trim())
    .map(cookie => ({
      key: resolveEnvironmentVariables(cookie.key.trim(), variables),
      value: resolveEnvironmentVariables(cookie.value, variables),
    }))
  const resolvedBody: ResolvedRequestBody = {
    binaryFilePath: request.request.body.binary.filePath,
    formData: request.request.body.formData
      .filter(item => item.enabled && item.key.trim())
      .map((item) => {
        const key = resolveEnvironmentVariables(item.key.trim(), variables)
        if ((item.entryType ?? 'text') === 'file' && item.filePath) {
          return {
            type: 'file' as const,
            key,
            filePath: resolveEnvironmentVariables(item.filePath, variables),
            fileName: item.fileName,
            contentType: item.contentType,
          }
        }
        return {
          type: 'text' as const,
          key,
          value: resolveEnvironmentVariables(item.value, variables),
        }
      }),
    json: resolveEnvironmentVariables(request.request.body.json, variables),
    mode: request.request.body.mode,
    raw: resolveEnvironmentVariables(request.request.body.raw, variables),
    urlEncoded: request.request.body.urlEncoded
      .filter(item => item.enabled && item.key.trim())
      .map(item => ({
        key: resolveEnvironmentVariables(item.key.trim(), variables),
        value: resolveEnvironmentVariables(item.value, variables),
      })),
  }

  const headerMap = new Map<string, ResolvedRequestHeader>()
  resolvedHeaders.forEach((header) => {
    const normalizedKey = header.key.trim().toLowerCase()
    if (!normalizedKey) {
      return
    }
    headerMap.set(normalizedKey, header)
  })

  if (resolvedCookies.length > 0) {
    const cookieHeaderValue = resolvedCookies
      .map(cookie => `${cookie.key}=${cookie.value}`)
      .join('; ')
    headerMap.set('cookie', {
      key: 'Cookie',
      value: cookieHeaderValue,
    })
  }

  const resolvedAuth = {
    ...mergedAuth,
    basic: {
      username: resolveEnvironmentVariables(mergedAuth.basic.username, variables),
      password: resolveEnvironmentVariables(mergedAuth.basic.password, variables),
    },
    bearerToken: resolveEnvironmentVariables(mergedAuth.bearerToken, variables),
    apiKey: {
      ...mergedAuth.apiKey,
      key: resolveEnvironmentVariables(mergedAuth.apiKey.key, variables),
      value: resolveEnvironmentVariables(mergedAuth.apiKey.value, variables),
    },
  }

  if (resolvedAuth.authType === 'basic') {
    const credentials = `${resolvedAuth.basic.username}:${resolvedAuth.basic.password}`
    headerMap.set('authorization', {
      key: 'Authorization',
      value: `Basic ${encodeBase64(credentials)}`,
    })
  }
  else if (resolvedAuth.authType === 'bearer' && resolvedAuth.bearerToken.trim()) {
    headerMap.set('authorization', {
      key: 'Authorization',
      value: `Bearer ${resolvedAuth.bearerToken}`,
    })
  }
  else if (resolvedAuth.authType === 'api-key' && resolvedAuth.apiKey.key.trim()) {
    if (resolvedAuth.apiKey.addTo.trim().toLowerCase() === 'query') {
      finalUrl = appendSingleQueryParam(finalUrl, resolvedAuth.apiKey.key.trim(), resolvedAuth.apiKey.value)
    }
    else {
      headerMap.set(resolvedAuth.apiKey.key.trim().toLowerCase(), {
        key: resolvedAuth.apiKey.key.trim(),
        value: resolvedAuth.apiKey.value,
      })
    }
  }

  if (resolvedBody.mode === 'json' && resolvedBody.json.trim() && !headerMap.has('content-type')) {
    headerMap.set('content-type', {
      key: 'Content-Type',
      value: 'application/json',
    })
  }

  if (resolvedBody.mode === 'x-www-form-urlencoded' && resolvedBody.urlEncoded.length > 0 && !headerMap.has('content-type')) {
    headerMap.set('content-type', {
      key: 'Content-Type',
      value: 'application/x-www-form-urlencoded',
    })
  }

  return {
    body: resolvedBody,
    headers: [...headerMap.values()],
    method: request.method.toUpperCase(),
    url: finalUrl,
  }
}

function resolveEnvironmentVariables(text: string, variables: KeyValue[]) {
  let result = text
  for (const variable of variables) {
    const pattern = `\${${variable.key}}`
    result = result.split(pattern).join(variable.value)
  }
  return result
}

function resolveUrl(url: string, baseUrl: string) {
  const normalizedUrl = normalizeRequestUrl(url)
  if (!baseUrl) {
    return normalizedUrl
  }

  if (/^https?:\/\//i.test(normalizedUrl)) {
    return normalizedUrl
  }

  const normalizedBaseUrl = baseUrl.endsWith('/') ? baseUrl.slice(0, -1) : baseUrl
  const normalizedPath = normalizedUrl.startsWith('/') ? normalizedUrl : `/${normalizedUrl}`
  return `${normalizedBaseUrl}${normalizedPath}`
}

function applyPathParams(url: string, params: KeyValue[], variables: KeyValue[]) {
  return params
    .filter(param => param.enabled && param.key.trim())
    .reduce((currentUrl, param) => {
      const key = resolveEnvironmentVariables(param.key.trim(), variables)
      const value = encodeURIComponent(resolveEnvironmentVariables(param.value, variables))
      return currentUrl
        .replaceAll(`{${key}}`, value)
        .replaceAll(`:${key}`, value)
    }, url)
}

function appendQueryParams(url: string, params: KeyValue[], variables: KeyValue[]) {
  return params
    .filter(param => param.enabled && param.key.trim())
    .reduce((currentUrl, param) => {
      const key = resolveEnvironmentVariables(param.key.trim(), variables)
      const value = resolveEnvironmentVariables(param.value, variables)
      return appendSingleQueryParam(currentUrl, key, value)
    }, url)
}

function appendSingleQueryParam(url: string, key: string, value: string) {
  try {
    const parsedUrl = new URL(url)
    parsedUrl.searchParams.append(key, value)
    return parsedUrl.toString()
  }
  catch {
    const separator = url.includes('?') ? '&' : '?'
    return `${url}${separator}${encodeURIComponent(key)}=${encodeURIComponent(value)}`
  }
}

function encodeBase64(value: string) {
  return btoa(unescape(encodeURIComponent(value)))
}

function buildCurlSnippet(request: ResolvedRequestExport) {
  const lines = [
    `curl --request ${request.method}`,
    `  --url '${escapeShellSingleQuoted(request.url)}'`,
  ]

  request.headers.forEach((header) => {
    lines.push(`  --header '${escapeShellSingleQuoted(`${header.key}: ${header.value}`)}'`)
  })

  if (request.body.mode === 'raw' && request.body.raw) {
    lines.push(`  --data-raw '${escapeShellSingleQuoted(request.body.raw)}'`)
  }
  else if (request.body.mode === 'json' && request.body.json) {
    lines.push(`  --data-raw '${escapeShellSingleQuoted(request.body.json)}'`)
  }
  else if (request.body.mode === 'form-data') {
    request.body.formData.forEach((item) => {
      if (item.type === 'file') {
        lines.push(`  --form '${escapeShellSingleQuoted(`${item.key}=@${item.filePath}`)}'`)
      }
      else {
        lines.push(`  --form '${escapeShellSingleQuoted(`${item.key}=${item.value}`)}'`)
      }
    })
  }
  else if (request.body.mode === 'x-www-form-urlencoded') {
    request.body.urlEncoded.forEach((item) => {
      lines.push(`  --data-urlencode '${escapeShellSingleQuoted(`${item.key}=${item.value}`)}'`)
    })
  }
  else if (request.body.mode === 'binary') {
    lines.push(`  --data-binary '@${escapeShellSingleQuoted(request.body.binaryFilePath || 'path/to/file')}'`)
  }

  return lines.join(' \\\n')
}

function buildJavaScriptSnippet(request: ResolvedRequestExport) {
  const lines: string[] = [
    'const headers = {',
    ...request.headers.map(header => `  ${JSON.stringify(header.key)}: ${JSON.stringify(header.value)},`),
    '}',
    '',
  ]

  const bodyVariable = buildJavaScriptBody(request)
  if (bodyVariable) {
    lines.push(...bodyVariable, '')
  }

  lines.push(
    'const response = await fetch(',
    `  ${JSON.stringify(request.url)},`,
    '  {',
    `    method: ${JSON.stringify(request.method)},`,
    '    headers,',
    ...(bodyVariable ? ['    body,'] : []),
    '  },',
    ')',
    '',
    'console.log(response.status)',
  )

  return lines.join('\n')
}

function buildJavaScriptBody(request: ResolvedRequestExport) {
  if (request.body.mode === 'raw') {
    return [`const body = ${JSON.stringify(request.body.raw)}`]
  }
  if (request.body.mode === 'json') {
    return [`const data = ${request.body.json}`, 'const body = JSON.stringify(data)']
  }
  if (request.body.mode === 'form-data') {
    return [
      'const body = new FormData()',
      'const { readFile } = await import(\'node:fs/promises\')',
      ...request.body.formData.flatMap((item, index) => item.type === 'file'
        ? [
            `const file${index} = await readFile(${JSON.stringify(item.filePath)})`,
            `body.append(${JSON.stringify(item.key)}, new Blob([file${index}], ${item.contentType ? `{ type: ${JSON.stringify(item.contentType)} }` : 'undefined'}), ${JSON.stringify(item.fileName || item.filePath.split(/[\\/]/).pop() || 'upload.bin')})`,
          ]
        : [`body.append(${JSON.stringify(item.key)}, ${JSON.stringify(item.value)})`]),
    ]
  }
  if (request.body.mode === 'x-www-form-urlencoded') {
    return [
      'const body = new URLSearchParams()',
      ...request.body.urlEncoded.map(item => `body.append(${JSON.stringify(item.key)}, ${JSON.stringify(item.value)})`),
    ]
  }
  if (request.body.mode === 'binary') {
    return [
      'const { readFile } = await import(\'node:fs/promises\')',
      `const body = await readFile(${JSON.stringify(request.body.binaryFilePath || 'path/to/file')})`,
    ]
  }
  return null
}

function buildPythonSnippet(request: ResolvedRequestExport) {
  const lines: string[] = [
    'import requests',
    '',
    `url = ${toPythonString(request.url)}`,
    `headers = ${toPythonDict(request.headers)}`,
  ]

  if (request.body.mode === 'raw') {
    lines.push(`data = ${toPythonString(request.body.raw)}`)
  }
  else if (request.body.mode === 'json') {
    lines.push(`data = ${toPythonDictFromJson(request.body.json)}`)
  }
  else if (request.body.mode === 'form-data') {
    const textItems = request.body.formData.filter((item): item is Extract<typeof item, { type: 'text' }> => item.type === 'text')
    const fileItems = request.body.formData.filter((item): item is Extract<typeof item, { type: 'file' }> => item.type === 'file')
    if (textItems.length > 0) {
      lines.push(`data = ${toPythonDict(textItems.map(item => ({ key: item.key, value: item.value })))}`)
    }
    if (fileItems.length > 0) {
      lines.push(`files = ${toPythonFileDict(fileItems)}`)
    }
  }
  else if (request.body.mode === 'x-www-form-urlencoded') {
    lines.push(`data = ${toPythonDict(request.body.urlEncoded)}`)
  }
  else if (request.body.mode === 'binary') {
    lines.push(`with open(${toPythonString(request.body.binaryFilePath || 'path/to/file')}, 'rb') as file:`, `    data = file.read()`)
  }

  if (request.body.mode === 'binary') {
    lines.push(
      `    response = requests.request(${toPythonString(request.method)}, url, headers=headers, data=data)`,
      '',
      '    print(response.status_code)',
    )
    return lines.join('\n')
  }

  const requestArgs = [
    toPythonString(request.method),
    'url',
    'headers=headers',
  ]
  if (request.body.mode === 'raw' || request.body.mode === 'x-www-form-urlencoded') {
    requestArgs.push('data=data')
  }
  else if (request.body.mode === 'json') {
    requestArgs.push('json=data')
  }
  else if (request.body.mode === 'form-data') {
    if (request.body.formData.some(item => item.type === 'text')) {
      requestArgs.push('data=data')
    }
    if (request.body.formData.some(item => item.type === 'file')) {
      requestArgs.push('files=files')
    }
  }

  lines.push(
    '',
    `response = requests.request(${requestArgs.join(', ')})`,
    '',
    'print(response.status_code)',
  )

  return lines.join('\n')
}

function buildGoSnippet(request: ResolvedRequestExport) {
  if (request.body.mode === 'form-data') {
    return buildGoMultipartSnippet(request)
  }
  if (request.body.mode === 'binary') {
    return buildGoBinarySnippet(request)
  }

  const hasStringBody = request.body.mode === 'raw'
    || request.body.mode === 'json'
    || request.body.mode === 'x-www-form-urlencoded'
  const bodyValue = request.body.mode === 'raw'
    ? request.body.raw
    : request.body.mode === 'json'
      ? request.body.json
      : request.body.mode === 'x-www-form-urlencoded'
        ? new URLSearchParams(request.body.urlEncoded.map(item => [item.key, item.value])).toString()
        : ''

  const imports = [
    '"fmt"',
    '"net/http"',
    ...(hasStringBody ? ['"strings"'] : []),
  ]

  const lines = [
    'package main',
    '',
    'import (',
    ...imports.map(item => `\t${item}`),
    ')',
    '',
    'func main() {',
    `\turl := ${toGoString(request.url)}`,
    hasStringBody
      ? `\tbody := strings.NewReader(${toGoRawString(bodyValue)})`
      : '\tvar body *strings.Reader',
    `\treq, err := http.NewRequest(${toGoString(request.method)}, url, body)`,
    '\tif err != nil {',
    '\t\tpanic(err)',
    '\t}',
    '',
    ...request.headers.map(header => `\treq.Header.Set(${toGoString(header.key)}, ${toGoString(header.value)})`),
    ...(request.headers.length > 0 ? [''] : []),
    '\tclient := &http.Client{}',
    '\tresp, err := client.Do(req)',
    '\tif err != nil {',
    '\t\tpanic(err)',
    '\t}',
    '\tdefer resp.Body.Close()',
    '',
    '\tfmt.Println(resp.StatusCode)',
    '}',
  ]

  return lines.join('\n')
}

function buildGoMultipartSnippet(request: ResolvedRequestExport) {
  const usesFile = request.body.formData.some(item => item.type === 'file')
  const lines = [
    'package main',
    '',
    'import (',
    '\t"bytes"',
    '\t"fmt"',
    ...(usesFile ? ['\t"io"', '\t"os"', '\t"path/filepath"'] : []),
    '\t"mime/multipart"',
    '\t"net/http"',
    ')',
    '',
    'func main() {',
    '\tvar body bytes.Buffer',
    '\twriter := multipart.NewWriter(&body)',
    '',
    ...request.body.formData.flatMap((item) => {
      if (item.type === 'file') {
        const filePath = toGoString(item.filePath)
        const fileName = toGoString(item.fileName || item.filePath.split(/[\\/]/).pop() || 'upload.bin')
        return [
          `\tfileBytes, err := os.ReadFile(${filePath})`,
          '\tif err != nil {',
          '\t\tpanic(err)',
          '\t}',
          `\tpart, err := writer.CreateFormFile(${toGoString(item.key)}, ${fileName})`,
          '\tif err != nil {',
          '\t\tpanic(err)',
          '\t}',
          '\tif _, err := io.Copy(part, bytes.NewReader(fileBytes)); err != nil {',
          '\t\tpanic(err)',
          '\t}',
          '',
        ]
      }
      return [`\t_ = writer.WriteField(${toGoString(item.key)}, ${toGoString(item.value)})`]
    }),
    '\tif err := writer.Close(); err != nil {',
    '\t\tpanic(err)',
    '\t}',
    '',
    `\treq, err := http.NewRequest(${toGoString(request.method)}, ${toGoString(request.url)}, &body)`,
    '\tif err != nil {',
    '\t\tpanic(err)',
    '\t}',
    `\treq.Header.Set("Content-Type", writer.FormDataContentType())`,
    ...request.headers
      .filter(header => header.key.toLowerCase() !== 'content-type')
      .map(header => `\treq.Header.Set(${toGoString(header.key)}, ${toGoString(header.value)})`),
    '',
    '\tclient := &http.Client{}',
    '\tresp, err := client.Do(req)',
    '\tif err != nil {',
    '\t\tpanic(err)',
    '\t}',
    '\tdefer resp.Body.Close()',
    '',
    '\tfmt.Println(resp.StatusCode)',
    '}',
  ]

  return lines.join('\n')
}

function buildGoBinarySnippet(request: ResolvedRequestExport) {
  const lines = [
    'package main',
    '',
    'import (',
    '\t"bytes"',
    '\t"fmt"',
    '\t"net/http"',
    '\t"os"',
    ')',
    '',
    'func main() {',
    `\tfileBytes, err := os.ReadFile(${toGoString(request.body.binaryFilePath || 'path/to/file')})`,
    '\tif err != nil {',
    '\t\tpanic(err)',
    '\t}',
    '',
    `\treq, err := http.NewRequest(${toGoString(request.method)}, ${toGoString(request.url)}, bytes.NewReader(fileBytes))`,
    '\tif err != nil {',
    '\t\tpanic(err)',
    '\t}',
    ...request.headers.map(header => `\treq.Header.Set(${toGoString(header.key)}, ${toGoString(header.value)})`),
    '',
    '\tclient := &http.Client{}',
    '\tresp, err := client.Do(req)',
    '\tif err != nil {',
    '\t\tpanic(err)',
    '\t}',
    '\tdefer resp.Body.Close()',
    '',
    '\tfmt.Println(resp.StatusCode)',
    '}',
  ]

  return lines.join('\n')
}

function toPythonDict(entries: Array<{ key: string, value: string }>) {
  if (entries.length === 0) {
    return '{}'
  }

  return `{\n${entries.map(entry => `    ${toPythonString(entry.key)}: ${toPythonString(entry.value)},`).join('\n')}\n}`
}

function toPythonFileDict(entries: Array<{ key: string, filePath: string }>) {
  if (entries.length === 0) {
    return '{}'
  }

  return `{\n${entries.map(entry => `    ${toPythonString(entry.key)}: open(${toPythonString(entry.filePath)}, 'rb'),`).join('\n')}\n}`
}

function toPythonString(value: string) {
  return `"${value.replaceAll('\\', '\\\\').replaceAll('"', '\\"').replaceAll('\n', '\\n').replaceAll('\r', '\\r').replaceAll('\t', '\\t')}"`
}

function toPythonDictFromJson(jsonString: string) {
  return jsonString
    .replace(/\btrue\b/g, 'True')
    .replace(/\bfalse\b/g, 'False')
    .replace(/\bnull\b/g, 'None')
}

function toGoString(value: string) {
  return JSON.stringify(value)
}

function toGoRawString(value: string) {
  if (!value.includes('`')) {
    return `\`${value}\``
  }
  return JSON.stringify(value)
}

function escapeShellSingleQuoted(value: string) {
  return value.replaceAll('\'', '\'\"\'\"\'')
}
