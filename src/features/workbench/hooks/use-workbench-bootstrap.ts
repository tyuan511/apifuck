import { useTheme } from 'next-themes'
import { useEffect, useRef } from 'react'
import { toast } from 'sonner'
import { openStartupProject, readTabState } from '@/lib/app-config'
import { readApi } from '@/lib/project'
import { useWorkbenchStore } from '../store/workbench-store'
import { findApiLocation } from '../utils'

export function useWorkbenchBootstrap() {
  const { setTheme } = useTheme()
  const hasBootstrappedRef = useRef(false)

  const selectedTreeNode = useWorkbenchStore(state => state.selectedTreeNode)
  const project = useWorkbenchStore(state => state.project)
  const hydrateProject = useWorkbenchStore(state => state.hydrateProject)
  const hydrateTabState = useWorkbenchStore(state => state.hydrateTabState)
  const setRequestLoaded = useWorkbenchStore(state => state.setRequestLoaded)
  const syncTabState = useWorkbenchStore(state => state.syncTabState)
  const setIsBooting = useWorkbenchStore(state => state.setIsBooting)
  const ensureTreeSelection = useWorkbenchStore(state => state.ensureTreeSelection)

  useEffect(() => {
    if (hasBootstrappedRef.current) {
      return
    }

    hasBootstrappedRef.current = true
    let cancelled = false

    async function bootstrapWorkbench() {
      setIsBooting(true)
      try {
        const startup = await openStartupProject()
        if (cancelled) {
          return
        }

        hydrateProject({
          projectPath: startup.projectPath,
          projectSnapshot: startup.projectSnapshot,
          recentProjectPaths: startup.appConfig.recentProjectPaths,
          appTheme: startup.appConfig.theme,
          appPrimaryColor: startup.appConfig.primaryColor,
        })
        setTheme(startup.appConfig.theme)

        // Restore saved tab state and load tab content
        const [savedTabs, savedActiveId] = await readTabState()
        if (savedTabs.length > 0 && startup.projectSnapshot) {
          // Filter tabs to only include requests that still exist in project
          const validTabs = savedTabs.filter(tab =>
            findApiLocation(startup.projectSnapshot, tab.requestId) !== null,
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
  }, [hydrateProject, hydrateTabState, setRequestLoaded, syncTabState, setIsBooting, setTheme])

  useEffect(() => {
    ensureTreeSelection()
  }, [ensureTreeSelection, project, selectedTreeNode])
}
