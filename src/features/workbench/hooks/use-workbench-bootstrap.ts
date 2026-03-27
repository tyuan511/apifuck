import { useTheme } from 'next-themes'
import { useEffect, useRef } from 'react'
import { toast } from 'sonner'
import { openStartupWorkspace } from '@/lib/app-config'
import { useWorkbenchStore } from '../store/workbench-store'
import { clamp } from '../utils'

export function useWorkbenchBootstrap() {
  const { setTheme } = useTheme()
  const splitContainerRef = useRef<HTMLDivElement | null>(null)

  const isDraggingSplit = useWorkbenchStore(state => state.isDraggingSplit)
  const activeProjectId = useWorkbenchStore(state => state.activeProjectId)
  const selectedTreeNode = useWorkbenchStore(state => state.selectedTreeNode)
  const workspace = useWorkbenchStore(state => state.workspace)
  const hydrateWorkspace = useWorkbenchStore(state => state.hydrateWorkspace)
  const setIsBooting = useWorkbenchStore(state => state.setIsBooting)
  const setSplitRatio = useWorkbenchStore(state => state.setSplitRatio)
  const setIsDraggingSplit = useWorkbenchStore(state => state.setIsDraggingSplit)
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
  }, [hydrateWorkspace, setIsBooting, setTheme])

  useEffect(() => {
    if (!isDraggingSplit) {
      return
    }

    function handlePointerMove(event: MouseEvent) {
      const bounds = splitContainerRef.current?.getBoundingClientRect()
      if (!bounds) {
        return
      }

      setSplitRatio(clamp((event.clientY - bounds.top) / bounds.height, 0.28, 0.78))
    }

    function stopDragging() {
      setIsDraggingSplit(false)
    }

    window.addEventListener('mousemove', handlePointerMove)
    window.addEventListener('mouseup', stopDragging)

    return () => {
      window.removeEventListener('mousemove', handlePointerMove)
      window.removeEventListener('mouseup', stopDragging)
    }
  }, [isDraggingSplit, setIsDraggingSplit, setSplitRatio])

  useEffect(() => {
    ensureActiveProject()
  }, [activeProjectId, ensureActiveProject, workspace])

  useEffect(() => {
    ensureTreeSelection()
  }, [activeProjectId, ensureTreeSelection, selectedTreeNode, workspace])

  return { splitContainerRef }
}
