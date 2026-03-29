import { ThemeProvider } from 'next-themes'
import ReactDOM from 'react-dom/client'
import { App } from './app'
import { Toaster } from './components/ui/sonner'
import { TooltipProvider } from './components/ui/tooltip'
import './index.css'

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <ThemeProvider attribute="class" defaultTheme="system" enableSystem>
    <TooltipProvider>
      <App />
      <Toaster />
    </TooltipProvider>
  </ThemeProvider>,
)
