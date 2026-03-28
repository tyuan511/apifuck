export type HighlightLanguage = 'html' | 'json'
export type HighlightThemeMode = 'dark' | 'light'

const darkTheme = 'github-dark-default'
const lightTheme = 'github-light-default'

interface ShikiHighlighterLike {
  codeToHtml: (
    code: string,
    options: {
      lang: HighlightLanguage
      theme: string
    },
  ) => string
}

let highlighterPromise: Promise<ShikiHighlighterLike> | null = null

export async function getHighlighter() {
  if (!highlighterPromise) {
    highlighterPromise = Promise.all([
      import('shiki/core'),
      import('shiki/engine/javascript'),
      import('@shikijs/langs/html'),
      import('@shikijs/langs/json'),
      import('@shikijs/themes/github-dark-default'),
      import('@shikijs/themes/github-light-default'),
    ]).then(([
      { createHighlighterCore },
      { createJavaScriptRegexEngine },
      htmlLanguage,
      jsonLanguage,
      darkThemeDefinition,
      lightThemeDefinition,
    ]) => createHighlighterCore({
      engine: createJavaScriptRegexEngine(),
      langs: [htmlLanguage.default, jsonLanguage.default],
      themes: [darkThemeDefinition.default, lightThemeDefinition.default],
    }))
  }

  return highlighterPromise
}

export async function highlightCodeToHtml(
  code: string,
  language: HighlightLanguage,
  themeMode: HighlightThemeMode,
) {
  const highlighter = await getHighlighter()

  return highlighter.codeToHtml(code || ' ', {
    lang: language,
    theme: themeMode === 'dark' ? darkTheme : lightTheme,
  })
}
