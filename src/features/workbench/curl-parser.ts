import type { AuthConfig, FormDataEntry, KeyValue, RequestDefinition } from '@/lib/project'
import { createDefaultRequest } from '@/lib/project'
import { createKeyValueDraft, getBaseUrl, normalizeRequestUrl, parseUrlQueryParams } from './utils'

interface ParsedCurlCommand {
  method: string
  url: string
  request: RequestDefinition
}

interface ParsedHeader {
  key: string
  value: string
}

interface ParsedFormField {
  key: string
  value: string
  type: 'text' | 'file'
}

const dataFlagsRequiringValue = new Set([
  '-d',
  '--data',
  '--data-ascii',
  '--data-binary',
  '--data-raw',
  '--data-urlencode',
  '--json',
])

const formFlagsRequiringValue = new Set([
  '-F',
  '--form',
  '--form-string',
])

const headerFlagsRequiringValue = new Set(['-H', '--header'])
const methodFlagsRequiringValue = new Set(['-X', '--request'])
const urlFlagsRequiringValue = new Set(['--url'])
const authFlagsRequiringValue = new Set(['-u', '--user'])
const cookieFlagsRequiringValue = new Set(['-b', '--cookie'])

export function looksLikeCurlCommand(input: string): boolean {
  return input.trimStart().startsWith('curl ')
    || input.trim() === 'curl'
}

export function parseCurlCommand(input: string): ParsedCurlCommand | null {
  if (!looksLikeCurlCommand(input)) {
    return null
  }

  const tokens = tokenizeShellLikeCommand(input)
  if (tokens.length === 0 || tokens[0] !== 'curl') {
    return null
  }

  let explicitMethod = ''
  let explicitUrl = ''
  let sawJsonFlag = false
  let sawBodyFlag = false
  const dataParts: string[] = []
  const headers: ParsedHeader[] = []
  const formFields: ParsedFormField[] = []
  const cookies: KeyValue[] = []
  let basicAuthValue = ''

  for (let index = 1; index < tokens.length; index += 1) {
    const token = tokens[index]
    if (!token) {
      continue
    }

    const [flagName, inlineValue] = splitFlagAndInlineValue(token)

    if (methodFlagsRequiringValue.has(flagName)) {
      const value = inlineValue ?? tokens[++index] ?? ''
      explicitMethod = value.trim().toUpperCase()
      continue
    }

    if (urlFlagsRequiringValue.has(flagName)) {
      const value = inlineValue ?? tokens[++index] ?? ''
      if (value.trim()) {
        explicitUrl = value.trim()
      }
      continue
    }

    if (headerFlagsRequiringValue.has(flagName)) {
      const value = inlineValue ?? tokens[++index] ?? ''
      const parsedHeader = parseHeader(value)
      if (parsedHeader) {
        headers.push(parsedHeader)
      }
      continue
    }

    if (authFlagsRequiringValue.has(flagName)) {
      basicAuthValue = inlineValue ?? tokens[++index] ?? ''
      continue
    }

    if (cookieFlagsRequiringValue.has(flagName)) {
      const value = inlineValue ?? tokens[++index] ?? ''
      cookies.push(...parseCookieEntries(value))
      continue
    }

    if (dataFlagsRequiringValue.has(flagName)) {
      const value = inlineValue ?? tokens[++index] ?? ''
      sawBodyFlag = true
      if (flagName === '--json') {
        sawJsonFlag = true
      }
      dataParts.push(value)
      continue
    }

    if (formFlagsRequiringValue.has(flagName)) {
      const value = inlineValue ?? tokens[++index] ?? ''
      sawBodyFlag = true
      const parsedField = parseFormField(value, flagName === '--form-string')
      if (parsedField) {
        formFields.push(parsedField)
      }
      continue
    }

    if (flagName === '-I' || flagName === '--head') {
      explicitMethod = 'HEAD'
      continue
    }

    if (flagName === '-G' || flagName === '--get') {
      explicitMethod = 'GET'
      continue
    }

    if (flagName === '--location' || flagName === '-L' || flagName === '--compressed' || flagName === '-s' || flagName === '-S' || flagName === '--silent' || flagName === '--include' || flagName === '-i' || flagName === '--insecure' || flagName === '-k') {
      continue
    }

    if (!token.startsWith('-') && !explicitUrl) {
      explicitUrl = token.trim()
    }
  }

  const normalizedUrl = normalizeRequestUrl(explicitUrl)
  if (!normalizedUrl) {
    return null
  }

  const request = createDefaultRequest()
  request.query = parseUrlQueryParams(normalizedUrl, request.query)
  request.headers = buildHeaderEntries(mergeCookieHeader(headers, cookies))
  request.cookies = []
  request.auth = buildAuthConfig(basicAuthValue, request.headers)

  const contentTypeHeader = findHeaderValue(headers, 'content-type')
  request.body = buildBody({
    contentTypeHeader,
    dataParts,
    formFields,
    request,
    sawBodyFlag,
    sawJsonFlag,
  })

  return {
    method: determineMethod({
      explicitMethod,
      hasBody: sawBodyFlag,
      request,
    }),
    url: getBaseUrl(normalizedUrl),
    request,
  }
}

function tokenizeShellLikeCommand(input: string) {
  const normalized = input.replace(/\\\r?\n/g, ' ')
  const tokens: string[] = []
  let current = ''
  let quote: '\'' | '"' | null = null
  let escaping = false

  for (let index = 0; index < normalized.length; index += 1) {
    const char = normalized[index]

    if (escaping) {
      current += char
      escaping = false
      continue
    }

    if (char === '\\') {
      escaping = true
      continue
    }

    if (quote) {
      if (char === quote) {
        quote = null
      }
      else {
        current += char
      }
      continue
    }

    if (char === '\'' || char === '"') {
      quote = char
      continue
    }

    if (/\s/.test(char)) {
      if (current) {
        tokens.push(current)
        current = ''
      }
      continue
    }

    current += char
  }

  if (current) {
    tokens.push(current)
  }

  return tokens
}

function splitFlagAndInlineValue(token: string): [string, string | null] {
  if (!token.startsWith('--')) {
    return [token, null]
  }

  const equalIndex = token.indexOf('=')
  if (equalIndex === -1) {
    return [token, null]
  }

  return [token.slice(0, equalIndex), token.slice(equalIndex + 1)]
}

function parseHeader(raw: string): ParsedHeader | null {
  const separatorIndex = raw.indexOf(':')
  if (separatorIndex === -1) {
    return null
  }

  const key = raw.slice(0, separatorIndex).trim()
  if (!key) {
    return null
  }

  return {
    key,
    value: raw.slice(separatorIndex + 1).trim(),
  }
}

function findHeaderValue(headers: ParsedHeader[], name: string) {
  return headers.find(header => header.key.trim().toLowerCase() === name)?.value ?? ''
}

function buildHeaderEntries(headers: ParsedHeader[]): KeyValue[] {
  return headers.map(header => ({
    ...createKeyValueDraft(),
    key: header.key,
    value: header.value,
  }))
}

function mergeCookieHeader(headers: ParsedHeader[], cookies: KeyValue[]): ParsedHeader[] {
  if (cookies.length === 0) {
    return headers
  }

  const cookieValue = cookies
    .filter(cookie => cookie.key.trim())
    .map(cookie => `${cookie.key.trim()}=${cookie.value}`)
    .join('; ')

  if (!cookieValue) {
    return headers
  }

  const existingCookieIndex = headers.findIndex(header => header.key.trim().toLowerCase() === 'cookie')
  if (existingCookieIndex >= 0) {
    return headers.map((header, index) => (index === existingCookieIndex
      ? { ...header, value: cookieValue }
      : header))
  }

  return [
    ...headers,
    {
      key: 'Cookie',
      value: cookieValue,
    },
  ]
}

function buildAuthConfig(rawBasicAuth: string, headers: ParsedHeader[]): AuthConfig {
  const defaultAuth = createDefaultRequest().auth
  if (rawBasicAuth.includes(':')) {
    const separatorIndex = rawBasicAuth.indexOf(':')
    return {
      ...defaultAuth,
      inherit: false,
      authType: 'basic',
      basic: {
        username: rawBasicAuth.slice(0, separatorIndex),
        password: rawBasicAuth.slice(separatorIndex + 1),
      },
    }
  }

  const authorizationValue = findHeaderValue(headers, 'authorization').trim()
  if (!authorizationValue) {
    return defaultAuth
  }

  const bearerToken = extractAuthorizationSchemeValue(authorizationValue, 'bearer')
  if (bearerToken) {
    return {
      ...defaultAuth,
      inherit: false,
      authType: 'bearer',
      bearerToken,
    }
  }

  const basicToken = extractAuthorizationSchemeValue(authorizationValue, 'basic')
  if (basicToken) {
    const decoded = decodeBase64(basicToken)
    if (decoded.includes(':')) {
      const separatorIndex = decoded.indexOf(':')
      return {
        ...defaultAuth,
        inherit: false,
        authType: 'basic',
        basic: {
          username: decoded.slice(0, separatorIndex),
          password: decoded.slice(separatorIndex + 1),
        },
      }
    }
  }

  return defaultAuth
}

function decodeBase64(value: string) {
  try {
    return globalThis.atob(value)
  }
  catch {
    return ''
  }
}

function extractAuthorizationSchemeValue(value: string, scheme: string) {
  const separatorIndex = value.indexOf(' ')
  if (separatorIndex === -1) {
    return ''
  }

  const prefix = value.slice(0, separatorIndex).trim().toLowerCase()
  if (prefix !== scheme) {
    return ''
  }

  return value.slice(separatorIndex + 1).trim()
}

function buildBody(args: {
  contentTypeHeader: string
  dataParts: string[]
  formFields: ParsedFormField[]
  request: RequestDefinition
  sawBodyFlag: boolean
  sawJsonFlag: boolean
}): RequestDefinition['body'] {
  const body = {
    ...args.request.body,
    mode: 'none' as RequestDefinition['body']['mode'],
    raw: '',
    json: '',
    formData: [] as FormDataEntry[],
    urlEncoded: [] as KeyValue[],
    binary: {},
  }

  if (args.formFields.length > 0) {
    body.mode = 'form-data'
    body.formData = args.formFields.map(field => ({
      ...createKeyValueDraft(),
      key: field.key,
      value: field.value,
      entryType: field.type,
      filePath: field.type === 'file' ? field.value : '',
      fileName: field.type === 'file' ? extractFileName(field.value) : '',
      contentType: '',
    }))
    return body
  }

  if (!args.sawBodyFlag) {
    return body
  }

  const mergedData = args.dataParts.join('&')
  const normalizedContentType = args.contentTypeHeader.toLowerCase()
  const shouldTreatAsJson = args.sawJsonFlag
    || normalizedContentType.includes('application/json')
    || looksLikeJsonPayload(mergedData)

  if (shouldTreatAsJson) {
    body.mode = 'json'
    body.json = mergedData
    return body
  }

  if (normalizedContentType.includes('application/x-www-form-urlencoded')) {
    body.mode = 'x-www-form-urlencoded'
    body.urlEncoded = buildUrlEncodedEntries(mergedData)
    return body
  }

  body.mode = 'raw'
  body.raw = mergedData
  return body
}

function parseFormField(raw: string, forceText: boolean): ParsedFormField | null {
  const separatorIndex = raw.indexOf('=')
  if (separatorIndex === -1) {
    return null
  }

  const key = raw.slice(0, separatorIndex).trim()
  if (!key) {
    return null
  }

  const value = raw.slice(separatorIndex + 1)
  if (!forceText && value.startsWith('@')) {
    return {
      key,
      value: value.slice(1),
      type: 'file',
    }
  }

  return {
    key,
    value,
    type: 'text',
  }
}

function extractFileName(filePath: string) {
  return filePath.split(/[/\\]/).pop() ?? filePath
}

function buildUrlEncodedEntries(raw: string): KeyValue[] {
  const params = new URLSearchParams(raw)
  const entries: KeyValue[] = []
  params.forEach((value, key) => {
    entries.push({
      ...createKeyValueDraft(),
      key,
      value,
    })
  })
  return entries
}

function parseCookieEntries(raw: string): KeyValue[] {
  return raw
    .split(';')
    .map(item => item.trim())
    .filter(Boolean)
    .flatMap((item) => {
      const separatorIndex = item.indexOf('=')
      if (separatorIndex === -1) {
        return []
      }

      const key = item.slice(0, separatorIndex).trim()
      if (!key) {
        return []
      }

      return [{
        ...createKeyValueDraft(),
        key,
        value: item.slice(separatorIndex + 1).trim(),
      }]
    })
}

function looksLikeJsonPayload(value: string) {
  const trimmed = value.trim()
  return (trimmed.startsWith('{') && trimmed.endsWith('}'))
    || (trimmed.startsWith('[') && trimmed.endsWith(']'))
}

function determineMethod(args: {
  explicitMethod: string
  hasBody: boolean
  request: RequestDefinition
}) {
  if (args.explicitMethod) {
    return args.explicitMethod
  }

  if (args.request.body.mode !== 'none' || args.hasBody) {
    return 'POST'
  }

  return 'GET'
}
