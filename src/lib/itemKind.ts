import type { ItemSearchResult } from '@/types';

/** Services hold no stock of their own; what they offer is the backend's `canMake`. */
export const isService = (item: Pick<ItemSearchResult, 'itemType'>): boolean =>
  item.itemType === 'SERVICE';
