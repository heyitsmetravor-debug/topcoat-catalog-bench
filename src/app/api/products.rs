mod id;

use catalog_bench::catalog::{Catalog, DEFAULT_PRODUCT_LIMIT, ProductPage};
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{content::Json, query_params, route},
};

use super::{ProductQuery, product_lookup};

#[route(GET)]
async fn list(cx: &Cx) -> Result<Json<ProductPage>> {
    let query = query_params::<ProductQuery>(cx)?;
    let lookup = product_lookup(query, DEFAULT_PRODUCT_LIMIT)?;
    Ok(Json(app_context::<Catalog>(cx).query_products(&lookup)))
}
