mod id;

use catalog_bench::catalog::{Catalog, DEFAULT_ORDER_LIMIT, OrderPage};
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{content::Json, query_params, route},
};

use super::{OrderQuery, order_lookup};

#[route(GET)]
async fn list(cx: &Cx) -> Result<Json<OrderPage>> {
    let query = query_params::<OrderQuery>(cx)?;
    let lookup = order_lookup(query, DEFAULT_ORDER_LIMIT)?;
    Ok(Json(app_context::<Catalog>(cx).query_orders(&lookup)))
}
