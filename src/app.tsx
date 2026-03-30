import { useEffect } from 'react'
import { preloadMonaco } from '@/components/monaco-editor'
import { WorkbenchLoadingScreen } from '@/features/workbench/components/shared'
import { WorkbenchShell } from '@/features/workbench/components/workbench-shell'
import { useTabStateSync } from '@/features/workbench/hooks/use-tab-state-sync'
import { useWorkbenchBootstrap } from '@/features/workbench/hooks/use-workbench-bootstrap'
import { useWorkbenchStore } from '@/features/workbench/store/workbench-store'
import { checkForAppUpdates } from '@/lib/updater'

export function App() {
  useWorkbenchBootstrap()
  useTabStateSync()
  const isBooting = useWorkbenchStore(state => state.isBooting)

  useEffect(() => {
    void checkForAppUpdates()
  }, [])

  useEffect(() => {
    void preloadMonaco().catch((error) => {
      console.error('Failed to preload modern-monaco.', error)
    })
  }, [])

  useEffect(() => {
    if (import.meta.env.DEV) {
      return
    }

    const handleContextMenu = (event: MouseEvent) => {
      event.preventDefault()
    }

    window.addEventListener('contextmenu', handleContextMenu)

    return () => {
      window.removeEventListener('contextmenu', handleContextMenu)
    }
  }, [])

  if (isBooting) {
    return <WorkbenchLoadingScreen />
  }

  return <WorkbenchShell />
}
