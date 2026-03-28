import type * as monacoNS from 'modern-monaco/editor-core'
import { init } from 'modern-monaco'
import { useTheme } from 'next-themes'
import * as React from 'react'

import { cn } from '@/lib/utils'

const lightThemeName = 'github-light-default'
const darkThemeName = 'github-dark-default'
const requestBodyModelUri = 'file:///request-body.json'

type Monaco = typeof monacoNS
export type MonacoLanguage = 'html' | 'json' | 'plaintext'

let monacoInstance: Monaco | null = null
let monacoSetupPromise: Promise<Monaco> | null = null

async function ensureMonacoConfigured() {
  if (monacoInstance) {
    return monacoInstance
  }

  if (!monacoSetupPromise) {
    monacoSetupPromise = init({
      defaultTheme: lightThemeName,
      themes: [lightThemeName, darkThemeName],
      langs: ['html', 'json'],
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
      },
    }).then((instance) => {
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
}) {
  const containerRef = React.useRef<HTMLDivElement | null>(null)
  const editorRef = React.useRef<monacoNS.editor.IStandaloneCodeEditor | null>(null)
  const lastEditorValueRef = React.useRef(props.value)
  const onChangeRef = React.useRef(props.onChange)
  const { resolvedTheme } = useTheme()
  const [isFocused, setIsFocused] = React.useState(false)
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
          minimap: {
            enabled: false,
          },
        })

        editorRef.current = editor

        const valueDisposable = editor.onDidChangeModelContent(() => {
          const nextValue = editor.getValue()
          lastEditorValueRef.current = nextValue
          onChangeRef.current?.(nextValue)
        })
        const focusDisposable = editor.onDidFocusEditorText(() => {
          setIsFocused(true)
        })
        const blurDisposable = editor.onDidBlurEditorText(() => {
          setIsFocused(false)
        })

        cleanup = () => {
          blurDisposable.dispose()
          focusDisposable.dispose()
          valueDisposable.dispose()
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
        'relative min-h-[220px] flex-1 overflow-visible',
        props.className,
      )}
    >
      {!props.value && !isFocused && props.placeholder && (
        <div className="pointer-events-none absolute top-3 left-12 z-10 font-mono text-xs leading-5 text-muted-foreground">
          <pre className="whitespace-pre-wrap">{props.placeholder}</pre>
        </div>
      )}

      <div className="h-full min-h-[220px] overflow-hidden rounded-xl border border-input">
        <div ref={containerRef} className="h-full min-h-[220px] w-full" />
      </div>
    </div>
  )
}

function MonacoJsonEditor(props: {
  className?: string
  onChange: (value: string) => void
  placeholder?: string
  value: string
}) {
  return (
    <MonacoCodeEditor
      className={props.className}
      language="json"
      modelUri={requestBodyModelUri}
      onChange={props.onChange}
      placeholder={props.placeholder}
      value={props.value}
    />
  )
}

export { MonacoCodeEditor, MonacoJsonEditor }
