// Apache-2.0; spacering-net/codeg aff1e6c278453ed8d0b96a5f664edcdca621aa3f.
"use client"

import { useEffect,useMemo } from "react"
import {
  backgroundPositionFor,
  spriteBackgroundSize,
  spriteRowsFromHeight,
  type PetState,
} from "./animation"
import { useImageNaturalSize } from "./use-image-natural-size"
import { usePetAnimator } from "./usePetAnimator"

export interface PetSpriteProps {
  spritesheetUrl: string
  state: PetState
  scale: number
  /** Aria-label for screen readers. */
  onFrame?: (row:number,col:number)=>void
  label: string
}

const FRAME_WIDTH = 192
const FRAME_HEIGHT = 208

export function PetSprite({
  spritesheetUrl,
  state,
  scale,
  label,
  onFrame,
}: PetSpriteProps) {
  const { row, col } = usePetAnimator(state)
  useEffect(()=>{onFrame?.(row,col)},[row,col,onFrame])
  const backgroundImage = useMemo(
    () => `url("${spritesheetUrl}")`,
    [spritesheetUrl]
  )
  // Derive the row count from the actual sheet so v2 (11-row) pets don't render
  // squished against the legacy 9-row assumption.
  const size = useImageNaturalSize(spritesheetUrl)
  const rows = spriteRowsFromHeight(size?.height)

  return (
    <div
      role="img"
      aria-label={label}
      style={{
        width: `${FRAME_WIDTH * scale}px`,
        height: `${FRAME_HEIGHT * scale}px`,
        backgroundImage,
        backgroundRepeat: "no-repeat",
        backgroundSize: spriteBackgroundSize(rows),
        backgroundPosition: backgroundPositionFor(row, col, rows),
        imageRendering: "pixelated",
      }}
    />
  )
}
