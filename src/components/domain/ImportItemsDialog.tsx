import React from 'react';
import { ImportDialog } from './ImportDialog';

export type { ImportSource } from './ImportDialog';

export interface ImportItemsDialogProps {
  isOpen: boolean;
  onClose: () => void;
  /** Called after a successful import so the catalog can refresh. */
  onImported: () => void;
}

/** The Inventory import: items from a spreadsheet, service components, or a VedBill backup. */
export const ImportItemsDialog: React.FC<ImportItemsDialogProps> = (props) => <ImportDialog kind="ITEMS" {...props} />;

export default ImportItemsDialog;
