import type * as monacoNS from 'modern-monaco/editor-core'
import { init } from 'modern-monaco'
import { useTheme } from 'next-themes'
import * as React from 'react'

import { cn } from '@/lib/utils'

const lightThemeName = 'github-light'
const darkThemeName = 'github-dark'
const requestBodyModelUri = 'file:///request-body.json'

const FUCK_GLOBAL_DTS_URI = 'file:///fuck-global.d.ts'
const FUCK_GLOBAL_DTS_CONTENT = `export {}

declare global {
  interface FuckKeyValueItem {
    id: string
    key: string
    value: string
    enabled: boolean
    description: string
  }

  interface FuckRequestConfig {
    /** HTTP 方法，如 'GET' | 'POST' | 'PUT' | 'DELETE' 等 */
    method: string
    /** 请求 URL */
    url: string
    /** 请求头列表 */
    headers: FuckKeyValueItem[]
    /** 查询参数列表 */
    query: FuckKeyValueItem[]
    /** 请求体 */
    body: {
      /** 请求体模式：'raw' | 'json' | 'form-data' | 'urlencoded' | 'none' */
      mode: string
      /** 纯文本请求体 */
      raw: string
      /** JSON 请求体 */
      json: string
      /** Form-data 参数列表 */
      formData: FuckKeyValueItem[]
      /** URL-encoded 参数列表 */
      urlEncoded: FuckKeyValueItem[]
    }
  }

  interface FuckResponseData {
    /** HTTP 状态码，如 200、404、500 */
    status: number
    /** 响应头键值对 */
    headers: Record<string, string>
    /** 响应体字符串 */
    body: string
    /** 请求耗时（毫秒） */
    time: number
    /**
     * 解析响应体 JSON 并通过 JSONPath 取值
     *
     * 支持语法：\`$\`（根节点）、\`.key\`（属性）、\`[n]\`（数组下标）
     *
     * @param path JSONPath 表达式，如 \`'$.data.token'\`、\`'$.list[0].id'\`
     * @returns 路径对应的值，路径不存在时返回 undefined
     *
     * @example
     * // 取顶层字段
     * const token = fuck.response.json('$.token')
     *
     * // 取嵌套字段
     * const name = fuck.response.json('$.data.user.name')
     *
     * // 取数组元素
     * const first = fuck.response.json('$.list[0]')
     *
     * // 不传参数时返回完整解析对象
     * const all = fuck.response.json()
     */
    json(path?: string): unknown
  }

  interface FuckEnv {
    /**
     * 读取环境变量
     * @param key 环境变量名称
     * @returns 变量值，不存在则返回 undefined
     */
    get(key: string): string | undefined
    /**
     * 设置环境变量（修改当前环境的变量值）
     * @param key 环境变量名称
     * @param value 变量值
     */
    set(key: string, value: string): void
  }

  interface FuckVars {
    /**
     * 读取临时变量（跨请求持久化在内存中）
     * @param key 变量名称
     * @returns 变量值，不存在则返回 undefined
     */
    get(key: string): unknown
    /**
     * 设置临时变量（跨请求持久化在内存中，可存储任意类型）
     * @param key 变量名称
     * @param value 变量值（任意可序列化类型）
     */
    set(key: string, value: unknown): void
  }

  interface Fuck {
    /** 请求配置对象（预请求脚本中可读写，后请求脚本中只读） */
    config: FuckRequestConfig
    /** 响应数据，仅后请求脚本中可用，预请求脚本中为 null */
    response: FuckResponseData | null
    /** 环境变量操作对象 */
    env: FuckEnv
    /** 跨请求临时变量存储 */
    vars: FuckVars
  }

  /** apifuck 脚本全局对象，提供请求配置、响应数据和变量操作能力 */
  const fuck: Fuck
}`

type Monaco = typeof monacoNS
export type MonacoLanguage = 'html' | 'json' | 'plaintext' | 'javascript' | 'typescript'

export interface EnvironmentVariable {
  key: string
  value: string
  description?: string
}

// Matches: $, $f, $foo, ${, ${f, ${foo — used to detect the prefix to replace on completion
const ENV_COMPLETION_PREFIX = /\$\{?\w*$/

// Matches trailing word chars + optional "}" after cursor — used to compute replace range
const ENV_AFTER_CURSOR = /^\w*\}?/

// Matches ${VARIABLE_NAME} anywhere in a line — used for hover detection
const ENV_VARIABLE_PATTERN = /\$\{(\w+)\}/g

let monacoInstance: Monaco | null = null
let monacoSetupPromise: Promise<Monaco> | null = null

function registerEnvironmentVariableProviders(
  monaco: Monaco,
  editor: monacoNS.editor.IStandaloneCodeEditor,
  variables: EnvironmentVariable[],
  environmentName: string,
) {
  // --- Completion Provider ---
  const completionDisposable = monaco.languages.registerCompletionItemProvider('json', {
    triggerCharacters: ['$', '{'],
    provideCompletionItems(model, position) {
      const textUntilPosition = model.getValueInRange({
        startLineNumber: position.lineNumber,
        startColumn: 1,
        endLineNumber: position.lineNumber,
        endColumn: position.column,
      })

      const match = textUntilPosition.match(ENV_COMPLETION_PREFIX)
      if (!match) {
        return { suggestions: [] }
      }

      const matchedText = match[0] // e.g. "$", "$f", "${", "${fo"
      const replaceStart = position.column - matchedText.length

      // Check if there's a closing "}" right after the cursor we should also replace
      const lineContent = model.getLineContent(position.lineNumber)
      const afterCursor = lineContent.substring(position.column - 1)
      // If the user already typed partial like "${FOO}" we want to replace through the "}"
      const closingMatch = ENV_AFTER_CURSOR.exec(afterCursor)
      const extraCharsAfter = closingMatch ? closingMatch[0].length : 0

      const range = {
        startLineNumber: position.lineNumber,
        endLineNumber: position.lineNumber,
        startColumn: replaceStart,
        endColumn: position.column + extraCharsAfter,
      }

      return {
        suggestions: variables.map(variable => ({
          label: `\${${variable.key}}`,
          kind: monaco.languages.CompletionItemKind.Variable,
          insertText: `\${${variable.key}}`,
          filterText: `\${${variable.key}}${variable.key}`,
          sortText: variable.key,
          detail: `[${environmentName}] ${variable.value}`,
          documentation: variable.description || undefined,
          range,
        })),
      }
    },
  })

  // --- Hover Provider ---
  const hoverDisposable = monaco.languages.registerHoverProvider('json', {
    provideHover(model, position) {
      const lineContent = model.getLineContent(position.lineNumber)
      const col = position.column // 1-based

      // Find all ${VAR} occurrences in the line and check if cursor is inside one
      for (const hoverMatch of lineContent.matchAll(ENV_VARIABLE_PATTERN)) {
        const start = hoverMatch.index + 1 // 1-based start of "$"
        const end = start + hoverMatch[0].length // 1-based exclusive end
        if (col >= start && col <= end) {
          const variableKey = hoverMatch[1]
          const variable = variables.find(v => v.key === variableKey)
          if (!variable) {
            return null
          }

          return {
            range: {
              startLineNumber: position.lineNumber,
              endLineNumber: position.lineNumber,
              startColumn: start,
              endColumn: end,
            },
            contents: [
              { value: `**\${${variableKey}}**` },
              { value: `Value: \`${variable.value}\`` },
              { value: `Environment: *${environmentName}*` },
              ...(variable.description ? [{ value: variable.description }] : []),
            ],
          }
        }
      }

      return null
    },
  })

  // --- Auto-trigger suggest when $ is typed ---
  const typeDisposable = editor.onDidChangeModelContent(() => {
    const model = editor.getModel()
    if (!model) {
      return
    }
    const pos = editor.getPosition()
    if (!pos) {
      return
    }
    const textUntilPos = model.getValueInRange({
      startLineNumber: pos.lineNumber,
      startColumn: 1,
      endLineNumber: pos.lineNumber,
      endColumn: pos.column,
    })
    if (textUntilPos.endsWith('$') || textUntilPos.endsWith('${')) {
      editor.trigger('env-vars', 'editor.action.triggerSuggest', {})
    }
  })

  // --- Cmd+I / Ctrl+I: Insert environment variable via quick pick ---
  const keybindingDisposable = editor.addAction({
    id: 'insert-env-variable',
    label: 'Insert Environment Variable',
    keybindings: [
      monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyI,
    ],
    run(ed) {
      if (variables.length === 0) {
        return
      }

      const pos = ed.getPosition()
      if (!pos) {
        return
      }

      if (variables.length === 1) {
        // Only one variable — insert directly
        ed.executeEdits('insert-env-var', [{
          range: { startLineNumber: pos.lineNumber, endLineNumber: pos.lineNumber, startColumn: pos.column, endColumn: pos.column },
          text: `\${${variables[0].key}}`,
        }])
        return
      }

      // Insert "$" to trigger the completion provider, which will show all env vars
      ed.executeEdits('insert-env-var', [{
        range: { startLineNumber: pos.lineNumber, endLineNumber: pos.lineNumber, startColumn: pos.column, endColumn: pos.column },
        text: '$',
      }])
      ed.trigger('insert-env-var', 'editor.action.triggerSuggest', {})
    },
  })

  return () => {
    completionDisposable.dispose()
    hoverDisposable.dispose()
    typeDisposable.dispose()
    keybindingDisposable.dispose()
  }
}

async function ensureMonacoConfigured() {
  if (monacoInstance) {
    return monacoInstance
  }

  if (!monacoSetupPromise) {
    monacoSetupPromise = init({
      defaultTheme: lightThemeName,
      themes: [lightThemeName, darkThemeName],
      langs: ['html', 'json', 'javascript', 'typescript'],
      lsp: {
        formatting: {
          tabSize: 2,
          insertSpaces: true,
        },
        json: {
          allowComments: false,
          comments: 'error',
          trailingCommas: 'error',
          diagnosticsOptions: {
            validate: true,
          },
        },
        typescript: {
          compilerOptions: {
            checkJs: true,
            strict: false,
            noUnusedLocals: false,
            noUnusedParameters: false,
          },
          diagnosticsOptions: {
            validate: true,
          },
        },
      },
    }).then((instance) => {
      // Register the fuck global type declarations so the TypeScript language
      // service can provide completions, hover docs, parameter hints and type
      // checking for the fuck object in pre/post-request scripts.
      const dtsUri = instance.Uri.parse(FUCK_GLOBAL_DTS_URI)
      if (!instance.editor.getModel(dtsUri)) {
        instance.editor.createModel(FUCK_GLOBAL_DTS_CONTENT, 'typescript', dtsUri)
      }
      monacoInstance = instance
      return instance
    })
  }

  return monacoSetupPromise
}

function applyMonacoTheme(monaco: Monaco, resolvedTheme?: string) {
  monaco.editor.setTheme(resolvedTheme === 'dark' ? darkThemeName : lightThemeName)
}

function MonacoCodeEditor(props: {
  className?: string
  language: MonacoLanguage
  lineNumbers?: 'off' | 'on'
  modelUri: string
  onChange?: (value: string) => void
  placeholder?: string
  readOnly?: boolean
  value: string
  wordWrap?: 'off' | 'on'
  environmentVariables?: EnvironmentVariable[]
  environmentName?: string
}) {
  const containerRef = React.useRef<HTMLDivElement | null>(null)
  const editorRef = React.useRef<monacoNS.editor.IStandaloneCodeEditor | null>(null)
  const lastEditorValueRef = React.useRef(props.value)
  const onChangeRef = React.useRef(props.onChange)
  const { resolvedTheme } = useTheme()
  const [isReady, setIsReady] = React.useState(Boolean(monacoInstance))

  React.useEffect(() => {
    onChangeRef.current = props.onChange
  }, [props.onChange])

  React.useEffect(() => {
    let cancelled = false

    ensureMonacoConfigured()
      .then((monaco) => {
        if (cancelled) {
          return
        }

        setIsReady(true)
        applyMonacoTheme(monaco, resolvedTheme)
      })
      .catch((error) => {
        console.error('Failed to initialize modern-monaco.', error)
      })

    return () => {
      cancelled = true
    }
  }, [resolvedTheme])

  React.useEffect(() => {
    let disposed = false
    let model: monacoNS.editor.ITextModel | null = null
    let cleanup: (() => void) | undefined
    let envProviderCleanup: (() => void) | undefined

    if (!isReady || !containerRef.current) {
      return
    }

    void ensureMonacoConfigured()
      .then((monaco) => {
        if (disposed || !containerRef.current) {
          return
        }

        applyMonacoTheme(monaco, resolvedTheme)

        const modelUri = monaco.Uri.parse(props.modelUri)
        model = monaco.editor.getModel(modelUri)
          ?? monaco.editor.createModel(props.value, props.language, modelUri)

        if (model.getValue() !== props.value) {
          model.setValue(props.value)
        }

        if (model.getLanguageId() !== props.language) {
          monaco.editor.setModelLanguage(model, props.language)
        }

        const editor = monaco.editor.create(containerRef.current, {
          model,
          theme: resolvedTheme === 'dark' ? darkThemeName : lightThemeName,
          tabSize: 2,
          automaticLayout: true,
          contextmenu: false,
          fixedOverflowWidgets: true,
          glyphMargin: false,
          lineDecorationsWidth: 0,
          readOnly: props.readOnly,
          renderValidationDecorations: props.readOnly ? 'off' : 'on',
          lineNumbers: props.lineNumbers ?? 'on',
          lineNumbersMinChars: 2,
          scrollBeyondLastLine: false,
          wordWrap: props.wordWrap ?? 'off',
          placeholder: props.placeholder,
          minimap: {
            enabled: false,
          },
        })

        editorRef.current = editor

        if (props.environmentVariables && props.environmentVariables.length > 0) {
          envProviderCleanup = registerEnvironmentVariableProviders(
            monaco,
            editor,
            props.environmentVariables,
            props.environmentName ?? 'default',
          )
        }

        const valueDisposable = editor.onDidChangeModelContent(() => {
          const nextValue = editor.getValue()
          lastEditorValueRef.current = nextValue
          onChangeRef.current?.(nextValue)
        })

        let layoutFrameId = 0
        let trailingLayoutFrameId = 0
        const scheduleEditorLayout = () => {
          window.cancelAnimationFrame(layoutFrameId)
          window.cancelAnimationFrame(trailingLayoutFrameId)

          layoutFrameId = window.requestAnimationFrame(() => {
            layoutFrameId = 0
            editor.layout()

            // A trailing layout keeps Monaco in sync after flex and resizable
            // containers finish settling their final dimensions.
            trailingLayoutFrameId = window.requestAnimationFrame(() => {
              trailingLayoutFrameId = 0
              editor.layout()
            })
          })
        }

        scheduleEditorLayout()

        // Use ResizeObserver to reliably inform Monaco of container size changes,
        // especially important in flex/ResizablePanel layouts where automaticLayout
        // may not detect height changes correctly.
        const resizeObserver = new ResizeObserver(() => {
          scheduleEditorLayout()
        })
        if (containerRef.current) {
          resizeObserver.observe(containerRef.current)
        }

        cleanup = () => {
          valueDisposable.dispose()
          envProviderCleanup?.()
          resizeObserver.disconnect()
          window.cancelAnimationFrame(layoutFrameId)
          window.cancelAnimationFrame(trailingLayoutFrameId)
          editor.dispose()
          model?.dispose()
          editorRef.current = null
        }
      })
      .catch((error) => {
        console.error('Failed to create modern-monaco editor.', error)
      })

    return () => {
      disposed = true
      cleanup?.()
    }
  }, [
    isReady,
    props.language,
    props.lineNumbers,
    props.modelUri,
    props.readOnly,
    props.wordWrap,
    props.environmentVariables,
    props.environmentName,
    resolvedTheme,
  ])

  React.useEffect(() => {
    if (!isReady || !monacoInstance) {
      return
    }

    applyMonacoTheme(monacoInstance, resolvedTheme)
  }, [isReady, resolvedTheme])

  React.useEffect(() => {
    const editor = editorRef.current
    if (!editor) {
      return
    }

    if (props.value === lastEditorValueRef.current) {
      return
    }

    const model = editor.getModel()
    if (!model) {
      return
    }

    const fullRange = model.getFullModelRange()
    const selection = editor.getSelection()

    editor.executeEdits('external-update', [
      {
        range: fullRange,
        text: props.value,
        forceMoveMarkers: true,
      },
    ], selection ? [selection] : undefined)

    lastEditorValueRef.current = props.value
  }, [props.value])

  return (
    <div
      className={cn(
        'relative flex h-full min-h-[220px] min-w-0 flex-1 overflow-hidden',
        props.className,
      )}
    >
      <div
        ref={containerRef}
        className="monaco-theme-surface h-full min-h-0 w-full overflow-hidden rounded-xl border border-input bg-background"
      />
    </div>
  )
}

function MonacoJsonEditor(props: {
  className?: string
  onChange: (value: string) => void
  placeholder?: string
  value: string
  environmentVariables?: EnvironmentVariable[]
  environmentName?: string
}) {
  return (
    <MonacoCodeEditor
      className={props.className}
      language="json"
      modelUri={requestBodyModelUri}
      onChange={props.onChange}
      placeholder={props.placeholder}
      value={props.value}
      environmentVariables={props.environmentVariables}
      environmentName={props.environmentName}
    />
  )
}

export function MonacoScriptEditor(props: {
  className?: string
  language?: 'javascript' | 'typescript'
  modelUri?: string
  onChange: (value: string) => void
  placeholder?: string
  value: string
}) {
  return (
    <MonacoCodeEditor
      className={props.className}
      language={props.language ?? 'javascript'}
      modelUri={props.modelUri ?? 'file:///script.js'}
      onChange={props.onChange}
      placeholder={props.placeholder}
      value={props.value}
    />
  )
}

export { MonacoCodeEditor, MonacoJsonEditor }
