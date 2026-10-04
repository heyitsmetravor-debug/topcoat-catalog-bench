use catalog_bench::catalog::{Catalog, RegionReport};
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{content::Json, error::RouterErrorExt, module_param, path_param, route},
};

module_param!(region);

#[route(GET "./stats")]
async fn stats(cx: &Cx) -> Result<Json<RegionReport>> {
    let region = path_param::<Region>(cx);
    let report = app_context::<Catalog>(cx)
        .region_report(region)
        .ok_or_not_found()?;
    Ok(Json(report))
}
