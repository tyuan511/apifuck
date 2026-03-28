import type { HighlightLanguage } from '@/lib/shiki'
import { useTheme } from 'next-themes'
import * as React from 'react'

import { highlightCodeToHtml } from '@/lib/shiki'
import { cn } from '@/lib/utils'

function escapeHtml(value: string) {
  return value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll('\'', '&#39;')
}

function createFallbackMarkup(code: string) {
  return `<pre class="shiki"><code>${escapeHtml(code || ' ')}</code></pre>`
}

function useHighlightedMarkup(code: string, language: HighlightLanguage) {
  const deferredCode = React.useDeferredValue(code)
  const { resolvedTheme } = useTheme()
  const [markup, setMarkup] = React.useState(() => createFallbackMarkup(code))

  React.useEffect(() => {
    let cancelled = false

    highlightCodeToHtml(
      deferredCode,
      language,
      resolvedTheme === 'dark' ? 'dark' : 'light',
    )
      .then((nextMarkup) => {
        if (!cancelled) {
          React.startTransition(() => {
            setMarkup(nextMarkup)
          })
        }
      })
      .catch(() => {
        if (!cancelled) {
          React.startTransition(() => {
            setMarkup(createFallbackMarkup(deferredCode))
          })
        }
      })

    return () => {
      cancelled = true
    }
  }, [deferredCode, language, resolvedTheme])

  return markup
}

function HighlightedCodeBlock(props: {
  body: string
  className?: string
  emptyLabel?: string
  language: HighlightLanguage
}) {
  const code = props.body || props.emptyLabel || ''
  const markup = useHighlightedMarkup(code, props.language)

  return (
    <div
      className={cn(
        'overflow-auto rounded-xl border border-border/70 bg-muted/40',
        '[&_pre]:m-0 [&_pre]:min-h-full [&_pre]:bg-transparent! [&_pre]:p-3 [&_pre]:font-mono [&_pre]:text-xs [&_pre]:leading-5',
        '[&_code]:font-mono [&_code]:text-xs [&_code]:leading-5',
        props.className,
      )}
      dangerouslySetInnerHTML={{ __html: markup }}
    />
  )
}

function HighlightedCodeTextarea(props: {
  className?: string
  contentClassName?: string
  language: HighlightLanguage
  onChange: (value: string) => void
  placeholder?: string
  value: string
}) {
  const backdropRef = React.useRef<HTMLDivElement | null>(null)
  const markup = useHighlightedMarkup(props.value, props.language)

  const syncScroll = React.useCallback((event: React.UIEvent<HTMLTextAreaElement>) => {
    if (!backdropRef.current) {
      return
    }

    backdropRef.current.scrollLeft = event.currentTarget.scrollLeft
    backdropRef.current.scrollTop = event.currentTarget.scrollTop
  }, [])

  return (
    <div
      className={cn(
        'relative min-h-[220px] flex-1 overflow-hidden rounded-xl border border-input bg-transparent focus-within:border-ring focus-within:ring-3 focus-within:ring-ring/50 dark:bg-input/30',
        props.className,
      )}
    >
      <div
        ref={backdropRef}
        aria-hidden="true"
        className="pointer-events-none absolute inset-0 overflow-auto"
      >
        <div
          className={cn(
            '[&_pre]:m-0 [&_pre]:min-h-full [&_pre]:bg-transparent! [&_pre]:p-3 [&_pre]:font-mono [&_pre]:text-xs [&_pre]:leading-5 [&_pre]:whitespace-pre [&_code]:font-mono [&_code]:text-xs [&_code]:leading-5',
            props.contentClassName,
          )}
          dangerouslySetInnerHTML={{ __html: markup }}
        />
      </div>

      <textarea
        autoCapitalize="off"
        spellCheck={false}
        value={props.value}
        wrap="off"
        onChange={event => props.onChange(event.target.value)}
        onScroll={syncScroll}
        placeholder={props.placeholder}
        className={cn(
          'relative z-10 min-h-[220px] w-full resize-none bg-transparent p-3 font-mono text-xs leading-5 text-transparent caret-foreground outline-none placeholder:text-muted-foreground selection:bg-primary/20',
          props.contentClassName,
        )}
      />
    </div>
  )
}

export { HighlightedCodeBlock, HighlightedCodeTextarea }
