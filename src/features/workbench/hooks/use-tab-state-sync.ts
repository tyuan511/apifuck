import { useEffect } from 'react'
import { useWorkbenchStore } from '../store/workbench-store'

export function useTabStateSync() {
  const openRequestTabs = useWorkbenchStore(state => state.openRequestTabs)
  const activeRequestId = useWorkbenchStore(state => state.activeRequestId)
  const syncTabState = useWorkbenchStore(state => state.syncTabState)
  const workspace = useWorkbenchStore(state => state.workspace)

  useEffect(() => {
    if (!workspace) {
      return
    }

    syncTabState()
  }, [openRequestTabs, activeRequestId, syncTabState, workspace])
}
