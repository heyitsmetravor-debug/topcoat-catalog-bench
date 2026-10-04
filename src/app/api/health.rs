use serde::Serialize;
use topcoat::{
    Result,
    router::{content::Json, route},
};

#[derive(Serialize)]
struct Health {
    status: &'static str,
}

#[route(GET)]
async fn health() -> Result<Json<Health>> {
    Ok(Json(Health { status: "ok" }))
}
