mod app;

use catalog_bench::catalog::Catalog;

#[tokio::main]
async fn main() {
    eprintln!("building catalog...");
    let catalog = Catalog::build();
    let meta = catalog.meta();
    eprintln!(
        "catalog ready in {} ms ({} bytes, {} products, {} orders)",
        meta.index_build_ms, meta.resident_bytes, meta.products, meta.orders,
    );
    topcoat::start(app::router(catalog)).await.unwrap();
}
