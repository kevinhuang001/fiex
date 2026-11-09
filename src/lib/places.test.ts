import { describe, expect, it } from 'vitest'
import { setPlaceHidden } from './places'

describe('common place visibility', () => {
  it('hides a place once and can show it again', () => {
    expect(setPlaceHidden([], 'recent://', true)).toEqual(['recent://'])
    expect(setPlaceHidden(['recent://'], 'recent://', true)).toEqual(['recent://'])
    expect(setPlaceHidden(['recent://', 'trash://'], 'recent://', false)).toEqual(['trash://'])
  })
})
