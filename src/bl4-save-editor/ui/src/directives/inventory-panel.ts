import { directive } from 'gonia'
import * as api from '../api/invoke.js'
import type { InventoryItem, ItemDetail, BankInfo } from '../types/index.js'

directive('inventory-panel', function inventoryPanel(
  $element: Element,
  $scope: Record<string, unknown>,
) {
  $scope['selectedSerial'] = null as string | null
  $scope['detail'] = null as ItemDetail | null
  $scope['detailLoading'] = false
  $scope['bankItems'] = [] as InventoryItem[]
  $scope['bankCount'] = 0
  $scope['bankWarning'] = false

  const isBankMode = $element.getAttribute('data-mode') === 'bank'
  $scope['isBankMode'] = isBankMode

  if (isBankMode) {
    api.getBank().then((bank: BankInfo) => {
      $scope['bankItems'] = bank.items
      $scope['bankCount'] = bank.count
      $scope['bankWarning'] = bank.sdu_warning
    }).catch(() => {})
  }

  $scope['selectItem'] = async (serial: string) => {
    $scope['selectedSerial'] = serial
    $scope['detailLoading'] = true
    $scope['detail'] = null
    try {
      $scope['detail'] = await api.getItemDetail(serial)
    } catch {
      $scope['detail'] = null
    } finally {
      $scope['detailLoading'] = false
    }
  }

  $scope['isSelected'] = (serial: string) => {
    return $scope['selectedSerial'] === serial
  }

  $scope['itemDisplayName'] = (item: InventoryItem) => {
    return item.name || item.item_type || 'Unknown Item'
  }

  $scope['itemMeta'] = (item: InventoryItem) => {
    const parts: string[] = []
    if (item.manufacturer) parts.push(item.manufacturer)
    if (item.weapon_type) parts.push(item.weapon_type)
    if (item.elements) parts.push(item.elements)
    return parts.join(' / ') || (item.item_type || '')
  }

  $element.innerHTML = `
    <div class="items-panel">
      <div class="action-bar">
        <span class="items-count" g-text="isBankMode ? ('Bank: ' + bankCount + ' items') : ('Inventory: ' + (inventory ? inventory.length : 0) + ' items')"></span>
        <span g-show="bankWarning" class="sdu-warning">Over base capacity</span>
      </div>
      <div class="items-split">
        <div class="items-list">
          <div g-for="item in (isBankMode ? bankItems : inventory)" class="item-row"
               g-class="{ selected: isSelected(item.serial), 'decode-success': item.decode_success, 'decode-failed': !item.decode_success }"
               g-on="click: selectItem(item.serial)">
            <div class="item-info">
              <div class="item-name" g-text="itemDisplayName(item)"></div>
              <div class="item-meta" g-text="itemMeta(item)"></div>
            </div>
            <span g-show="item.level" class="item-level" g-text="'L' + item.level"></span>
            <div class="item-flags">
              <span g-show="item.is_equipped" class="badge badge-equipped">EQ</span>
              <span g-show="item.is_favorite" class="badge badge-favorite">FAV</span>
              <span g-show="item.is_junk" class="badge badge-junk">JUNK</span>
            </div>
          </div>
          <div g-show="!isBankMode && (!inventory || inventory.length === 0)" class="muted" style="text-align: center; padding: 2rem">
            No items
          </div>
          <div g-show="isBankMode && bankItems.length === 0" class="muted" style="text-align: center; padding: 2rem">
            Bank is empty
          </div>
        </div>
        <div class="item-detail-panel" g-class="{ empty: !detail && !detailLoading }">
          <div g-show="detailLoading" class="loading">Loading...</div>
          <div g-show="!detail && !detailLoading && !selectedSerial" class="muted" style="text-align: center">
            Select an item to view details
          </div>
          <div g-show="detail">
            <item-detail></item-detail>
          </div>
        </div>
      </div>
    </div>
  `
}, { scope: true })
