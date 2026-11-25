import { parseOptional } from './formUtils';

/** Item form fields shared by the edit drawer and the quick-create modal. */
export type ItemField =
  | 'name'
  | 'baseUnit'
  | 'mrp'
  | 'defaultSalePrice'
  | 'initialStock'
  | 'initialCost'
  | 'lowStockThreshold'
  | 'serviceCost';

/** Backend ValidationError `details.field` -> form field. Category/barcode are not in the item UI (D5). */
export const FIELD_MAP: Record<string, ItemField> = {
  name: 'name',
  base_unit: 'baseUnit',
  mrp: 'mrp',
  default_sale_price: 'defaultSalePrice',
  initial_stock: 'initialStock',
  initial_cost: 'initialCost',
  low_stock_threshold: 'lowStockThreshold',
  service_cost: 'serviceCost',
};

export type ItemFormErrors = Partial<Record<ItemField, string>>;

export interface ItemFormInput {
  name: string;
  baseUnit: string;
  mrp: string;
  price: string;
  lowStock: string;
  initialStock: string;
  initialCost: string;
  /** Service form only (FR-36). */
  serviceCost?: string;
}

export type ItemFormMode =
  /** Edit drawer: stock fields are not part of the form. */
  | 'edit'
  /** Create drawer: opening stock is optional, cost required when stock > 0. */
  | 'create'
  /** Quick create on New Sale: stock and purchase price are both required (> 0). */
  | 'quick-stock'
  /** Quick create on New Purchase: no stock fields. */
  | 'quick'
  /** Service create / edit (FR-36): no stock, bundles or low stock level; optional Service cost. */
  | 'service';

export interface ItemFormValues {
  mrp: number | null;
  price: number;
  lowStock: number;
  stock: number | null;
  cost: number | null;
  serviceCost: number | null;
}

/** Validate the item fields; returns the errors and, when clean, the parsed numbers. */
export function validateItemForm(
  input: ItemFormInput,
  mode: ItemFormMode
): { errors: ItemFormErrors; values: ItemFormValues | null } {
  const errs: ItemFormErrors = {};
  const mrpVal = parseOptional(input.mrp);
  const priceVal = parseOptional(input.price);
  const stockVal = parseOptional(input.initialStock);
  const costVal = parseOptional(input.initialCost);
  const lowVal = parseOptional(input.lowStock);

  if (!input.name.trim()) errs.name = 'Item name is required';
  if (!input.baseUnit.trim()) errs.baseUnit = 'Base unit is required';
  if (priceVal === null || !Number.isFinite(priceVal) || priceVal < 0) {
    errs.defaultSalePrice = 'Enter a sale price (0 or more)';
  }
  if (mode !== 'service' && (lowVal === null || !Number.isFinite(lowVal) || lowVal < 0)) {
    errs.lowStockThreshold = 'Low stock level must be 0 or more';
  }
  const svcCostVal = parseOptional(input.serviceCost ?? '');
  if (mode === 'service' && svcCostVal !== null && (!Number.isFinite(svcCostVal) || svcCostVal < 0)) {
    errs.serviceCost = 'Service cost must be 0 or more (leave empty for none)';
  }
  if (mrpVal !== null && (!Number.isFinite(mrpVal) || mrpVal <= 0)) {
    errs.mrp = 'MRP must be greater than 0 (leave empty for none)';
  }
  if (mode === 'create') {
    if (stockVal !== null && (!Number.isFinite(stockVal) || stockVal < 0)) {
      errs.initialStock = 'Initial stock cannot be negative';
    }
    if (costVal !== null && (!Number.isFinite(costVal) || costVal < 0)) {
      errs.initialCost = 'Cost cannot be negative';
    }
    if (stockVal !== null && stockVal > 0 && costVal === null) {
      errs.initialCost = 'Enter the cost of the opening stock';
    }
  } else if (mode === 'quick-stock') {
    if (stockVal === null || !Number.isFinite(stockVal) || stockVal <= 0) {
      errs.initialStock = 'Stock on hand must be greater than 0';
    }
    if (costVal === null || !Number.isFinite(costVal) || costVal <= 0) {
      errs.initialCost = 'Purchase price must be greater than 0';
    }
  }
  if (Object.keys(errs).length > 0) return { errors: errs, values: null };
  return {
    errors: errs,
    values: {
      mrp: mrpVal,
      price: priceVal as number,
      lowStock: (lowVal ?? 5) as number,
      stock: stockVal,
      cost: costVal,
      serviceCost: svcCostVal,
    },
  };
}
