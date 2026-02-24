import { createRouter } from 'navigonia'

export const router = createRouter({
  '/': { view: 'selector' },
  '/editor/character': { view: 'character' },
  '/editor/inventory': { view: 'inventory' },
  '/editor/bank': { view: 'bank' },
})

export type RouteState = { view: string; path: string }

const isTauri = '__TAURI_INTERNALS__' in window

let currentView = 'selector'
let currentPath = '/'
const listeners: Array<(view: string, path: string) => void> = []

export function onRouteChange(fn: (view: string, path: string) => void) {
  listeners.push(fn)
  return () => {
    const idx = listeners.indexOf(fn)
    if (idx >= 0) listeners.splice(idx, 1)
  }
}

function notify(view: string, path: string) {
  currentView = view
  currentPath = path
  for (const fn of listeners) fn(view, path)
}

export function navigate(path: string) {
  const match = router.match(path)
  if (!match) return
  notify(match.route.view, match.pathname)
  if (!isTauri) {
    history.pushState(null, '', match.pathname)
  }
}

export function getRouteState(): RouteState {
  return { view: currentView, path: currentPath }
}

if (!isTauri) {
  const initial = router.match(location.pathname)
  if (initial) {
    currentView = initial.route.view
    currentPath = initial.pathname
  }
  window.addEventListener('popstate', () => {
    const match = router.match(location.pathname)
    if (match) {
      notify(match.route.view, match.pathname)
    }
  })
}
