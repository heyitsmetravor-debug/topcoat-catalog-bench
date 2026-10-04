use catalog_bench::catalog::{Catalog, Meta};
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{content::Json, route},
};

#[route(GET)]
async fn meta(cx: &Cx) -> Result<Json<Meta>> {
    Ok(Json(app_context::<Catalog>(cx).meta()))
}
