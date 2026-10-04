use catalog_bench::catalog::{Catalog, ProductRow};
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{content::Json, error::RouterErrorExt, module_param, path_param, route},
};

module_param!(id: u32, error = bad_request("product id must be a number"));

#[route(GET)]
async fn show(cx: &Cx) -> Result<Json<ProductRow>> {
    let id = *path_param::<Id>(cx)?;
    let row = app_context::<Catalog>(cx).product(id).ok_or_not_found()?;
    Ok(Json(row))
}
