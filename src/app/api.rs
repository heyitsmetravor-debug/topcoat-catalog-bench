mod categories;
mod customers;
mod health;
mod meta;
mod orders;
mod products;
mod regions;
mod search;

use catalog_bench::catalog::{self, OrderLookup, ProductLookup};
use topcoat::{
    Result,
    router::{error::bad_request, query_params},
};

#[query_params(error = bad_request)]
pub struct ProductQuery {
    pub q: Option<String>,
    pub category: Option<String>,
    pub region: Option<String>,
    pub min_price: Option<u32>,
    pub max_price: Option<u32>,
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

#[query_params(error = bad_request)]
pub struct OrderQuery {
    pub region: Option<String>,
    pub status: Option<String>,
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

#[query_params(error = bad_request)]
pub struct PageQuery {
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

pub fn product_lookup(query: &ProductQuery, default_limit: u32) -> Result<ProductLookup> {
    catalog::product_lookup_from_parts(
        query.q.as_deref(),
        query.category.as_deref(),
        query.region.as_deref(),
        query.min_price,
        query.max_price,
        query.page.unwrap_or(1),
        query.limit.unwrap_or(default_limit),
    )
    .map_err(|message| bad_request(message).into())
}

pub fn order_lookup(query: &OrderQuery, default_limit: u32) -> Result<OrderLookup> {
    catalog::order_lookup_from_parts(
        query.region.as_deref(),
        query.status.as_deref(),
        query.page.unwrap_or(1),
        query.limit.unwrap_or(default_limit),
    )
    .map_err(|message| bad_request(message).into())
}

pub fn page_limit(page: Option<u32>, limit: Option<u32>, default_limit: u32) -> Result<(u32, u32)> {
    catalog::normalize_window(page.unwrap_or(1), limit.unwrap_or(default_limit))
        .map_err(|message| bad_request(message).into())
}
