import { useTheme } from 'next-themes'
import { useEffect } from 'react'
import { toast } from 'sonner'
import { openStartupWorkspace, readTabState } from '@/lib/app-config'
import { readApi } from '@/lib/workspace'
import { useWorkbenchStore } from '../store/workbench-store'
import { findApiLocation } from '../utils'

export function useWorkbenchBootstrap() {
  const { setTheme } = useTheme()

  const activeProjectId = useWorkbenchStore(state => state.activeProjectId)
  const selectedTreeNode = useWorkbenchStore(state => state.selectedTreeNode)
  const workspace = useWorkbenchStore(state => state.workspace)
  const hydrateWorkspace = useWorkbenchStore(state => state.hydrateWorkspace)
  const hydrateTabState = useWorkbenchStore(state => state.hydrateTabState)
  const setRequestLoaded = useWorkbenchStore(state => state.setRequestLoaded)
  const syncTabState = useWorkbenchStore(state => state.syncTabState)
  const setIsBooting = useWorkbenchStore(state => state.setIsBooting)
  const ensureActiveProject = useWorkbenchStore(state => state.ensureActiveProject)
  const ensureTreeSelection = useWorkbenchStore(state => state.ensureTreeSelection)

  useEffect(() => {
    let cancelled = false

    async function bootstrapWorkbench() {
      setIsBooting(true)
      try {
        const startup = await openStartupWorkspace()
        if (cancelled) {
          return
        }

        hydrateWorkspace({
          workspacePath: startup.workspacePath,
          workspaceSnapshot: startup.workspaceSnapshot,
        })
        setTheme(startup.appConfig.theme)

        // Restore saved tab state and load tab content
        const [savedTabs, savedActiveId] = await readTabState()
        if (savedTabs.length > 0 && startup.workspaceSnapshot) {
          // Filter tabs to only include requests that still exist in workspace
          const validTabs = savedTabs.filter(tab =>
            findApiLocation(startup.workspaceSnapshot, tab.requestId) !== null,
          )

          // Ensure active tab is a valid one
          const activeIdIsValid = savedActiveId && validTabs.some(t => t.requestId === savedActiveId)
          const initialActiveId = activeIdIsValid ? savedActiveId : validTabs[0]?.requestId ?? null

          hydrateTabState(validTabs, initialActiveId)

          // Load each valid tab's request content (don't change active tab)
          const loadedTabs: string[] = []
          await Promise.all(
            validTabs.map(tab =>
              readApi(tab.requestId)
                .then((definition) => {
                  setRequestLoaded(definition, false) // Don't change active tab
                  loadedTabs.push(tab.requestId)
                })
                .catch(() => {
                  // Tab will be removed - request no longer exists
                }),
            ),
          )

          // Remove tabs that failed to load content (request was deleted)
          const failedTabs = validTabs.filter(tab => !loadedTabs.includes(tab.requestId))
          if (failedTabs.length > 0) {
            for (const tab of failedTabs) {
              useWorkbenchStore.getState().closeRequestTab(tab.requestId)
            }
            syncTabState()
          }
        }
      }
      catch (error) {
        if (!cancelled) {
          toast.error(error instanceof Error ? error.message : String(error))
        }
      }
      finally {
        if (!cancelled) {
          setIsBooting(false)
        }
      }
    }

    void bootstrapWorkbench()

    return () => {
      cancelled = true
    }
  }, [hydrateWorkspace, hydrateTabState, setRequestLoaded, syncTabState, setIsBooting, setTheme])

  useEffect(() => {
    ensureActiveProject()
  }, [activeProjectId, ensureActiveProject, workspace])

  useEffect(() => {
    ensureTreeSelection()
  }, [activeProjectId, ensureTreeSelection, selectedTreeNode, workspace])
}
