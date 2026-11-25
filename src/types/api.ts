/**
 * API client contracts, payloads, and DTOs. camelCase mirrors of the Rust DTOs
 * in src-tauri/src/core/dtos; the IPC layer converts keys at the boundary.
 */

import type {
  AdjustmentReason,
  CashFlowReport,
  ChargeAppliesTo,
  ChargeEffect,
  ChargeType,
  DashboardMetrics,
  Expense,
  ExpenseType,
  SavedBundle,
  ItemBillRef,
  ItemLot,
  LotDraw,
  ItemSearchResult,
  ItemType,
  LowStockItem,
  Party,
  PartyDueSummary,
  PartyLedgerView,
  PartyListRow,
  PartyType,
  PaymentSettlement,
  PaymentStatus,
  PnLReport,
  PurchaseDetailView,
  PurchaseHistoryRow,
  RecentTransaction,
  ReturnDetailView,
  ReturnTransaction,
  ReturnType,
  SaleDetailView,
  SaleHistoryRow,
  SettlementType,
  ShopSettings,
  AppConfigView,
  ResetScope,
  ResetSummary,
  StockAdjustment,
  StockCondition,
  StockValuationReport,
  TransactionType,
  TrendDataPoint,
} from './index';

/** Rust Paginated<T>. Page count on the client is ceil(total / limit). */
export interface Paginated<T> {
  items: T[];
  total: number;
  page: number;
  limit: number;
}

/** Payload for create_expense / update_expense. */
export interface ExpensePayload {
  expenseDate: string;
  expenseTypeId: number;
  amount: number;
  notes: string | null;
}

/** Filter for list_expenses. Dates are YYYY-MM-DD, inclusive; newest first. */
export interface ExpenseFilter {
  startDate?: string | null;
  endDate?: string | null;
  expenseTypeId?: number | null;
  searchQuery?: string | null;
  page?: number | null;
  limit?: number | null;
}

/** `periodTotal` sums every row matching the filter, not just this page. */
export interface ExpenseListResponse extends Paginated<Expense> {
  periodTotal: number;
}

export type PaymentStatusFilter = 'ALL' | PaymentStatus;

/** Filter for get_sales_history and get_purchases_history. Dates are YYYY-MM-DD, inclusive. */
export interface HistoryFilter {
  startDate?: string | null;
  endDate?: string | null;
  paymentStatus?: PaymentStatusFilter | null;
  partyId?: number | null;
  searchQuery?: string | null;
  page?: number | null;
  limit?: number | null;
  /** Sales history only (FR-34): rows carry the bill profit. */
  includeProfit?: boolean | null;
}

export interface ReturnsFilter {
  returnType?: ReturnType | null;
  startDate?: string | null;
  endDate?: string | null;
  page?: number | null;
  limit?: number | null;
}

export interface CatalogFilter {
  category?: string | null;
  lowStockOnly?: boolean | null;
  search?: string | null;
  page?: number | null;
  limit?: number | null;
}

export interface CreateSaleItemPayload {
  itemId: number;
  bundleId: number | null;
  bundleQuantity: number;
  unitPrice: number;
}

export interface CreateSalePayload {
  transactionDate: string;
  partyId: number | null;
  paymentStatus: PaymentStatus;
  paidAmount: number;
  paymentMode: string | null;
  notes: string | null;
  items: CreateSaleItemPayload[];
  /** Bill-level charge/discount lines (FR-19). */
  charges: BillChargeInput[];
}

export interface BillChargeInput {
  chargeTypeId: number;
  amount: number;
}

export interface CreateChargeTypePayload {
  name: string;
  effect: ChargeEffect;
  appliesTo: ChargeAppliesTo;
  defaultAmount: number | null;
}

export interface SaleSummaryResponse {
  saleId: number;
  transactionCode: string;
  totalAmount: number;
  paidAmount: number;
  balanceDue: number;
  changeAmount: number;
  paymentStatus: PaymentStatus;
}

export interface CreatePurchaseItemPayload {
  itemId: number;
  bundleId: number | null;
  bundleQuantity: number;
  unitCost: number;
  /** Edit only (FR-20): the existing lot this line keeps; null/absent = a new line. Create rejects non-null. */
  purchaseItemId?: number | null;
}

export interface CreatePurchasePayload {
  transactionDate: string;
  partyId: number | null;
  supplierInvoiceRef: string | null;
  paymentStatus: PaymentStatus;
  paidAmount: number;
  paymentMode: string | null;
  notes: string | null;
  items: CreatePurchaseItemPayload[];
  /** Bill-level charge/discount lines (FR-19). */
  charges: BillChargeInput[];
}

export interface PurchaseSummaryResponse {
  purchaseId: number;
  transactionCode: string;
}

export interface SettlePaymentPayload {
  partyId: number;
  transactionType: TransactionType;
  /** null = lump-sum settlement across open bills (oldest first). */
  transactionId: number | null;
  settlementDate: string;
  amountPaid: number;
  paymentMode: string | null;
  notes: string | null;
}

export interface SettleAllocation {
  transactionId: number;
  amountPaid: number;
}

/** D9: 1..=200 unique bills of one party, settled atomically (all or nothing). */
export interface SettleBatchPayload {
  partyId: number;
  transactionType: TransactionType;
  settlementDate: string;
  paymentMode: string | null;
  notes: string | null;
  allocations: SettleAllocation[];
}

/** D8: edit of one SETTLEMENT ledger row. */
export interface UpdateSettlementPayload {
  amountPaid: number;
  settlementDate: string;
  paymentMode: string | null;
  notes: string | null;
}

/** FR-25: new Purchase Price of one lot (per pack when the lot has a pack label, else per base unit). */
export interface UpdateLotPricePayload {
  purchaseItemId: number;
  unitPrice: number;
}

export interface LotPriceBillEffect {
  purchaseId: number;
  transactionCode: string;
  oldTotal: number;
  newTotal: number;
  oldPaid: number;
  newPaid: number;
  balanceDue: number;
  paymentStatus: PaymentStatus;
}

export interface LotPriceUpdateResult {
  purchaseItemId: number;
  oldUnitCost: number;
  newUnitCost: number;
  /** Null for OPENING / ADJUSTMENT lots (no bill). */
  bill: LotPriceBillEffect | null;
}

export interface DeleteSettlementResult {
  removedAmount: number;
  transactionId: number;
}

export interface SettlementResult {
  settlementIds: number[];
  billsTouched: number[];
}

export interface CreateBundlePayload {
  itemId: number | null;
  bundleName: string;
  unitsPerBundle: number;
  mrp: number | null;
  defaultBundlePrice: number | null;
  barcode: string | null;
}

export interface UpdateBundlePayload {
  bundleName: string;
  unitsPerBundle: number;
  mrp: number | null;
  defaultBundlePrice: number | null;
  barcode: string | null;
}
export interface CreateItemPayload {
  name: string;
  category: string | null;
  barcode: string | null;
  baseUnit: string;
  mrp: number | null;
  defaultSalePrice: number;
  initialStock: number | null;
  initialCost: number | null;
  bundles: CreateBundlePayload[] | null;
  /** Optional; omitted => PHYSICAL. SERVICE is rejected by the backend in v1.1. */
  itemType?: ItemType | null;
  /** Optional; omitted => 5. 0 = never low. */
  lowStockThreshold?: number | null;
}

export interface UpdateItemPayload {
  name: string;
  category: string | null;
  barcode: string | null;
  baseUnit: string;
  mrp: number | null;
  defaultSalePrice: number;
  itemType?: ItemType | null;
  /** Optional; omitted/null => keep the current level. */
  lowStockThreshold?: number | null;
}

export interface BundleSuggestion {
  bundleName: string;
  unitsPerBundle: number;
  usageCount: number;
}

export interface CreatePartyPayload {
  name: string;
  phone: string | null;
  partyType: PartyType;
  notes: string | null;
}

export type UpdatePartyPayload = CreatePartyPayload;

export interface CustomerReturnItemPayload {
  itemId: number;
  saleItemId: number | null;
  quantity: number;
  unitPrice: number;
  condition: StockCondition;
  unitCostOverride: number | null;
}

export interface ReplacementItemPayload {
  itemId: number;
  quantity: number;
}

export interface CustomerReturnPayload {
  originalSaleId: number | null;
  partyId: number | null;
  returnDate: string;
  settlementType: SettlementType;
  reason?: string | null;
  notes: string | null;
  items: CustomerReturnItemPayload[];
  replacementItems: ReplacementItemPayload[];
}

export interface VendorReturnItemPayload {
  itemId: number;
  purchaseItemId: number | null;
  quantity: number;
}

/** Vendor returns must use settlementType REFUND. Unit cost comes from the lot. */
export interface VendorReturnPayload {
  originalPurchaseId: number | null;
  partyId: number | null;
  returnDate: string;
  settlementType: SettlementType;
  reason: string | null;
  notes: string | null;
  items: VendorReturnItemPayload[];
}

export interface ReturnResult {
  returnId: number;
  returnCode: string;
  creditedAmount: number;
  cashRefunded: number;
  /** Value of the returned lines (customer: qty x refund rate; vendor: the lot cost removed). */
  totalAmount: number;
  /** Replacement only: value of the issued items at their sale price; 0 otherwise. */
  issuedValue: number;
}

/** DD-56: one problem a preview found; the same field and message the real command would raise. */
export interface PreviewError {
  field: string;
  message: string;
}

export interface BillPaymentPreview {
  paidAmount: number;
  balanceDue: number;
  changeAmount: number;
  paymentStatus: PaymentStatus;
}

/** DD-56: what a sale/purchase would look like when saved. Every figure comes from the backend. */
export interface BillPreview {
  lineTotals: number[];
  itemsSubtotal: number;
  chargesTotal: number;
  discountsTotal: number;
  totalAmount: number;
  /** Settlements and return credits already applied to the bill (edit only). */
  appliedAmount: number;
  /** What the payment must cover: total - applied. */
  openAmount: number;
  /** null when the payload carried no payment status. */
  payment: BillPaymentPreview | null;
  errors: PreviewError[];
}

/** Bill payload for a preview: the payment is optional (totals and stock checks only without it). */
export type PreviewSalePayload = Omit<CreateSalePayload, 'paymentStatus' | 'paidAmount'> & {
  paymentStatus?: PaymentStatus | null;
  paidAmount?: number | null;
};
export type PreviewPurchasePayload = Omit<CreatePurchasePayload, 'paymentStatus' | 'paidAmount'> & {
  paymentStatus?: PaymentStatus | null;
  paidAmount?: number | null;
};

export interface StockAdjustmentPreview {
  previousStock: number;
  newStock: number;
  /** Lot cost a positive adjustment would use (explicit or the latest-lot default); null for a write-off. */
  unitCost: number | null;
  lossValue: number;
  valueChange: number;
}

export interface StockAdjustmentPayload {
  adjustmentDate: string;
  itemId: number;
  /** Positive adds stock, negative writes off. */
  quantity: number;
  reason: AdjustmentReason;
  /** Lot cost for positive adjustments. */
  unitCost: number | null;
  notes: string | null;
}

export interface StockAdjustmentResult {
  adjustmentId: number;
  lossValue: number;
}

export interface SetItemStockPayload {
  itemId: number;
  countedQuantity: number;
  unitCost: number | null;
  adjustmentDate: string;
  notes: string | null;
}

export interface SetItemStockResult {
  adjustmentId: number | null;
  delta: number;
  reason: AdjustmentReason | null;
  previousStock: number;
  newStock: number;
  unitCost: number | null;
  valueChange: number;
}

export type ConflictStrategy = 'SKIP' | 'OVERWRITE' | 'ADD_STOCK';

export interface ImportRowError {
  row: number;
  /** Item name on that row, when it has one. */
  name?: string | null;
  field: string;
  message: string;
}

/** The import will do something differently from the source; the row still imports. Row 0 = whole file. */
export interface ImportRowWarning {
  row: number;
  name: string;
  message: string;
}

export type HeaderIssueKind = 'UNKNOWN' | 'MISSING' | 'DUPLICATE';

export interface HeaderIssue {
  kind: HeaderIssueKind;
  column: string;
  position: number | null;
  columnLetter: string | null;
  message: string;
  suggestion: string | null;
}

export interface ExpectedColumn {
  name: string;
  required: boolean;
  rule: string;
  example: string;
}

export interface ImportDuplicate {
  row: number;
  name: string;
  existingItemId: number;
}

export interface ImportValidationReport {
  validCount: number;
  rowsWithErrors: ImportRowError[];
  duplicates: ImportDuplicate[];
  /** Non-empty: fix the header row first (rows were not checked). */
  headerIssues: HeaderIssue[];
  warnings: ImportRowWarning[];
  lotsToCreate: number;
  expectedColumns: ExpectedColumn[];
}

export type TemplateFormat = 'CSV' | 'XLSX';

export interface ImportSummary {
  created: number;
  updated: number;
  skipped: number;
  stockLotsCreated: number;
}

/** FR-20: what an edit/delete of a bill or return is blocked by (mirrors Rust `BillEditGuard`). */
export type CorrectionTarget = 'SALE' | 'PURCHASE' | 'RETURN';

export interface GuardReason {
  code: string;
  message: string;
  blocksEdit: boolean;
  blocksDelete: boolean;
}

export interface LotEditLimit {
  purchaseItemId: number;
  /** Base units that already left the lot: the floor for its new quantity. */
  consumedQuantity: number;
  /** Item change / removal is blocked. */
  referenced: boolean;
  /** The cost is locked (a vendor return used the lot). */
  vendorReturned: boolean;
}

export interface BillEditGuard {
  target: CorrectionTarget;
  id: number;
  code: string;
  canEdit: boolean;
  canDelete: boolean;
  canChangeParty: boolean;
  reasons: GuardReason[];
  /** Σ UPFRONT of the bill. */
  upfrontAmount: number;
  /** Σ SETTLEMENT + Σ RETURN_CREDIT: stays on an edit. */
  appliedAmount: number;
  /** Cash that leaves the records on delete. */
  removedCashAmount: number;
  /** Return credit that is taken back (returns only). */
  removedCreditAmount: number;
  lotLimits: LotEditLimit[];
}

export interface DeleteBillResult {
  removedCashAmount: number;
}

export interface DeleteReturnResult {
  removedCashAmount: number;
  removedCreditAmount: number;
}

export interface ApiClient {
  // Sales
  getNextSaleCode(): Promise<string>;
  createSale(payload: CreateSalePayload): Promise<SaleSummaryResponse>;
  getSalesHistory(filter?: HistoryFilter): Promise<Paginated<SaleHistoryRow>>;
  getSaleDetails(saleId: number): Promise<SaleDetailView>;
  /** FR-20: replaces the bill (same id and code). Rejects with `BillLocked` when guarded. */
  updateSale(saleId: number, payload: CreateSalePayload): Promise<SaleSummaryResponse>;
  deleteSale(saleId: number): Promise<DeleteBillResult>;
  /** DD-56: dry run of create/update (`saleId` = edit). Business errors come back in `errors`. */
  previewSale(payload: PreviewSalePayload, saleId?: number | null): Promise<BillPreview>;

  // Purchases
  getNextPurchaseCode(): Promise<string>;
  createPurchase(payload: CreatePurchasePayload): Promise<PurchaseSummaryResponse>;
  getPurchasesHistory(filter?: HistoryFilter): Promise<Paginated<PurchaseHistoryRow>>;
  getPurchaseDetails(purchaseId: number): Promise<PurchaseDetailView>;
  updatePurchase(purchaseId: number, payload: CreatePurchasePayload): Promise<PurchaseSummaryResponse>;
  deletePurchase(purchaseId: number): Promise<DeleteBillResult>;
  /** FR-25: re-price one lot (bill + payment rule for PURCHASE lots; re-cost only otherwise). */
  updateLotPurchasePrice(payload: UpdateLotPricePayload): Promise<LotPriceUpdateResult>;
  /** DD-56: dry run of `updatePurchase` (`purchaseId` = edit). Business errors come back in `errors`. */
  previewPurchase(payload: PreviewPurchasePayload, purchaseId?: number | null): Promise<BillPreview>;
  /** DD-56: dry run of `updateLotPurchasePrice`; rejects with the error the command would give. */
  previewLotPurchasePrice(payload: UpdateLotPricePayload): Promise<LotPriceUpdateResult>;

  // Items & Catalog
  searchItemsFts(query: string, limit?: number): Promise<ItemSearchResult[]>;
  getItemsCatalog(filter?: CatalogFilter): Promise<Paginated<ItemSearchResult>>;
  listCategories(): Promise<string[]>;
  createItem(payload: CreateItemPayload): Promise<number>;
  updateItem(itemId: number, payload: UpdateItemPayload): Promise<void>;
  /** Sets the per-item low stock level (>= 0; 0 = never) and returns the refreshed row. */
  updateItemLowStockThreshold(itemId: number, threshold: number): Promise<ItemSearchResult>;
  getBundleSuggestions(): Promise<BundleSuggestion[]>;
  createBundle(payload: CreateBundlePayload): Promise<SavedBundle>;
  updateBundle(bundleId: number, payload: UpdateBundlePayload): Promise<SavedBundle>;
  deleteBundle(bundleId: number): Promise<void>;

  // Dues & Ledger
  getPartyDuesSummary(partyType: PartyType): Promise<PartyDueSummary[]>;
  settleBillPayment(payload: SettlePaymentPayload): Promise<SettlementResult>;
  /** DD-56: the oldest-first split a lump-sum settlement of `amount` would book (rejects like the command). */
  previewSettlementAllocation(
    partyId: number,
    transactionType: TransactionType,
    amount: number
  ): Promise<SettleAllocation[]>;
  /** The single settlement core: all allocations commit together or none do. */
  settleBillsBatch(payload: SettleBatchPayload): Promise<SettlementResult>;
  /** SETTLEMENT rows only; other kinds are a ValidationError(kind). */
  updateSettlement(settlementId: number, payload: UpdateSettlementPayload): Promise<PaymentSettlement>;
  deleteSettlement(settlementId: number): Promise<DeleteSettlementResult>;
  getPartyLedger(partyId: number): Promise<PartyLedgerView>;

  // Parties
  createParty(payload: CreatePartyPayload): Promise<number>;
  updateParty(partyId: number, payload: UpdatePartyPayload): Promise<void>;
  getParty(partyId: number): Promise<Party>;
  listParties(partyType?: PartyType, activeOnly?: boolean): Promise<PartyListRow[]>;
  searchParties(query: string, partyType?: PartyType): Promise<PartyListRow[]>;

  // Charge / discount types (FR-19)
  /** Live type search for one bill side; a blank query lists the applicable types by name. */
  searchChargeTypes(query: string, appliesTo: TransactionType, limit?: number): Promise<ChargeType[]>;
  /** A name that already exists (case-insensitive) is rejected with a ValidationError on `name`. */
  createChargeType(payload: CreateChargeTypePayload): Promise<ChargeType>;

  // Expenses (FR-18)
  /** Live type search; a blank query returns the first active types. */
  searchExpenseTypes(query: string, limit?: number): Promise<ExpenseType[]>;
  listExpenseTypes(): Promise<ExpenseType[]>;
  /** Idempotent: an existing name (case-insensitive) returns the existing type. */
  createExpenseType(name: string): Promise<ExpenseType>;
  updateExpenseType(expenseTypeId: number, name: string): Promise<ExpenseType>;
  getNextExpenseCode(): Promise<string>;
  createExpense(payload: ExpensePayload): Promise<Expense>;
  updateExpense(expenseId: number, payload: ExpensePayload): Promise<Expense>;
  deleteExpense(expenseId: number): Promise<void>;
  listExpenses(filter?: ExpenseFilter): Promise<ExpenseListResponse>;

  // Returns & Adjustments
  getNextReturnCode(): Promise<string>;
  createCustomerReturn(payload: CustomerReturnPayload): Promise<ReturnResult>;
  createVendorReturn(payload: VendorReturnPayload): Promise<ReturnResult>;
  /** DD-56: dry runs of the return commands (same result and errors, nothing saved). */
  previewCustomerReturn(payload: CustomerReturnPayload): Promise<ReturnResult>;
  previewVendorReturn(payload: VendorReturnPayload): Promise<ReturnResult>;
  getReturnDetails(returnId: number): Promise<ReturnDetailView>;
  /** FR-20: newest return of a bill first; rejects with `BillLocked` otherwise. */
  deleteReturn(returnId: number): Promise<DeleteReturnResult>;
  /** FR-20 pre-check shown before edit/delete actions. */
  getBillEditGuard(target: CorrectionTarget, id: number): Promise<BillEditGuard>;
  listReturns(filter?: ReturnsFilter): Promise<Paginated<ReturnTransaction>>;
  createStockAdjustment(payload: StockAdjustmentPayload): Promise<StockAdjustmentResult>;
  /** Set counted stock on hand; delta>0 = unrecorded stock lot, <0 = count-correction write-off, 0 = no-op. */
  setItemStock(payload: SetItemStockPayload): Promise<SetItemStockResult>;
  /** DD-56: dry runs of the stock commands: the unit cost used, the FIFO loss and the value change. */
  previewStockAdjustment(payload: StockAdjustmentPayload): Promise<StockAdjustmentPreview>;
  previewSetItemStock(payload: SetItemStockPayload): Promise<SetItemStockResult>;
  /** FR-31: open lots in FIFO order and the draw of `quantity` after `priorQuantity` (base units). */
  previewLotDraw(itemId: number, quantity: number, priorQuantity: number): Promise<LotDraw>;
  /**
   * NOTE: get_item_lots is implemented in adjustment_controller.rs but is NOT yet in
   * main.rs generate_handler!. It will reject until the backend registers it.
   */
  getItemLots(itemId: number): Promise<ItemLot[]>;
  /** FR-23: bills (sales) containing the item, newest first, one row per bill. Default limit 10, max 50. */
  getItemSales(itemId: number, page?: number, limit?: number): Promise<Paginated<ItemBillRef>>;
  /** FR-23: `source = PURCHASE` bills containing the item. */
  getItemPurchases(itemId: number, page?: number, limit?: number): Promise<Paginated<ItemBillRef>>;
  listStockAdjustments(
    itemId?: number,
    page?: number,
    limit?: number
  ): Promise<Paginated<StockAdjustment>>;

  // Reports
  getDashboardMetrics(): Promise<DashboardMetrics>;
  getLowStockAlerts(): Promise<LowStockItem[]>;
  /** period: '7d' | '30d' | '1y'. */
  getSalesPurchaseTrend(period: string): Promise<TrendDataPoint[]>;
  getRecentTransactions(limit: number): Promise<RecentTransaction[]>;
  getPnLReport(fromDate: string, toDate: string): Promise<PnLReport>;
  getStockValuationReport(): Promise<StockValuationReport>;
  getCashFlowReport(fromDate: string, toDate: string): Promise<CashFlowReport>;

  // Settings & System
  getShopSettings(): Promise<ShopSettings>;
  updateShopSettings(settings: ShopSettings): Promise<void>;
  backupDatabase(destinationPath: string): Promise<string>;
  getAppConfig(): Promise<AppConfigView>;
  /** `null` resets to the default backup folder. */
  setBackupDirectory(path: string | null): Promise<AppConfigView>;
  /** Testing only: the backend refuses unless testing mode is on. Writes a pre-reset backup first. */
  resetData(scope: ResetScope): Promise<ResetSummary>;
  verifyDatabaseIntegrity(): Promise<boolean>;

  // Items import
  validateItemsImport(filePath: string): Promise<ImportValidationReport>;
  executeItemsImport(
    filePath: string,
    conflictStrategy: ConflictStrategy
  ): Promise<ImportSummary>;
  validateVedbillImport(filePath: string): Promise<ImportValidationReport>;
  executeVedbillImport(filePath: string, conflictStrategy: ConflictStrategy): Promise<ImportSummary>;
  /** Saves the items-import template; resolves to the final path. */
  writeItemsImportTemplate(destPath: string, format: TemplateFormat): Promise<string>;
}
