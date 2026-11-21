/** Page count for a Paginated<T> result: ceil(total / limit), at least 1. */
export function totalPages(total: number, limit: number): number {
  if (limit <= 0) return 1;
  return Math.max(1, Math.ceil(total / limit));
}
