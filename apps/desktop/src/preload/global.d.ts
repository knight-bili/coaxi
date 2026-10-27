import type { CoaxiApi } from '@contracts'

declare global {
  interface Window {
    coaxi: CoaxiApi
  }
}

export {}
