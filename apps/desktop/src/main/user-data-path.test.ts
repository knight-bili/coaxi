import { describe, expect, it } from 'vitest'
import { join } from 'node:path'
import { legacyUserDataPath } from './user-data-path'

describe('legacyUserDataPath', () => {
  it('keeps the existing Coaxi data directory after the display name changes to Coaxi AI', () => {
    const existing = new Set([
      join('/application-support', 'Coaxi AI'),
      join('/application-support', 'Coaxi')
    ])
    expect(
      legacyUserDataPath(
        '/application-support',
        'Coaxi AI',
        false,
        (path) => existing.has(path)
      )
    ).toBe(join('/application-support', 'Coaxi'))
  })

  it('prefers the most recent existing Horizonward userData directory', () => {
    const existing = new Set([
      join('/application-support', 'Horizonward'),
      join('/application-support', 'Lumen AI')
    ])
    expect(
      legacyUserDataPath(
        '/application-support',
        'Coaxi',
        false,
        (path) => existing.has(path)
      )
    ).toBe(join('/application-support', 'Horizonward'))
  })

  it('keeps an existing Lumen userData directory when Coaxi has none', () => {
    const existing = new Set([join('/application-support', 'Lumen AI')])
    expect(
      legacyUserDataPath(
        '/application-support',
        'Coaxi',
        false,
        (path) => existing.has(path)
      )
    ).toBe(join('/application-support', 'Lumen AI'))
  })

  it('prefers the Coaxi directory once it exists', () => {
    const existing = new Set([
      join('/application-support', 'Coaxi'),
      join('/application-support', 'Horizonward'),
      join('/application-support', 'Lumen AI')
    ])
    expect(
      legacyUserDataPath(
        '/application-support',
        'Coaxi',
        false,
        (path) => existing.has(path)
      )
    ).toBeNull()
  })

  it('does not override an explicit userData directory', () => {
    expect(
      legacyUserDataPath('/application-support', 'Coaxi', true, () => true)
    ).toBeNull()
  })
})
