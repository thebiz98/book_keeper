import React, { useState } from 'react';
import type { ItemSearchResult } from '@/types';
import { AppShell } from '@/components/layout/AppShell';
import { Modal } from '@/components/ui/Modal';
import { ItemSearchTypeahead } from '@/components/domain/ItemSearchTypeahead';
import { CurrencyDisplay } from '@/components/common/CurrencyDisplay';
import { StockStatusBadge } from '@/components/common/StockStatusBadge';
import { useAppStore } from '@/store/useAppStore';
import { useAppHotkeys } from '@/hooks/useAppHotkeys';
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
};

export const ItemLookupModal: React.FC<{ isOpen: boolean; onClose: () => void }> = ({
  isOpen,
  onClose,
}) => {
  const [item, setItem] = useState<ItemSearchResult | null>(null);
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
        <ItemSearchTypeahead onSelectItem={setItem} autoFocus clearOnSelect={false} />
        {item && (
          <div
            data-testid="lookup-result"
            className="rounded-md border border-slate-200 dark:border-slate-800 p-3 text-sm flex flex-col gap-1"
          >
            <span className="font-semibold">{item.name}</span>
            <StockStatusBadge currentStock={item.currentStock} baseUnit={item.baseUnit} />
            <span>
              Rate: <CurrencyDisplay amount={item.defaultSalePrice} />
              {item.mrp != null && (
                <>
                  {' '}
                  | MRP: <CurrencyDisplay amount={item.mrp} />
                </>
              )}
            </span>
          </div>
        )}
      </div>
    </Modal>
  );
};

const App: React.FC = () => {
  const activeRoute = useAppStore((s) => s.activeRoute);
  const lookupOpen = useAppStore((s) => s.lookupOpen);
  const closeLookup = useAppStore((s) => s.closeLookup);

  useAppHotkeys();

  const Page = ROUTES[activeRoute] ?? DashboardPage;

  return (
    <AppShell>
      <Page />
      <ItemLookupModal isOpen={lookupOpen} onClose={closeLookup} />
    </AppShell>
  );
};

export default App;
