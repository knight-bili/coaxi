import { describe, expect, it } from 'vitest'
import { extractDisplayCommandFromToolInput } from './ordinary-tool-call'

describe('native command display quoting', () => {
  it('unwraps a JSON-carried bash command without decoding Windows path backslashes', () => {
    const shellPayload = String.raw`\"C:\new folder\tool.exe\" --message \"hello\"`
    const wrapped = `/bin/bash -lc "${shellPayload}"`
    const input = JSON.stringify({ command: wrapped })

    expect(extractDisplayCommandFromToolInput(input)).toBe(String.raw`"C:\new folder\tool.exe" --message "hello"`)
  })

  it.each([
    String.raw`cmd /c "C:\new folder\tool.exe"`,
    String.raw`powershell -NoProfile -Command "& 'C:\new\tool.exe'"`
  ])('leaves Windows command text unchanged: %s', command => {
    expect(extractDisplayCommandFromToolInput(JSON.stringify({ command }))).toBe(command)
  })
})
