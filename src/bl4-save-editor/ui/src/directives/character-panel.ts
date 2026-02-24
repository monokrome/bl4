import { directive } from 'gonia'
import * as api from '../api/invoke.js'
import type { CharacterInfo } from '../types/index.js'

directive('character-panel', function characterPanel(
  $element: Element,
  $scope: Record<string, unknown>,
) {
  $scope['editing'] = null as string | null
  $scope['editValue'] = ''

  const editableFields: Array<{ key: string; label: string; type: 'text' | 'number' }> = [
    { key: 'name', label: 'Name', type: 'text' },
    { key: 'cash', label: 'Cash', type: 'number' },
    { key: 'eridium', label: 'Eridium', type: 'number' },
    { key: 'xp', label: 'XP', type: 'number' },
    { key: 'specialization_xp', label: 'Specialization XP', type: 'number' },
  ]

  $scope['editableFields'] = editableFields

  $scope['startEdit'] = (key: string) => {
    const character = $scope['character'] as CharacterInfo | null
    if (!character) return
    $scope['editing'] = key
    const value = character[key as keyof CharacterInfo]
    $scope['editValue'] = value != null ? String(value) : ''
  }

  $scope['cancelEdit'] = () => {
    $scope['editing'] = null
    $scope['editValue'] = ''
  }

  $scope['saveEdit'] = async () => {
    const editing = $scope['editing'] as string | null
    if (!editing) return

    const request: Record<string, unknown> = {}
    const field = editableFields.find(f => f.key === editing)
    if (field?.type === 'number') {
      request[editing] = parseInt($scope['editValue'] as string, 10)
    } else {
      request[editing] = $scope['editValue']
    }

    try {
      await api.setCharacter(request)
      const refreshCharacter = $scope['refreshCharacter'] as (() => Promise<void>) | undefined
      if (refreshCharacter) await refreshCharacter()
    } catch (e) {
      $scope['error'] = String(e)
    }

    $scope['editing'] = null
    $scope['editValue'] = ''
  }

  $scope['getFieldValue'] = (key: string) => {
    const character = $scope['character'] as CharacterInfo | null
    if (!character) return '\u2014'
    const value = character[key as keyof CharacterInfo]
    if (value == null) return '\u2014'
    if (typeof value === 'number') return value.toLocaleString()
    return String(value)
  }

  $element.innerHTML = `
    <div class="panel">
      <div class="panel-section">
        <h3>Character Info</h3>
        <div class="field-row">
          <label>Class</label>
          <div class="display-value">
            <span g-text="character ? (character.class || '\u2014') : '\u2014'"></span>
          </div>
        </div>
        <div class="field-row">
          <label>Difficulty</label>
          <div class="display-value">
            <span g-text="character ? (character.difficulty || '\u2014') : '\u2014'"></span>
          </div>
        </div>
        <div class="field-row">
          <label>Level</label>
          <div class="display-value">
            <span g-text="character ? (character.level != null ? character.level : '\u2014') : '\u2014'"></span>
          </div>
        </div>
        <div class="field-row">
          <label>Specialization Level</label>
          <div class="display-value">
            <span g-text="character ? (character.specialization_level != null ? character.specialization_level : '\u2014') : '\u2014'"></span>
          </div>
        </div>
      </div>

      <div class="panel-section">
        <h3>Editable Fields</h3>
        <div g-for="field in editableFields" class="field-row">
          <label g-text="field.label"></label>
          <div g-show="editing !== field.key" class="display-value">
            <span g-text="getFieldValue(field.key)"></span>
            <button class="btn-icon" g-on="click: startEdit(field.key)">edit</button>
          </div>
          <div g-show="editing === field.key" class="edit-inline">
            <input g-model="editValue" g-on="keydown.enter: saveEdit" />
            <button class="btn btn-primary" g-on="click: saveEdit">ok</button>
            <button class="btn btn-ghost" g-on="click: cancelEdit">x</button>
          </div>
        </div>
      </div>
    </div>
  `
}, { scope: true })
