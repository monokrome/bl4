import { directive, createContextKey, registerContext } from 'gonia'
import * as api from '../api/invoke.js'
import { loadThemes, applyTheme, getSavedThemeId, saveThemeId } from '../themes/index.js'
import { navigate, onRouteChange, getRouteState } from '../router.js'
import type { SaveInfo, CharacterInfo, InventoryItem } from '../types/index.js'

export const ThemeKey = createContextKey<string>('theme')

directive('save-editor', function saveEditor(
  $element: Element,
  $scope: Record<string, unknown>,
) {
  const savedThemeId = getSavedThemeId()
  const initial = getRouteState()

  $scope['saveInfo'] = null as SaveInfo | null
  $scope['character'] = null as CharacterInfo | null
  $scope['inventory'] = [] as InventoryItem[]
  $scope['loading'] = false
  $scope['error'] = null as string | null
  $scope['routeView'] = initial.view
  $scope['tabs'] = [
    { id: 'character', label: 'Character', path: '/editor/character' },
    { id: 'inventory', label: 'Inventory', path: '/editor/inventory' },
    { id: 'bank', label: 'Bank', path: '/editor/bank' },
  ]
  $scope['themes'] = [] as Array<{ id: string; name: string; css: string }>
  $scope['activeTheme'] = savedThemeId

  registerContext($element, ThemeKey, savedThemeId)

  onRouteChange((view) => {
    $scope['routeView'] = view
  })

  const nav = (path: string) => navigate(path)
  $scope['navigate'] = nav

  loadThemes().then(themes => {
    $scope['themes'] = themes
    const current = themes.find(t => t.id === savedThemeId)
    applyTheme(current || null)
  })

  $scope['setTheme'] = (id: string) => {
    $scope['activeTheme'] = id
    saveThemeId(id)
    registerContext($element, ThemeKey, id)
    const themes = $scope['themes'] as Array<{ id: string; name: string; css: string }>
    const theme = themes.find(t => t.id === id)
    applyTheme(theme || null)
  }

  $scope['clearError'] = () => {
    $scope['error'] = null
  }

  $scope['handleSave'] = async () => {
    $scope['loading'] = true
    $scope['error'] = null
    try {
      await api.saveChanges()
      $scope['saveInfo'] = await api.getSaveInfo()
    } catch (e) {
      $scope['error'] = String(e)
    } finally {
      $scope['loading'] = false
    }
  }

  $scope['handleClose'] = () => {
    $scope['saveInfo'] = null
    $scope['character'] = null
    $scope['inventory'] = []
    navigate('/')
  }

  $scope['loadSave'] = async (path: string, steamId: string) => {
    $scope['loading'] = true
    $scope['error'] = null
    try {
      $scope['saveInfo'] = await api.openSave(path, steamId)
      $scope['character'] = await api.getCharacter()
      $scope['inventory'] = await api.getInventory()
      navigate('/editor/character')
    } catch (e) {
      $scope['error'] = String(e)
    } finally {
      $scope['loading'] = false
    }
  }

  $scope['refreshCharacter'] = async () => {
    try {
      $scope['character'] = await api.getCharacter()
      $scope['saveInfo'] = await api.getSaveInfo()
    } catch (e) {
      $scope['error'] = String(e)
    }
  }

  $scope['refreshInventory'] = async () => {
    try {
      $scope['inventory'] = await api.getInventory()
    } catch (e) {
      $scope['error'] = String(e)
    }
  }

  if (initial.view !== 'selector' && !$scope['saveInfo']) {
    navigate('/')
  }
}, { scope: true })
