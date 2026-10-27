import {resolve} from 'node:path'
import {defineConfig} from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins:[react()],base:'./',
  resolve:{alias:{'@contracts':resolve(import.meta.dirname,'packages/contracts/src/index.ts')}},
  build:{outDir:resolve(import.meta.dirname,'out/mobile-cards'),emptyOutDir:true,assetsInlineLimit:65536,
    lib:{entry:resolve(import.meta.dirname,'apps/desktop/src/renderer/src/mobile-card-page.tsx'),name:'CoaxiCardBundle',formats:['iife'],fileName:()=>'mobile-card.js',cssFileName:'mobile-card'},
    rollupOptions:{output:{inlineDynamicImports:true}},
  },
})
