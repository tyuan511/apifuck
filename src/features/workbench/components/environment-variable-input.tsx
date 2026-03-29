import type { EnvironmentVariable } from '@/components/monaco-editor'
import * as React from 'react'
import { createPortal } from 'react-dom'
import { Input } from '@/components/ui/input'
import { cn } from '@/lib/utils'

interface EnvironmentVariableInputProps {
  value: string
  onChange: (value: string) => void
  variables: EnvironmentVariable[]
  environmentName: string
  placeholder?: string
  disabled?: boolean
  className?: string
}

export function EnvironmentVariableInput(props: EnvironmentVariableInputProps) {
  const [showSuggestions, setShowSuggestions] = React.useState(false)
  const [suggestionFilter, setSuggestionFilter] = React.useState('')
  const [selectedIndex, setSelectedIndex] = React.useState(0)
  const inputRef = React.useRef<HTMLInputElement>(null)

  const filteredVariables = React.useMemo(() => {
    if (!suggestionFilter) {
      return props.variables
    }
    const lower = suggestionFilter.toLowerCase()
    return props.variables.filter(v => v.key.toLowerCase().includes(lower))
  }, [props.variables, suggestionFilter])

  const handleInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const newValue = e.target.value
    props.onChange(newValue)

    // Detect ${ trigger for autocomplete
    const cursorPos = e.target.selectionStart ?? 0
    const textBeforeCursor = newValue.slice(0, cursorPos)

    // Check if we should show suggestions
    const dollarPos = textBeforeCursor.lastIndexOf('$')
    if (dollarPos >= 0) {
      const afterDollar = textBeforeCursor.slice(dollarPos)
      if (afterDollar === '$' || afterDollar.startsWith('${')) {
        // Extract filter text after ${
        if (afterDollar.startsWith('${')) {
          setSuggestionFilter(afterDollar.slice(2))
        }
        else {
          setSuggestionFilter('')
        }
        setShowSuggestions(true)
        setSelectedIndex(0)
      }
      else {
        setShowSuggestions(false)
      }
    }
    else {
      setShowSuggestions(false)
    }
  }

  const insertVariable = React.useCallback((variableKey: string) => {
    const input = inputRef.current
    if (!input) {
      return
    }

    const cursorPos = input.selectionStart ?? 0
    const valueBeforeCursor = props.value.slice(0, cursorPos)
    const valueAfterCursor = props.value.slice(cursorPos)

    // Find the $ or ${ position to replace
    const dollarPos = valueBeforeCursor.lastIndexOf('$')
    if (dollarPos < 0) {
      return
    }

    const beforeDollar = props.value.slice(0, dollarPos)
    const newValue = `${beforeDollar}\${${variableKey}}${valueAfterCursor}`

    props.onChange(newValue)
    setShowSuggestions(false)
    setSuggestionFilter('')

    // Set cursor position after the inserted variable
    requestAnimationFrame(() => {
      const newCursorPos = beforeDollar.length + variableKey.length + 3
      input.setSelectionRange(newCursorPos, newCursorPos)
      input.focus()
    })
  }, [props.value, props.onChange])

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (!showSuggestions || filteredVariables.length === 0) {
      if (e.key === 'Escape') {
        setShowSuggestions(false)
      }
      return
    }

    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault()
        setSelectedIndex(i => Math.min(i + 1, filteredVariables.length - 1))
        break
      case 'ArrowUp':
        e.preventDefault()
        setSelectedIndex(i => Math.max(i - 1, 0))
        break
      case 'Enter':
      case 'Tab':
        e.preventDefault()
        if (filteredVariables[selectedIndex]) {
          insertVariable(filteredVariables[selectedIndex].key)
        }
        break
      case 'Escape':
        e.preventDefault()
        setShowSuggestions(false)
        break
    }
  }

  const handleBlur = () => {
    // Delay to allow click on suggestion
    setTimeout(() => {
      setShowSuggestions(false)
    }, 150)
  }

  const suggestionsRef = React.useRef<HTMLDivElement>(null)

  const getDropdownPosition = React.useCallback(() => {
    const input = inputRef.current
    if (!input) {
      return { top: 0, left: 0, width: 200 }
    }
    const rect = input.getBoundingClientRect()
    return {
      top: rect.bottom + window.scrollY + 4,
      left: rect.left + window.scrollX,
      width: rect.width,
    }
  }, [])

  const [dropdownPosition, setDropdownPosition] = React.useState({ top: 0, left: 0, width: 200 })

  React.useEffect(() => {
    if (showSuggestions && filteredVariables.length > 0) {
      setDropdownPosition(getDropdownPosition())
    }
  }, [showSuggestions, filteredVariables.length, getDropdownPosition])

  const suggestionsContent = showSuggestions && filteredVariables.length > 0
    ? (
        <div
          ref={suggestionsRef}
          className="fixed z-50 mt-1 max-h-60 overflow-auto rounded-md border bg-popover p-1 shadow-md"
          style={{ top: dropdownPosition.top, left: dropdownPosition.left, width: dropdownPosition.width, minWidth: '200px' }}
        >
          {filteredVariables.map((variable, index) => (
            <button
              key={variable.key}
              type="button"
              onClick={() => insertVariable(variable.key)}
              className={cn(
                'flex w-full cursor-pointer items-start gap-2 rounded-sm px-2 py-1 text-left text-xs',
                index === selectedIndex && 'bg-accent',
              )}
            >
              <span className="font-mono text-primary">
                $
                {variable.key}
              </span>
              <span className="text-muted-foreground">=</span>
              <span className="truncate text-muted-foreground">{variable.value}</span>
            </button>
          ))}
        </div>
      )
    : null

  return (
    <div className="relative">
      <Input
        ref={inputRef}
        value={props.value}
        onChange={handleInputChange}
        onBlur={handleBlur}
        onKeyDown={handleKeyDown}
        disabled={props.disabled}
        placeholder={props.placeholder}
        className={props.className}
        autoCapitalize="none"
        spellCheck={false}
      />

      {/* Autocomplete dropdown - rendered via portal to escape overflow containers */}
      {suggestionsContent && typeof document !== 'undefined'
        ? createPortal(suggestionsContent, document.body)
        : null}
    </div>
  )
}
