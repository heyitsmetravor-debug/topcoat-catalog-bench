use catalog_bench::catalog::{Catalog, format_bytes};
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::page,
    view::{View, view},
};

#[page]
pub async fn overview(cx: &Cx) -> Result<impl View> {
    let meta = app_context::<Catalog>(cx).meta();
    Ok(view! {
        <h1 class="text-3xl font-semibold tracking-tight">"Bench"</h1>
        <p class="mt-3 max-w-2xl text-stone-400">
            "The load tool opens keep-alive connections and mixes id lookups, filtered pages, and word search. It prints requests per second and p50, p95, and p99 latency."
        </p>
        <dl class="mt-8 grid grid-cols-2 gap-4 md:grid-cols-3">
            <div class="rounded-lg border border-stone-800 bg-stone-900 px-4 py-3">
                <dt class="text-xs uppercase tracking-wide text-stone-500">"Target"</dt>
                <dd class="mt-1 text-lg">(format_bytes(meta.target_bytes))</dd>
            </div>
            <div class="rounded-lg border border-stone-800 bg-stone-900 px-4 py-3">
                <dt class="text-xs uppercase tracking-wide text-stone-500">"Resident"</dt>
                <dd class="mt-1 text-lg">(format_bytes(meta.resident_bytes))</dd>
            </div>
            <div class="rounded-lg border border-stone-800 bg-stone-900 px-4 py-3">
                <dt class="text-xs uppercase tracking-wide text-stone-500">"Customers"</dt>
                <dd class="mt-1 text-lg">(meta.customers)</dd>
            </div>
        </dl>
        <h2 class="mt-10 text-lg font-medium">"Run against this server"</h2>
        <pre class="mt-3 overflow-x-auto rounded-lg bg-stone-900 p-4 text-sm text-stone-200">
            "cargo run --release --bin load -- --concurrency 200 --duration 10"
        </pre>
        <p class="mt-4 max-w-2xl text-sm text-stone-400">
            "The mix is 70% product id lookups, 20% filtered product pages, and 10% search. The default concurrency is 256. --concurrency 10000 is accepted. The catalog is shared and has no lock on the read path, so those connections do not copy the dataset. How many sockets stay open still depends on the operating system."
        </p>
        <h2 class="mt-8 text-lg font-medium">"Sample queries"</h2>
        <ul class="mt-3 space-y-1 text-sm text-amber-400">
            <li><a href="/api/products/0">"/api/products/0"</a></li>
            <li><a href="/api/products?category=hardware&region=us-east&limit=20">"/api/products?category=hardware&region=us-east&limit=20"</a></li>
            <li><a href="/api/search?q=amber&limit=20">"/api/search?q=amber&limit=20"</a></li>
            <li><a href="/api/orders?status=paid&limit=20">"/api/orders?status=paid&limit=20"</a></li>
            <li><a href="/api/customers/0/orders?limit=20">"/api/customers/0/orders?limit=20"</a></li>
        </ul>
    })
}
