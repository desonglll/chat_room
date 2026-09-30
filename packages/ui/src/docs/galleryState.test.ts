import { expect, test } from 'bun:test'
import { panesFor, parseFragment, toFragment } from './galleryState'

const pages = ['button', 'menu', 'modal']

test('an empty fragment gives the default state: first page, split themes, factory accent', () => {
  expect(parseFragment('', pages)).toEqual({ page: 'button', theme: 'split', accent: '', wallpaper: 'none' })
})

test('a full fragment round-trips', () => {
  const state = { page: 'menu', theme: 'night', accent: 'cyan', wallpaper: 'midnight-blue' } as const
  expect(parseFragment(toFragment(state), pages)).toEqual(state)
})

test('the shortest link omits the defaults', () => {
  expect(toFragment({ page: 'button', theme: 'split', accent: '', wallpaper: 'none' })).toBe('#page=button&theme=split')
})

test('unknown values fall back instead of producing an undefined attribute', () => {
  expect(parseFragment('#page=nope&theme=sepia&accent=jade&wallpaper=lava', pages)).toEqual({
    page: 'button',
    theme: 'split',
    accent: '',
    wallpaper: 'none',
  })
})

test('split renders both themes, a single mode renders one', () => {
  expect(panesFor('split')).toEqual(['day', 'night'])
  expect(panesFor('day')).toEqual(['day'])
  expect(panesFor('night')).toEqual(['night'])
})
