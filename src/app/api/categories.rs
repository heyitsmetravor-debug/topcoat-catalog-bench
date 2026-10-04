use catalog_bench::catalog::{Catalog, CategoryReport};
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{content::Json, route},
};

#[route(GET)]
async fn categories(cx: &Cx) -> Result<Json<Vec<CategoryReport>>> {
    Ok(Json(app_context::<Catalog>(cx).categories()))
}
