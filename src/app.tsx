import { WorkbenchLoadingScreen } from '@/features/workbench/components/shared'
import { WorkbenchShell } from '@/features/workbench/components/workbench-shell'
import { useTabStateSync } from '@/features/workbench/hooks/use-tab-state-sync'
import { useWorkbenchBootstrap } from '@/features/workbench/hooks/use-workbench-bootstrap'
import { useWorkbenchStore } from '@/features/workbench/store/workbench-store'

export function App() {
  useWorkbenchBootstrap()
  useTabStateSync()
  const isBooting = useWorkbenchStore(state => state.isBooting)

  if (isBooting) {
    return <WorkbenchLoadingScreen />
  }

  return <WorkbenchShell />
}
