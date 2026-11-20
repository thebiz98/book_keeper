// ============================================================================
// Ambient Module Type Contracts for Book Keeper Frontend
// Derived strictly from docs/book_keeper_UI_LLD.md & TEST_INFRA.md
// ============================================================================

declare module '@/store/useCartStore' {
  export interface CartItem {
    itemId: number;
    name: string;
    barcode?: string;
    baseUnit: string;
    unitsPerBundle: number;
    bundleName?: string;
    quantity: number;
    baseQuantity: number;
    rate: number;
    mrp?: number;
    lineTotal: number;
    availableStock: number;
  }

  export interface ItemInput {
    id: number;
    name: string;
    barcode?: string;
    base_unit?: string;
    baseUnit?: string;
    default_sale_price?: number;
    defaultSalePrice?: number;
    mrp?: number;
    current_stock?: number;
    currentStock?: number;
  }

  export interface BundleInput {
    id?: number;
    name: string;
    bundle_name?: string;
    bundleName?: string;
    multiplier: number;
    units_per_bundle?: number;
    unitsPerBundle?: number;
    sale_price?: number;
    salePrice?: number;
    mrp?: number;
  }

  export interface CustomerInput {
    id: number;
    name: string;
    phone?: string;
    balanceDue?: number;
    balance_due?: number;
  }

  export interface CartState {
    items: CartItem[];
    subtotal: number;
    totalBaseUnits: number;
    totalItems: number;
    selectedCustomer: CustomerInput | null;
    paymentStatus: 'PAID' | 'PARTIAL' | 'UNPAID';
    paidAmount: number;
    balanceDue: number;

    addItem: (item: ItemInput, quantity?: number, bundle?: BundleInput | null) => boolean;
    updateQuantity: (itemId: number, quantity: number) => boolean;
    setBundle: (itemId: number, bundle: BundleInput | null) => void;
    updateRate: (itemId: number, rate: number) => void;
    removeItem: (itemId: number) => void;
    clearCart: () => void;
    setCustomer: (customer: CustomerInput | null) => void;
    setPaymentStatus: (status: 'PAID' | 'PARTIAL' | 'UNPAID', paidAmount?: number) => void;
    setPaidAmount: (amount: number) => void;
  }

  export const useCartStore: {
    (): CartState;
    getState: () => CartState;
    setState: (partial: Partial<CartState> | ((state: CartState) => Partial<CartState>)) => void;
    subscribe: (listener: (state: CartState) => void) => () => void;
  };
}

declare module '@/store/useAppStore' {
  export interface ShopProfile {
    shopName: string;
    phone?: string;
    address?: string;
    taxId?: string;
  }

  export interface CashDrawerFlow {
    salesCash?: number;
    customerDuesSettled?: number;
    purchaseCash?: number;
    vendorDuesSettled?: number;
    refunds?: number;
  }

  export interface AppState {
    activeRoute: string;
    shopProfile: ShopProfile;
    isDbConnected: boolean;
    salesCash: number;
    customerDuesSettled: number;
    purchaseCash: number;
    vendorDuesSettled: number;
    refunds: number;

    setActiveRoute: (route: string) => void;
    setShopProfile: (profile: Partial<ShopProfile>) => void;
    setDbConnected: (connected: boolean) => void;
    recordCashFlow: (flow: CashDrawerFlow) => void;
    resetCashDrawer: () => void;
    calculateCashDrawer: () => number;
    liveCashInDrawer: number;
  }

  export const useAppStore: {
    (): AppState;
    getState: () => AppState;
    setState: (partial: Partial<AppState> | ((state: AppState) => Partial<AppState>)) => void;
    subscribe: (listener: (state: AppState) => void) => () => void;
  };
}

declare module '@/store/useThemeStore' {
  export interface ThemeState {
    theme: 'light' | 'dark';
    toggleTheme: () => void;
    setTheme: (theme: 'light' | 'dark') => void;
  }

  export const useThemeStore: {
    (): ThemeState;
    getState: () => ThemeState;
    setState: (partial: Partial<ThemeState> | ((state: ThemeState) => Partial<ThemeState>)) => void;
    subscribe: (listener: (state: ThemeState) => void) => () => void;
  };
}

declare module '@/components/domain/RapidBillingRow' {
  import React from 'react';

  export interface RapidBillingRowProps {
    onAddItem?: (item: any) => void;
    onOpenBundleModal?: () => void;
    autoFocus?: boolean;
    className?: string;
  }

  export const RapidBillingRow: React.FC<RapidBillingRowProps>;
  export default RapidBillingRow;
}

declare module '@/components/RapidBillingRow' {
  import React from 'react';
  export interface RapidBillingRowProps {
    onAddItem?: (item: any) => void;
    onOpenBundleModal?: () => void;
    autoFocus?: boolean;
    className?: string;
  }
  export const RapidBillingRow: React.FC<RapidBillingRowProps>;
  export default RapidBillingRow;
}

declare module '@/components/domain/SaleSummaryModal' {
  import React from 'react';

  export interface SaleSummaryModalProps {
    isOpen: boolean;
    onClose: () => void;
    onConfirm?: () => void;
    grandTotal: number;
    invoiceCode?: string;
    customerName?: string;
    paymentStatus?: 'PAID' | 'PARTIAL' | 'UNPAID';
    defaultCashReceived?: number;
  }

  export const SaleSummaryModal: React.FC<SaleSummaryModalProps>;
  export default SaleSummaryModal;
}

declare module '@/components/SaleSummaryModal' {
  import React from 'react';
  export interface SaleSummaryModalProps {
    isOpen: boolean;
    onClose: () => void;
    onConfirm?: () => void;
    grandTotal: number;
    invoiceCode?: string;
    customerName?: string;
    paymentStatus?: 'PAID' | 'PARTIAL' | 'UNPAID';
    defaultCashReceived?: number;
  }
  export const SaleSummaryModal: React.FC<SaleSummaryModalProps>;
  export default SaleSummaryModal;
}

declare module '@/components/common/StockStatusBadge' {
  import React from 'react';

  export interface StockStatusBadgeProps {
    currentStock: number;
    isLowStock?: boolean;
    baseUnit?: string;
    className?: string;
  }

  export const StockStatusBadge: React.FC<StockStatusBadgeProps>;
  export default StockStatusBadge;
}

declare module '@/components/StockStatusBadge' {
  import React from 'react';
  export interface StockStatusBadgeProps {
    currentStock: number;
    isLowStock?: boolean;
    baseUnit?: string;
    className?: string;
  }
  export const StockStatusBadge: React.FC<StockStatusBadgeProps>;
  export default StockStatusBadge;
}

declare module '@/components/common/CurrencyDisplay' {
  import React from 'react';

  export interface CurrencyDisplayProps {
    amount: number;
    currencySymbol?: string;
    size?: 'sm' | 'md' | 'lg' | 'xl' | 'hero';
    colored?: boolean;
    showSign?: boolean;
    className?: string;
  }

  export const CurrencyDisplay: React.FC<CurrencyDisplayProps>;
  export default CurrencyDisplay;
}

declare module '@/components/CurrencyDisplay' {
  import React from 'react';
  export interface CurrencyDisplayProps {
    amount: number;
    currencySymbol?: string;
    size?: 'sm' | 'md' | 'lg' | 'xl' | 'hero';
    colored?: boolean;
    showSign?: boolean;
    className?: string;
  }
  export const CurrencyDisplay: React.FC<CurrencyDisplayProps>;
  export default CurrencyDisplay;
}

declare module '@/api/mock/mockClient' {
  export interface ApiClient {
    getDashboardMetrics(): Promise<any>;
    getLowStockAlerts(): Promise<any[]>;
    getSalesPurchaseTrend(period: '7d' | '30d' | '1y'): Promise<any[]>;
    getNextSaleCode(): Promise<string>;
    createSale(payload: any): Promise<any>;
    getSalesHistory(filter?: any): Promise<any>;
    getSaleDetails(saleId: number): Promise<any>;
    getNextPurchaseCode(): Promise<string>;
    createPurchase(payload: any): Promise<any>;
    getPurchasesHistory(filter?: any): Promise<any>;
    searchItemsFts(query: string, limit?: number): Promise<any[]>;
    getItemsCatalog(filter?: any): Promise<any>;
    createItem(payload: any): Promise<any>;
    updateItem(itemId: number, payload: any): Promise<any>;
    archiveItem(itemId: number): Promise<boolean>;
    getBundleSuggestions(): Promise<any[]>;
    createBundle(payload: any): Promise<any>;
    getPartyDuesSummary(partyType: 'CUSTOMER' | 'VENDOR'): Promise<any[]>;
    settleBillPayment(payload: any): Promise<boolean>;
    createParty(payload: any): Promise<any>;
    updateParty(partyId: number, payload: any): Promise<boolean>;
    archiveParty(partyId: number): Promise<boolean>;
    listParties(partyType?: 'CUSTOMER' | 'VENDOR', activeOnly?: boolean): Promise<any[]>;
    searchParties(query: string, partyType?: 'CUSTOMER' | 'VENDOR'): Promise<any[]>;
    createCustomerReturn(payload: any): Promise<any>;
    createVendorReturn(payload: any): Promise<any>;
    createStockAdjustment(payload: any): Promise<any>;
    listStockAdjustments(itemId?: number, page?: number, limit?: number): Promise<any>;
    getPnLReport(fromDate: string, toDate: string): Promise<any>;
    getStockValuationReport(): Promise<any>;
    getCashFlowReport(fromDate: string, toDate: string): Promise<any>;
    backupDatabase(destinationPath: string): Promise<string>;
    getShopSettings(): Promise<any>;
    updateShopSettings(settings: any): Promise<boolean>;
  }

  export const MockApiClient: new () => ApiClient;
  export const mockApiClient: ApiClient;
}

declare module '@/api/mockApiClient' {
  export * from '@/api/mock/mockClient';
}
