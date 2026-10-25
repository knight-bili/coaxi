import { resolve } from 'node:path'
import react from '@vitejs/plugin-react'
import { defineConfig, externalizeDepsPlugin } from 'electron-vite'

const root = resolve(import.meta.dirname)

export default defineConfig({
  main: {
    plugins: [externalizeDepsPlugin()],
    build: {
      rollupOptions: {
        input: resolve(root, 'apps/desktop/src/main/index.ts')
      }
    },
    resolve: {
      alias: {
        '@contracts': resolve(root, 'packages/contracts/src/index.ts')
      }
    }
  },
  preload: {
    plugins: [externalizeDepsPlugin()],
    build: {
      rollupOptions: {
        input: {'project-launcher':resolve(root,'apps/desktop/src/preload/project-launcher.ts'),index:resolve(root, 'apps/desktop/src/preload/index.ts'),pet:resolve(root,'apps/desktop/src/preload/pet.ts')},
        output: {
          format: 'cjs',
          entryFileNames: '[name].js'
        }
      }
    },
    resolve: {
      alias: {
        '@contracts': resolve(root, 'packages/contracts/src/index.ts')
      }
    }
  },
  renderer: {
    root: resolve(root, 'apps/desktop/src/renderer'),
    plugins: [react()],
    build: {
      rollupOptions: {
        input: {projectLauncher:resolve(root,'apps/desktop/src/renderer/project-launcher.html'),index:resolve(root, 'apps/desktop/src/renderer/index.html'),pet:resolve(root,'apps/desktop/src/renderer/pet.html')}
      }
    },
    resolve: {
      alias: {
        '@contracts': resolve(root, 'packages/contracts/src/index.ts')
      }
    }
  }
})
