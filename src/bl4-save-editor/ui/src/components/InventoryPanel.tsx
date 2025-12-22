import { InventoryItem } from '../api';

interface Props {
  items: InventoryItem[];
}

export function InventoryPanel({ items }: Props) {
  if (items.length === 0) {
    return (
      <div className="panel inventory-panel">
        <h2>Inventory</h2>
        <p className="empty-state">No items in backpack</p>
      </div>
    );
  }

  return (
    <div className="panel inventory-panel">
      <h2>Inventory ({items.length} items)</h2>

      <div className="inventory-grid">
        {items.map((item) => (
          <div
            key={item.slot}
            className={`inventory-item ${item.is_favorite ? 'favorite' : ''} ${item.is_junk ? 'junk' : ''}`}
          >
            <div className="item-header">
              <span className="slot">Slot {item.slot}</span>
              <div className="badges">
                {item.is_favorite && <span className="badge favorite">★</span>}
                {item.is_junk && <span className="badge junk">J</span>}
                {item.is_equipped && <span className="badge equipped">E</span>}
              </div>
            </div>
            <div className="item-serial" title={item.serial}>
              {item.serial.length > 30
                ? `${item.serial.substring(0, 30)}...`
                : item.serial}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
