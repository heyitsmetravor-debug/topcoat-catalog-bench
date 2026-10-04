use catalog_bench::catalog::{Catalog, OrderPage};
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{content::Json, error::RouterErrorExt, module_param, path_param, query_params, route},
};

use super::super::{PageQuery, page_limit};

module_param!(id: u32, error = bad_request("customer id must be a number"));

#[route(GET "./orders")]
async fn orders(cx: &Cx) -> Result<Json<OrderPage>> {
    let id = *path_param::<Id>(cx)?;
    let query = query_params::<PageQuery>(cx)?;
    let (page, limit) = page_limit(query.page, query.limit, 50)?;
    let rows = app_context::<Catalog>(cx)
        .customer_orders(id, page, limit)
        .ok_or_not_found()?;
    Ok(Json(rows))
}
