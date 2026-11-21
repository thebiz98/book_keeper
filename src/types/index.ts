/**
 * Core Domain Models for Book Keeper.
 * Derived from src-tauri/src/core/entities/ & dtos/.
 */

export * from './store';
export * from './api';

export type PartyType = 'CUSTOMER' | 'VENDOR' | 'Customer' | 'Vendor';

export interface PackagingBundle {
  id: number;
  itemId?: number;
  item_id?: number;
  name?: string;
  bundleName: string;
  bundle_name?: string;
  multiplier?: number;
  unitsPerBundle: number;
  units_per_bundle?: number;
  salePrice?: number | null;
  sale_price?: number | null;
  defaultBundlePrice?: number | null;
  default_bundle_price?: number | null;
  mrp?: number | null;
  barcode?: string | null;
}

export interface Item {
  id: number;
  itemId?: number;
  item_id?: number;
  name: string;
  category?: string | null;
  barcode?: string | null;
  baseUnit: string;
  base_unit?: string;
  mrp?: number | null;
  costPrice?: number;
  cost_price?: number;
  defaultSalePrice: number;
  default_sale_price?: number;
  currentStock: number;
  current_stock?: number;
  isLowStock?: boolean;
  is_low_stock?: boolean;
  isActive?: boolean;
  is_active?: boolean;
  createdAt?: string;
  created_at?: string;
  updatedAt?: string;
  updated_at?: string;
  bundles?: PackagingBundle[];
}

export interface Party {
  id: number;
  partyId?: number;
  party_id?: number;
  name: string;
  partyName?: string;
  party_name?: string;
  phone?: string | null;
  address?: string | null;
  partyType: PartyType;
  party_type?: PartyType;
  balanceDue?: number;
  balance_due?: number;
  netDueBalance?: number;
  totalDue?: number;
  total_due?: number;
  oldestBillDate?: string | null;
  oldest_bill_date?: string | null;
  unpaidBillsCount?: number;
  unpaid_bills_count?: number;
  notes?: string | null;
  isActive?: boolean;
  is_active?: boolean;
  createdAt?: string;
  created_at?: string;
  updatedAt?: string;
  updated_at?: string;
}

export interface SaleItem {
  id?: number;
  saleTransactionId?: number;
  sale_transaction_id?: number;
  itemId: number;
  item_id?: number;
  name?: string;
  itemName?: string;
  item_name?: string;
  bundleId?: number | null;
  bundle_id?: number | null;
  bundleName?: string | null;
  bundle_name?: string | null;
  bundleQuantity?: number;
  bundle_quantity?: number;
  quantity: number;
  baseQuantity?: number;
  rate?: number;
  unitPrice?: number;
  unit_price?: number;
  totalAmount?: number;
  total_amount?: number;
  lineTotal?: number;
  unitsPerBundle?: number;
  units_per_bundle?: number;
}

export interface Sale {
  id: number;
  saleId?: number;
  sale_id?: number;
  transactionCode: string;
  transaction_code?: string;
  saleCode?: string;
  sale_code?: string;
  transactionDate: string;
  transaction_date?: string;
  date?: string;
  partyId?: number | null;
  party_id?: number | null;
  customerId?: number | null;
  customerName?: string | null;
  paymentMode?: string | null;
  payment_mode?: string | null;
  paymentStatus: string;
  payment_status?: string;
  totalAmount: number;
  total_amount?: number;
  paidAmount: number;
  paid_amount?: number;
  balanceDue: number;
  balance_due?: number;
  changeAmount?: number;
  change_amount?: number;
  changeReturned?: number;
  change_returned?: number;
  notes?: string | null;
  createdAt?: string;
  created_at?: string;
  items?: SaleItem[];
}

export interface PurchaseItem {
  id?: number;
  purchaseTransactionId?: number;
  purchase_transaction_id?: number;
  itemId: number;
  item_id?: number;
  itemName?: string;
  item_name?: string;
  bundleId?: number | null;
  bundle_id?: number | null;
  bundleName?: string | null;
  bundle_name?: string | null;
  bundleQuantity?: number;
  bundle_quantity?: number;
  quantity: number;
  unitCost: number;
  unit_cost?: number;
  totalAmount?: number;
  total_amount?: number;
  remainingQuantity?: number;
  remaining_quantity?: number;
  unitsPerBundle?: number;
  units_per_bundle?: number;
}

export interface Purchase {
  id: number;
  purchaseId?: number;
  purchase_id?: number;
  transactionCode: string;
  transaction_code?: string;
  purchaseCode?: string;
  purchase_code?: string;
  transactionDate: string;
  transaction_date?: string;
  date?: string;
  partyId?: number | null;
  party_id?: number | null;
  vendorId?: number | null;
  vendorName?: string | null;
  paymentMode?: string | null;
  payment_mode?: string | null;
  paymentStatus: string;
  payment_status?: string;
  totalAmount: number;
  total_amount?: number;
  paidAmount: number;
  paid_amount?: number;
  balanceDue: number;
  balance_due?: number;
  notes?: string | null;
  createdAt?: string;
  created_at?: string;
  items?: PurchaseItem[];
}

export interface StockLot {
  id: number;
  saleItemId?: number;
  sale_item_id?: number;
  purchaseItemId: number;
  purchase_item_id?: number;
  quantityDepleted: number;
  quantity_depleted?: number;
  unitCost: number;
  unit_cost?: number;
}

export interface Payment {
  id?: number;
  settlementDate: string;
  settlement_date?: string;
  date?: string;
  partyId?: number | null;
  party_id?: number | null;
  transactionType: 'SALE' | 'PURCHASE';
  transaction_type?: string;
  transactionId: number;
  transaction_id?: number;
  amountPaid: number;
  amount_paid?: number;
  amount?: number;
  paymentMode?: string | null;
  payment_mode?: string | null;
  notes?: string | null;
  createdAt?: string;
  created_at?: string;
}

export interface PartyBalance {
  partyId: number;
  party_id?: number;
  id?: number;
  partyName: string;
  party_name?: string;
  name?: string;
  phone?: string | null;
  partyType: PartyType;
  party_type?: PartyType;
  totalDue?: number;
  total_due?: number;
  netDueBalance?: number;
  balanceDue?: number;
  balance_due?: number;
  oldestBillDate?: string | null;
  oldest_bill_date?: string | null;
  unpaidBillsCount?: number;
  unpaid_bills_count?: number;
}

export interface PnlReport {
  fromDate?: string;
  from_date?: string;
  toDate?: string;
  to_date?: string;
  grossSales: number;
  gross_sales?: number;
  cogs: number;
  cost_of_goods_sold?: number;
  netProfit: number;
  net_profit?: number;
  totalDiscounts?: number;
  total_discounts?: number;
  returnsRefunded?: number;
  returns_refunded?: number;
}

export interface CategoryValuation {
  category: string;
  itemsCount?: number;
  items_count?: number;
  valuation: number;
}

export interface StockValuationReport {
  totalValuation: number;
  total_valuation?: number;
  activeLotsCount?: number;
  active_lots_count?: number;
  totalItemsCount?: number;
  total_items_count?: number;
  categoriesBreakdown?: CategoryValuation[];
  categories_breakdown?: CategoryValuation[];
  categories?: CategoryValuation[];
}

export interface CashFlowReport {
  fromDate?: string;
  from_date?: string;
  toDate?: string;
  to_date?: string;
  totalInflow?: number;
  total_inflow?: number;
  totalOutflow?: number;
  total_outflow?: number;
  netCashFlow?: number;
  net_cash_flow?: number;
  closingDrawerBalance?: number;
  closing_drawer_balance?: number;
  salesCashInflow?: number;
  sales_cash_inflow?: number;
  customerSettlementsInflow?: number;
  customer_settlements_inflow?: number;
  purchaseCashOutflow?: number;
  purchase_cash_outflow?: number;
  vendorSettlementsOutflow?: number;
  vendor_settlements_outflow?: number;
  refundsCashOutflow?: number;
  refunds_cash_outflow?: number;
  netCashBalance?: number;
  net_cash_balance?: number;
}

export interface ShopSettings {
  id?: number;
  shopName: string;
  shop_name?: string;
  phone?: string | null;
  address?: string | null;
  taxId?: string | null;
  tax_id?: string | null;
  gstNumber?: string | null;
  gst_number?: string | null;
}
