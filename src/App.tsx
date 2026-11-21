import React, { useMemo, useState } from 'react';
import { AppShell } from '@/components/layout/AppShell';
import { Modal } from '@/components/ui/Modal';
import { ItemSearchTypeahead } from '@/components/domain/ItemSearchTypeahead';
import { CurrencyDisplay } from '@/components/common/CurrencyDisplay';
import { StockStatusBadge } from '@/components/common/StockStatusBadge';
import { useAppStore } from '@/store/useAppStore';
import { useGlobalHotkeys } from '@/hooks/useGlobalHotkeys';
import { DashboardPage } from '@/pages/DashboardPage';
import { NewSalePage } from '@/pages/NewSalePage';
import { NewPurchasePage } from '@/pages/NewPurchasePage';
import { CreditDuesPage } from '@/pages/CreditDuesPage';
import { InventoryPage } from '@/pages/InventoryPage';
import { SalesHistoryPage } from '@/pages/SalesHistoryPage';
import { PurchaseHistoryPage } from '@/pages/PurchaseHistoryPage';
import { ReturnsAdjustmentsPage } from '@/pages/ReturnsAdjustmentsPage';
import { PartiesPage } from '@/pages/PartiesPage';
import { ReportsPage } from '@/pages/ReportsPage';
import { SettingsPage } from '@/pages/SettingsPage';
import { MigrationPage } from '@/pages/MigrationPage';

const ROUTES: Record<string, React.FC> = {
  dashboard: DashboardPage,
  sale: NewSalePage,
  billing: NewSalePage,
  'new-sale': NewSalePage,
  purchase: NewPurchasePage,
  purchases: NewPurchasePage,
  'new-purchase': NewPurchasePage,
  dues: CreditDuesPage,
  'credit-dues': CreditDuesPage,
  inventory: InventoryPage,
  items: InventoryPage,
  'sales-history': SalesHistoryPage,
  'purchase-history': PurchaseHistoryPage,
  returns: ReturnsAdjustmentsPage,
  'returns-adjustments': ReturnsAdjustmentsPage,
  parties: PartiesPage,
  reports: ReportsPage,
  settings: SettingsPage,
  migration: MigrationPage,
  'data-migration': MigrationPage,
};

const ItemLookupModal: React.FC<{ isOpen: boolean; onClose: () => void }> = ({
  isOpen,
  onClose,
}) => {
  const [item, setItem] = useState<any>(null);
  const stock = item ? (item.currentStock ?? item.current_stock ?? 0) : 0;
  return (
    <Modal
      isOpen={isOpen}
      onClose={() => {
        setItem(null);
        onClose();
      }}
      title="Item Stock & Rate Lookup (F4)"
      size="md"
    >
      <div className="flex flex-col gap-4">
        <ItemSearchTypeahead onSelectItem={setItem} autoFocus />
        {item && (
          <div className="rounded-md border border-slate-200 dark:border-slate-800 p-3 text-sm flex flex-col gap-1">
            <span className="font-semibold">{item.name}</span>
            <StockStatusBadge currentStock={stock} baseUnit={item.baseUnit ?? item.base_unit ?? 'PCS'} />
            <span>
              Rate:{' '}
              <CurrencyDisplay amount={item.defaultSalePrice ?? item.default_sale_price ?? 0} />
            </span>
          </div>
        )}
      </div>
    </Modal>
  );
};

const App: React.FC = () => {
  const activeRoute = useAppStore((s) => s.activeRoute);
  const setActiveRoute = useAppStore((s) => s.setActiveRoute);
  const [lookupOpen, setLookupOpen] = useState(false);

  const handlers = useMemo(
    () => ({
      F1: () => setActiveRoute('dashboard'),
      F2: () => setActiveRoute('sale'),
      F3: () => setActiveRoute('purchase'),
      F4: () => setLookupOpen(true),
      F5: () => setActiveRoute('dues'),
      F6: () => setActiveRoute('inventory'),
      'Ctrl+B': () => setActiveRoute('settings'),
    }),
    [setActiveRoute]
  );
  useGlobalHotkeys(handlers);

  const Page = ROUTES[activeRoute] ?? DashboardPage;

  return (
    <AppShell>
      <Page />
      <ItemLookupModal isOpen={lookupOpen} onClose={() => setLookupOpen(false)} />
    </AppShell>
  );
};

export default App;
