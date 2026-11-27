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
  /** Create drawer: opening stock is optional, cost required when stock is entered. */
  | 'create'
  /** Quick create on New Sale: stock and purchase price are both required. */
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

  // Format and required-field checks only (DD-56, DD-97): the backend refuses negative, zero or
  // too-large values, and its message lands on the field through FIELD_MAP.
  const notNumber = (v: number | null) => v !== null && !Number.isFinite(v);
  if (!input.name.trim()) errs.name = 'Item name is required';
  if (!input.baseUnit.trim()) errs.baseUnit = 'Base unit is required';
  if (priceVal === null || !Number.isFinite(priceVal)) errs.defaultSalePrice = 'Enter a sale price';
  if (mode !== 'service' && notNumber(lowVal)) errs.lowStockThreshold = 'Enter a number for the low stock level';
  const svcCostVal = parseOptional(input.serviceCost ?? '');
  if (mode === 'service' && notNumber(svcCostVal)) errs.serviceCost = 'Enter a number (leave empty for none)';
  if (notNumber(mrpVal)) errs.mrp = 'Enter a number (leave empty for none)';
  if (mode === 'create') {
    if (notNumber(stockVal)) errs.initialStock = 'Enter a number';
    if (notNumber(costVal)) errs.initialCost = 'Enter a number';
    if (stockVal !== null && costVal === null) errs.initialCost = 'Enter the cost of the opening stock';
  } else if (mode === 'quick-stock') {
    if (stockVal === null || !Number.isFinite(stockVal)) errs.initialStock = 'Enter the stock on hand';
    if (costVal === null || !Number.isFinite(costVal)) errs.initialCost = 'Enter the purchase price';
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
