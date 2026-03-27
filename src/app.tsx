import { useCallback } from 'react'
import { toast } from 'sonner'
import { Button } from './components/ui/button'

export function App() {
  const onClick = useCallback(() => {
    toast.success('Hello world!')
  }, [])

  return (
    <main className="h-screen flex items-center justify-center">
      <Button onClick={onClick}>Click me</Button>
    </main>
  )
}
