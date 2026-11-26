import { fileURLToPath } from 'node:url'
import { defineConfig, searchForWorkspaceRoot } from 'vite'
import react from '@vitejs/plugin-react'

// https://vitejs.dev/config/
// restarted for tailwind config: 1
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    // Offline Help imports selected, versioned feature guides at build time.
    fs: { allow: [searchForWorkspaceRoot(process.cwd()), fileURLToPath(new URL('../../../docs/features', import.meta.url))] },
  },
  build: {
    outDir: 'dist',
    emptyOutDir: true,
  }
})
