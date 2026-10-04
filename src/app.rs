mod api;
mod bench;
mod catalog;

use catalog_bench::catalog::{Catalog, format_bytes};
use topcoat::{
    Result,
    asset::{AssetBundle, RouterBuilderAssetExt},
    context::{Cx, app_context},
    router::{Router, RouterBuilderDiscoverExt, Slot, href, layout, module_router, page},
    runtime::RouterBuilderRuntimeExt,
    tailwind,
    view::{View, view},
};

pub fn router(catalog: Catalog) -> Router {
    module_router!()
        .assets(AssetBundle::load().expect(
            "asset bundle missing; run `topcoat asset bundle --bin catalog-bench`",
        ))
        .discover()
        .runtime()
        .app_context(catalog)
        .build()
}

#[layout]
async fn root(slot: Slot<'_>) -> Result<impl View> {
    Ok(view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8">
                <meta name="viewport" content="width=device-width, initial-scale=1">
                <title>"Catalog bench"</title>
                <link rel="stylesheet" href=(tailwind::stylesheet!())>
                if cfg!(debug_assertions) {
                    topcoat::dev::script()
                }
                topcoat::runtime::script()
            </head>
            <body class="min-h-screen bg-stone-950 text-stone-100">
                <header class="border-b border-stone-800">
                    <nav class="mx-auto flex max-w-6xl items-center gap-6 px-6 py-4">
                        <a href=(href!(page)) class="text-lg font-semibold tracking-tight text-amber-400">
                            "Catalog bench"
                        </a>
                        <a href=(href!(catalog::browse)) class="text-sm text-stone-300">"Catalog"</a>
                        <a href=(href!(bench::overview)) class="text-sm text-stone-300">"Bench"</a>
                    </nav>
                </header>
                <main class="mx-auto max-w-6xl px-6 py-10">
                    (slot)
                </main>
            </body>
        </html>
    })
}

#[page]
async fn page(cx: &Cx) -> Result<impl View> {
    let meta = app_context::<Catalog>(cx).meta();
    Ok(view! {
        <h1 class="text-3xl font-semibold tracking-tight">"In-memory catalog"</h1>
        <p class="mt-3 max-w-2xl text-stone-400">
            "Products and orders stay in one shared catalog. Each response is a short page, so the working set can stay near 1 GiB while many clients query it."
        </p>
        <dl class="mt-8 grid grid-cols-2 gap-4 md:grid-cols-4">
            <div class="rounded-lg border border-stone-800 bg-stone-900 px-4 py-3">
                <dt class="text-xs uppercase tracking-wide text-stone-500">"Resident"</dt>
                <dd class="mt-1 text-lg">(format_bytes(meta.resident_bytes))</dd>
            </div>
            <div class="rounded-lg border border-stone-800 bg-stone-900 px-4 py-3">
                <dt class="text-xs uppercase tracking-wide text-stone-500">"Products"</dt>
                <dd class="mt-1 text-lg">(meta.products)</dd>
            </div>
            <div class="rounded-lg border border-stone-800 bg-stone-900 px-4 py-3">
                <dt class="text-xs uppercase tracking-wide text-stone-500">"Orders"</dt>
                <dd class="mt-1 text-lg">(meta.orders)</dd>
            </div>
            <div class="rounded-lg border border-stone-800 bg-stone-900 px-4 py-3">
                <dt class="text-xs uppercase tracking-wide text-stone-500">"Build"</dt>
                <dd class="mt-1 text-lg">(meta.index_build_ms) " ms"</dd>
            </div>
        </dl>
        <h2 class="mt-10 text-lg font-medium">"JSON APIs"</h2>
        <ul class="mt-3 space-y-1 text-sm text-stone-300">
            <li><a class="text-amber-400" href="/api/health">"GET /api/health"</a></li>
            <li><a class="text-amber-400" href="/api/meta">"GET /api/meta"</a></li>
            <li><a class="text-amber-400" href="/api/products?limit=2">"GET /api/products"</a></li>
            <li>"GET /api/products/{id}"</li>
            <li><a class="text-amber-400" href="/api/orders?limit=2">"GET /api/orders"</a></li>
            <li>"GET /api/orders/{id}"</li>
            <li>"GET /api/customers/{id}/orders"</li>
            <li><a class="text-amber-400" href="/api/categories">"GET /api/categories"</a></li>
            <li><a class="text-amber-400" href="/api/regions/us-east/stats">"GET /api/regions/{region}/stats"</a></li>
            <li><a class="text-amber-400" href="/api/search?q=amber&limit=2">"GET /api/search"</a></li>
        </ul>
    })
}
