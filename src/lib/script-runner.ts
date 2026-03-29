/**
 * Script runner for pre-request and post-request scripts.
 * Provides a sandboxed `fuck` global object for manipulating request config and response.
 */

export interface RequestConfig {
  method: string
  url: string
  headers: Array<{ id: string, key: string, value: string, enabled: boolean, description: string }>
  query: Array<{ id: string, key: string, value: string, enabled: boolean, description: string }>
  body: {
    mode: string
    raw: string
    json: string
    formData: Array<{ id: string, key: string, value: string, enabled: boolean, description: string }>
    urlEncoded: Array<{ id: string, key: string, value: string, enabled: boolean, description: string }>
  }
}

export interface ResponseData {
  status: number
  headers: Record<string, string>
  body: string
  time: number
}

export interface ScriptContext {
  phase: 'pre-request' | 'post-request'
  config: RequestConfig
  response: ResponseData | null
  env: {
    get: (key: string) => string | undefined
    set: (key: string, value: string) => void
  }
  vars: {
    get: (key: string) => unknown
    set: (key: string, value: unknown) => void
  }
}

type ScriptRunner = (script: string, context: ScriptContext) => ScriptResult

export interface ScriptResult {
  config?: RequestConfig
  response?: ResponseData
  error?: string
}

/**
 * Evaluate a simple dot-bracket JSONPath against a parsed JSON value.
 * Supports: $ (root), .key (property), [n] (array index).
 * Returns undefined when the path does not exist.
 */
function evaluateJsonPath(root: unknown, path: string): unknown {
  if (!path || path === '$') {
    return root
  }

  const normalized = path.startsWith('$') ? path.slice(1) : path
  const tokenRe = /\.([^.[\]]+)|\[(\d+)\]/g
  let current: unknown = root
  let match: RegExpExecArray | null

  // eslint-disable-next-line no-cond-assign
  while ((match = tokenRe.exec(normalized)) !== null) {
    if (current === null || current === undefined || typeof current !== 'object') {
      return undefined
    }
    const key = match[1] ?? match[2]
    current = (current as Record<string, unknown>)[key]
  }

  return current
}

function createScriptRunner(): ScriptRunner {
  return function runScript(script: string, context: ScriptContext): ScriptResult {
    if (!script.trim()) {
      return {}
    }

    // Clone config and response so we can detect modifications
    const originalConfig = JSON.parse(JSON.stringify(context.config)) as RequestConfig
    const originalResponse = context.response ? JSON.parse(JSON.stringify(context.response)) as ResponseData : null

    // Track changes
    let mutatedConfig = false
    let mutatedResponse = false
    const scriptVars = new Map<string, unknown>()

    // Attach a non-enumerable json() helper to the response so it does not
    // interfere with mutation detection (JSON.stringify skips non-enumerable).
    if (context.phase === 'post-request' && context.response) {
      const response = context.response
      Object.defineProperty(response, 'json', {
        value(path?: string): unknown {
          let parsed: unknown
          try {
            parsed = JSON.parse(response.body)
          }
          catch {
            throw new Error('Response body is not valid JSON')
          }
          return evaluateJsonPath(parsed, path ?? '$')
        },
        enumerable: false,
        configurable: true,
      })
    }

    // Build the fuck object
    const fuck = {
      config: context.phase === 'pre-request' ? context.config : Object.freeze(context.config),
      response: context.phase === 'post-request' && context.response
        ? context.response
        : null,
      env: {
        get: (key: string) => context.env.get(key),
        set: (key: string, value: string) => {
          context.env.set(key, value)
        },
      },
      vars: {
        get: (key: string) => scriptVars.get(key) ?? context.vars.get(key),
        set: (key: string, value: unknown) => {
          scriptVars.set(key, value)
        },
      },
    }

    try {
      // Use Function constructor for sandboxed execution (no access to outer scope except fuck)
      // eslint-disable-next-line no-new-func
      const fn = new Function('fuck', `
        "use strict";
        ${script}
      `)

      fn(fuck)

      // Detect if config was mutated (simple deep compare for top-level)
      if (JSON.stringify(context.config) !== JSON.stringify(originalConfig)) {
        mutatedConfig = true
      }

      if (context.response && originalResponse && JSON.stringify(context.response) !== JSON.stringify(originalResponse)) {
        mutatedResponse = true
      }

      return {
        config: mutatedConfig ? context.config : undefined,
        response: mutatedResponse ? context.response ?? undefined : undefined,
      }
    }
    catch (error) {
      return {
        error: error instanceof Error ? error.message : String(error),
      }
    }
  }
}

export const runScript = createScriptRunner()

export function createPreRequestContext(
  config: RequestConfig,
  getEnvVar: (key: string) => string | undefined,
  setEnvVar: (key: string, value: string) => void,
  getVar: (key: string) => unknown,
  setVar: (key: string, value: unknown) => void,
): ScriptContext {
  return {
    phase: 'pre-request',
    config,
    response: null,
    env: { get: getEnvVar, set: setEnvVar },
    vars: { get: getVar, set: setVar },
  }
}

export function createPostRequestContext(
  config: RequestConfig,
  response: ResponseData,
  getEnvVar: (key: string) => string | undefined,
  setEnvVar: (key: string, value: string) => void,
  getVar: (key: string) => unknown,
  setVar: (key: string, value: unknown) => void,
): ScriptContext {
  return {
    phase: 'post-request',
    config,
    response,
    env: { get: getEnvVar, set: setEnvVar },
    vars: { get: getVar, set: setVar },
  }
}
