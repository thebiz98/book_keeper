use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaginatedList<T> {
    pub items: Vec<T>,
    pub total_count: usize,
    pub page: usize,
    pub limit: usize,
    pub total_pages: usize,
}

impl<T> PaginatedList<T> {
    pub fn new(items: Vec<T>, total_count: usize, page: usize, limit: usize) -> Self {
        let total_pages = if limit == 0 {
            0
        } else {
            total_count.div_ceil(limit)
        };
        Self {
            items,
            total_count,
            page,
            limit,
            total_pages,
        }
    }
}
