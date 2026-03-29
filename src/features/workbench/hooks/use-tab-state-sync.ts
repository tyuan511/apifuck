import { useEffect } from 'react'
import { useWorkbenchStore } from '../store/workbench-store'

export function useTabStateSync() {
  const openRequestTabs = useWorkbenchStore(state => state.openRequestTabs)
  const activeRequestId = useWorkbenchStore(state => state.activeRequestId)
  const syncTabState = useWorkbenchStore(state => state.syncTabState)
  const project = useWorkbenchStore(state => state.project)

  useEffect(() => {
    if (!project) {
      return
    }

    syncTabState()
  }, [openRequestTabs, activeRequestId, project, syncTabState])
}
