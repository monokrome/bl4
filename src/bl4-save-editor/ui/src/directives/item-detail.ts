import { directive } from 'gonia'
import type { ItemDetail } from '../types/index.js'

directive('item-detail', function itemDetail(
  $element: Element,
  $scope: Record<string, unknown>,
) {
  $scope['detailTitle'] = () => {
    const detail = $scope['detail'] as ItemDetail | null
    if (!detail) return ''
    const parts: string[] = []
    if (detail.rarity) parts.push(detail.rarity)
    if (detail.manufacturer) parts.push(detail.manufacturer)
    if (detail.weapon_type) parts.push(detail.weapon_type)
    if (parts.length > 0) return parts.join(' ')
    return detail.item_type_name
  }

  $scope['detailSubtitle'] = () => {
    const detail = $scope['detail'] as ItemDetail | null
    if (!detail) return ''
    return detail.item_type_name
  }

  $scope['copySerial'] = () => {
    const detail = $scope['detail'] as ItemDetail | null
    if (!detail) return
    navigator.clipboard.writeText(detail.serial)
  }

  $element.innerHTML = `
    <div g-show="detail">
      <div class="detail-header">
        <div class="detail-title" g-text="detailTitle()"></div>
        <div class="detail-subtitle" g-text="detailSubtitle()"></div>
      </div>

      <div class="detail-section">
        <h3>Properties</h3>
        <div class="detail-rows">
          <div class="detail-row">
            <span class="detail-label">Type</span>
            <span class="detail-value" g-text="detail ? detail.item_type_name : ''"></span>
          </div>
          <div g-show="detail && detail.manufacturer" class="detail-row">
            <span class="detail-label">Manufacturer</span>
            <span class="detail-value" g-text="detail ? detail.manufacturer : ''"></span>
          </div>
          <div g-show="detail && detail.weapon_type" class="detail-row">
            <span class="detail-label">Weapon Type</span>
            <span class="detail-value" g-text="detail ? detail.weapon_type : ''"></span>
          </div>
          <div g-show="detail && detail.level" class="detail-row">
            <span class="detail-label">Level</span>
            <span class="detail-value" g-text="detail ? detail.level : ''"></span>
          </div>
          <div g-show="detail && detail.rarity" class="detail-row">
            <span class="detail-label">Rarity</span>
            <span class="detail-value" g-text="detail ? detail.rarity : ''"></span>
          </div>
          <div g-show="detail && detail.elements" class="detail-row">
            <span class="detail-label">Elements</span>
            <span class="detail-value" g-text="detail ? detail.elements : ''"></span>
          </div>
        </div>
      </div>

      <div g-show="detail && detail.parts && detail.parts.length > 0" class="detail-section">
        <h3>Parts</h3>
        <div class="parts-list">
          <div g-for="part in (detail ? detail.parts : [])" class="part-row">
            <span class="part-category" g-text="part.category || '\u2014'"></span>
            <span class="part-name" g-text="part.name || ('#' + part.index)"></span>
          </div>
        </div>
      </div>

      <div g-show="detail" class="detail-section">
        <h3>Serial</h3>
        <div class="serial-wrapper">
          <code class="serial" g-text="detail ? detail.serial : ''"></code>
          <button class="btn-copy" g-on="click: copySerial">copy</button>
        </div>
      </div>
    </div>
  `
}, { scope: true })
