use catalog_bench::catalog::{Catalog, OrderRow};
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{content::Json, error::RouterErrorExt, module_param, path_param, route},
};

module_param!(id: u32, error = bad_request("order id must be a number"));

#[route(GET)]
async fn show(cx: &Cx) -> Result<Json<OrderRow>> {
    let id = *path_param::<Id>(cx)?;
    let row = app_context::<Catalog>(cx).order(id).ok_or_not_found()?;
    Ok(Json(row))
}
