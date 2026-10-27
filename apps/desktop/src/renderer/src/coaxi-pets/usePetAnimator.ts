// Apache-2.0; spacering-net/codeg aff1e6c278453ed8d0b96a5f664edcdca621aa3f.
"use client"

import { useEffect, useRef, useState } from "react"
import {
  IDLE_FLOURISH_MAX_MS,
  IDLE_FLOURISH_MIN_MS,
  IDLE_FLOURISH_OPTIONS,
  PET_FRAME_DURATIONS_MS,
  PET_STATE_ROW,
  type PetState,
} from "./animation"

export interface AnimatorTick {
  row: number
  col: number
}

/**
 * Drives the (row, col) cell of the spritesheet for the given state. Each
 * state runs as a chained `setTimeout` loop using its per-frame durations
 * — cheaper than `requestAnimationFrame` accumulators and naturally throttles
 * when the window is minimized (browsers throttle background timers).
 *
 * When the input state is `"idle"`, the hook randomly inserts a one-shot
 * flourish animation (`waving` or `jumping`) every 8–15s, then returns to
 * idle. This keeps the pet feeling alive without spamming the CPU.
 */
export function usePetAnimator(state: PetState): AnimatorTick {
  const [tick, setTick] = useState<AnimatorTick>(() => ({
    row: PET_STATE_ROW[state],
    col: 0,
  }))

  // Generation counter so an in-flight setTimeout from a previous state
  // never wins a race against a new state's first frame.
  const genRef = useRef(0)
  const [reducedMotion,setReducedMotion]=useState(()=>window.matchMedia('(prefers-reduced-motion: reduce)').matches)
  const [visible,setVisible]=useState(()=>document.visibilityState!=='hidden')
  useEffect(()=>{const media=window.matchMedia('(prefers-reduced-motion: reduce)'),motion=():void=>setReducedMotion(media.matches),visibility=():void=>setVisible(document.visibilityState!=='hidden');media.addEventListener('change',motion);document.addEventListener('visibilitychange',visibility);return()=>{media.removeEventListener('change',motion);document.removeEventListener('visibilitychange',visibility)}},[])


  useEffect(() => {
    const gen = ++genRef.current
    let timer: ReturnType<typeof setTimeout> | null = null
    let flourishTimer: ReturnType<typeof setTimeout> | null = null
    let activeState: PetState = state

    const setRow = (s: PetState, col: number) => {
      setTick({ row: PET_STATE_ROW[s], col })
    }

    const playFrame = (s: PetState, col: number, onFinish?: () => void) => {
      const durations = PET_FRAME_DURATIONS_MS[s]
      if (durations.length === 0) return
      setRow(s, col)
      const dur = durations[col] ?? durations[durations.length - 1]
      timer = setTimeout(() => {
        if (gen !== genRef.current) return
        const next = col + 1
        if (next >= durations.length) {
          if (onFinish) onFinish()
          else playFrame(s, 0)
        } else {
          playFrame(s, next, onFinish)
        }
      }, dur)
    }

    const startFlourishLoop = () => {
      const delay =
        IDLE_FLOURISH_MIN_MS +
        Math.floor(
          Math.random() * (IDLE_FLOURISH_MAX_MS - IDLE_FLOURISH_MIN_MS)
        )
      flourishTimer = setTimeout(() => {
        if (gen !== genRef.current) return
        if (activeState !== "idle") return
        const opts = IDLE_FLOURISH_OPTIONS
        const pick = opts[Math.floor(Math.random() * opts.length)]
        if (timer) {
          clearTimeout(timer)
          timer = null
        }
        playFrame(pick, 0, () => {
          if (gen !== genRef.current) return
          if (activeState !== "idle") return
          // Resume idle loop and queue the next flourish.
          playFrame("idle", 0)
          startFlourishLoop()
        })
      }, delay)
    }

    if(reducedMotion||!visible){setRow(state,0);return}
    activeState = state
    playFrame(state, 0)
    if (state === "idle") {
      startFlourishLoop()
    }

    return () => {
      if (timer) clearTimeout(timer)
      if (flourishTimer) clearTimeout(flourishTimer)
    }
  }, [state,reducedMotion,visible])

  return tick
}
