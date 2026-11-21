import React, { useState, useEffect } from 'react';
import {
  Package,
  Plus,
  Search,
  Filter,
  SlidersHorizontal,
  RefreshCw,
} from 'lucide-react';
import { api } from '@/api';
import type { Item } from '@/types';
import { StockStatusBadge } from '@/components/common/StockStatusBadge';
import { CurrencyDisplay } from '@/components/common/CurrencyDisplay';
import { BundleCreationModal } from '@/components/domain/BundleCreationModal';
import { StockAdjustmentModal } from '@/components/domain/StockAdjustmentModal';
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from '@/components/ui/Table';
import { Button } from '@/components/ui/Button';
import { Modal } from '@/components/ui/Modal';
import { TextInput } from '@/components/ui/TextInput';
import { NumericInput } from '@/components/ui/NumericInput';
import { Select } from '@/components/ui/Select';

type InventoryRow = Pick<
  Item,
  'id' | 'name' | 'category' | 'barcode' | 'baseUnit' | 'mrp' | 'defaultSalePrice' | 'currentStock'
>;

export const InventoryPage: React.FC = () => {
  const [items, setItems] = useState<InventoryRow[]>([]);
  const [searchQuery, setSearchQuery] = useState('');
  const [selectedCategory, setSelectedCategory] = useState<string>('ALL');
  const [lowStockOnly, setLowStockOnly] = useState(false);
  const [isLoading, setIsLoading] = useState(true);

  // Modals state
  const [isAddModalOpen, setIsAddModalOpen] = useState(false);
  const [isBundleModalOpen, setIsBundleModalOpen] = useState(false);
  const [isAdjustModalOpen, setIsAdjustModalOpen] = useState(false);
  const [selectedItemForAction, setSelectedItemForAction] = useState<InventoryRow | null>(null);

  // Add Item Form State
  const [newItemName, setNewItemName] = useState('');
  const [newItemCategory, setNewItemCategory] = useState('Notebooks');
  const [newItemBarcode, setNewItemBarcode] = useState('');
  const [newItemBaseUnit, setNewItemBaseUnit] = useState('PCS');
  const [newItemCost, setNewItemCost] = useState(0);
  const [newItemPrice, setNewItemPrice] = useState(0);
  const [newItemMrp, setNewItemMrp] = useState(0);
  const [newItemStock, setNewItemStock] = useState(0);
  const [isSaving, setIsSaving] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);

  const fetchItems = () => {
    setIsLoading(true);
    if (searchQuery.trim().length >= 2) {
      api
        .searchItemsFts(searchQuery.trim(), 50)
        .then((res) => {
          setItems(res.map((r) => ({ ...r, id: r.itemId })));
        })
        .catch(() => setItems([]))
        .finally(() => setIsLoading(false));
    } else {
      api
        .getItemsCatalog({
          category: selectedCategory === 'ALL' ? undefined : selectedCategory,
          lowStockOnly,
          limit: 100,
        })
        .then((res) => setItems(res.items))
        .catch(() => setItems([]))
        .finally(() => setIsLoading(false));
    }
  };

  useEffect(() => {
    fetchItems();
  }, [selectedCategory, lowStockOnly]);

  const handleSearchSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    fetchItems();
  };

  const handleCreateItem = async () => {
    if (!newItemName.trim()) {
      setFormError('Item name is required');
      return;
    }

    setIsSaving(true);
    setFormError(null);

    const payload = {
      name: newItemName.trim(),
      category: newItemCategory,
      barcode: newItemBarcode.trim() || null,
      baseUnit: newItemBaseUnit,
      initialCost: newItemCost,
      defaultSalePrice: newItemPrice,
      mrp: newItemMrp,
      initialStock: newItemStock,
      bundles: null,
    };

    try {
      await api.createItem(payload);
      setIsAddModalOpen(false);
      setNewItemName('');
      setNewItemCost(0);
      setNewItemPrice(0);
      setNewItemMrp(0);
      setNewItemStock(0);
      fetchItems();
    } catch (err) {
      setFormError(err instanceof Error ? err.message : 'Failed to create item');
    } finally {
      setIsSaving(false);
    }
  };

  const filteredItems = items.filter((it) => {
    const stock = it.currentStock;
    if (lowStockOnly && stock >= 5) return false;
    return true;
  });

  return (
    <div className="flex flex-col gap-4">
      {/* Top Filter & Toolbar */}
      <div className="p-3.5 rounded-lg bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 shadow-xs flex flex-wrap items-center justify-between gap-3">
        <form onSubmit={handleSearchSubmit} className="flex items-center gap-2 flex-1 max-w-md">
          <div className="relative flex-1">
            <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 h-4 w-4 text-slate-400" />
            <input
              type="text"
              placeholder="Search items by name, barcode, category (F4)..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="w-full text-xs pl-8 pr-3 py-1.5 rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 focus:outline-none focus:ring-2 focus:ring-blue-500"
            />
          </div>
          <Button type="submit" variant="secondary" size="sm">
            Search
          </Button>
        </form>

        <div className="flex items-center gap-2.5">
          {/* Category Filter */}
          <div className="flex items-center gap-1.5 text-xs text-slate-500">
            <Filter className="h-3.5 w-3.5" />
            <select
              value={selectedCategory}
              onChange={(e) => setSelectedCategory(e.target.value)}
              className="px-2 py-1 rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-xs text-slate-800 dark:text-slate-200 focus:outline-none"
            >
              <option value="ALL">All Categories</option>
              <option value="Notebooks">Notebooks</option>
              <option value="Pens & Pencils">Pens & Pencils</option>
              <option value="Adhesives">Adhesives</option>
              <option value="Files & Folders">Files & Folders</option>
              <option value="Paper">Paper</option>
              <option value="Geometry & Drawing">Geometry & Drawing</option>
            </select>
          </div>

          {/* Low Stock Toggle */}
          <label className="flex items-center gap-1.5 text-xs font-medium text-slate-600 dark:text-slate-300 cursor-pointer select-none">
            <input
              type="checkbox"
              checked={lowStockOnly}
              onChange={(e) => setLowStockOnly(e.target.checked)}
              className="rounded text-amber-600 focus:ring-amber-500"
            />
            <span>Low Stock Only (&lt; 5)</span>
          </label>

          <Button
            variant="outline"
            size="sm"
            onClick={fetchItems}
            title="Refresh Catalog"
            leftIcon={<RefreshCw className="h-3.5 w-3.5" />}
          >
            Refresh
          </Button>

          <Button
            variant="primary"
            size="sm"
            onClick={() => setIsAddModalOpen(true)}
            leftIcon={<Plus className="h-4 w-4" />}
          >
            Add Item
          </Button>
        </div>
      </div>

      {/* Catalog Table */}
      <div className="rounded-lg border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 overflow-hidden shadow-xs flex-1">
        <Table dense>
          <TableHeader>
            <TableRow>
              <TableHead className="w-12 text-center">#</TableHead>
              <TableHead>Barcode</TableHead>
              <TableHead>Item Name</TableHead>
              <TableHead>Category</TableHead>
              <TableHead className="text-center">Stock Health</TableHead>
              <TableHead align="right">Sale Price</TableHead>
              <TableHead align="right">MRP</TableHead>
              <TableHead className="text-center w-28">Actions</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {isLoading ? (
              <TableRow>
                <TableCell colSpan={8} className="py-12 text-center text-slate-400 text-xs">
                  Loading item catalog...
                </TableCell>
              </TableRow>
            ) : filteredItems.length === 0 ? (
              <TableRow>
                <TableCell colSpan={8} className="py-12 text-center text-slate-400 text-xs">
                  No inventory items match current filter criteria.
                </TableCell>
              </TableRow>
            ) : (
              filteredItems.map((item, idx) => {
                const stock = item.currentStock;
                const unit = item.baseUnit;
                const salePrice = item.defaultSalePrice;
                const mrp = item.mrp ?? 0;

                return (
                  <TableRow key={item.id ?? idx}>
                    <TableCell className="text-center text-slate-400 select-none">
                      {idx + 1}
                    </TableCell>
                    <TableCell mono className="text-xs text-slate-500">
                      {item.barcode || '—'}
                    </TableCell>
                    <TableCell className="font-semibold text-slate-900 dark:text-slate-100">
                      {item.name}
                    </TableCell>
                    <TableCell className="text-slate-500 text-xs">
                      {item.category || 'General'}
                    </TableCell>
                    <TableCell className="text-center">
                      <StockStatusBadge currentStock={stock} baseUnit={unit} />
                    </TableCell>
                    <TableCell align="right" mono className="font-bold text-slate-900 dark:text-slate-100">
                      <CurrencyDisplay amount={salePrice} size="sm" />
                    </TableCell>
                    <TableCell align="right" mono className="text-slate-400 text-xs">
                      {mrp > 0 ? <CurrencyDisplay amount={mrp} size="sm" /> : '—'}
                    </TableCell>
                    <TableCell className="text-center">
                      <div className="flex items-center justify-center gap-1.5">
                        <button
                          type="button"
                          onClick={() => {
                            setSelectedItemForAction(item);
                            setIsAdjustModalOpen(true);
                          }}
                          title="Adjust Stock / Scrap"
                          className="p-1 rounded text-slate-500 hover:text-amber-600 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors cursor-pointer"
                        >
                          <SlidersHorizontal className="h-3.5 w-3.5" />
                        </button>
                        <button
                          type="button"
                          onClick={() => {
                            setSelectedItemForAction(item);
                            setIsBundleModalOpen(true);
                          }}
                          title="Add Packaging Bundle (Alt+B)"
                          className="p-1 rounded text-slate-500 hover:text-blue-600 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors cursor-pointer"
                        >
                          <Package className="h-3.5 w-3.5" />
                        </button>
                      </div>
                    </TableCell>
                  </TableRow>
                );
              })
            )}
          </TableBody>
        </Table>
      </div>

      {/* Add Item Modal */}
      <Modal
        isOpen={isAddModalOpen}
        onClose={() => setIsAddModalOpen(false)}
        title="Add New Stationery Catalog Item"
        size="md"
        footer={
          <>
            <Button variant="outline" size="sm" onClick={() => setIsAddModalOpen(false)}>
              Cancel
            </Button>
            <Button variant="primary" size="sm" onClick={handleCreateItem} isLoading={isSaving}>
              Save Item
            </Button>
          </>
        }
      >
        <div className="flex flex-col gap-3 py-1">
          <TextInput
            label="Item Name *"
            placeholder="e.g. Doms Groove Graphite Pencil"
            value={newItemName}
            onChange={(e) => setNewItemName(e.target.value)}
            autoFocus
          />

          <div className="grid grid-cols-2 gap-3">
            <Select
              label="Category"
              value={newItemCategory}
              onChange={(val) => setNewItemCategory(String(val))}
              options={[
                { value: 'Notebooks', label: 'Notebooks' },
                { value: 'Pens & Pencils', label: 'Pens & Pencils' },
                { value: 'Adhesives', label: 'Adhesives' },
                { value: 'Files & Folders', label: 'Files & Folders' },
                { value: 'Paper', label: 'Paper' },
                { value: 'Geometry & Drawing', label: 'Geometry & Drawing' },
              ]}
            />

            <TextInput
              label="Barcode / SKU"
              placeholder="Scan or enter barcode"
              value={newItemBarcode}
              onChange={(e) => setNewItemBarcode(e.target.value)}
            />
          </div>

          <div className="grid grid-cols-2 gap-3">
            <TextInput
              label="Base Stock Unit"
              placeholder="e.g. PCS, KG, REAM"
              value={newItemBaseUnit}
              onChange={(e) => setNewItemBaseUnit(e.target.value.toUpperCase())}
            />

            <NumericInput
              label="Initial Stock Quantity"
              value={newItemStock}
              onChange={setNewItemStock}
              min={0}
              allowDecimals={false}
            />
          </div>

          <div className="grid grid-cols-3 gap-3">
            <NumericInput
              label="Cost Price (₹)"
              value={newItemCost}
              onChange={setNewItemCost}
              min={0}
            />

            <NumericInput
              label="Sale Price (₹)"
              value={newItemPrice}
              onChange={setNewItemPrice}
              min={0}
            />

            <NumericInput
              label="MRP (₹)"
              value={newItemMrp}
              onChange={setNewItemMrp}
              min={0}
            />
          </div>

          {formError && (
            <p role="alert" className="text-xs text-rose-600 font-medium">
              {formError}
            </p>
          )}
        </div>
      </Modal>

      {/* Stock Adjustment Modal */}
      {selectedItemForAction && (
        <StockAdjustmentModal
          isOpen={isAdjustModalOpen}
          onClose={() => setIsAdjustModalOpen(false)}
          itemId={selectedItemForAction.id}
          itemName={selectedItemForAction.name}
          currentStock={selectedItemForAction.currentStock}
          baseUnit={selectedItemForAction.baseUnit}
          onAdjusted={fetchItems}
        />
      )}

      {/* Bundle Creation Modal */}
      {selectedItemForAction && (
        <BundleCreationModal
          isOpen={isBundleModalOpen}
          onClose={() => setIsBundleModalOpen(false)}
          initialItemId={selectedItemForAction.id}
          baseUnit={selectedItemForAction.baseUnit}
          onBundleCreated={fetchItems}
        />
      )}
    </div>
  );
};

export default InventoryPage;
