use catalog_bench::catalog::{
    Catalog, CATEGORIES, REGIONS, format_cents, parse_optional_u32, product_lookup_from_parts,
};
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::page,
    runtime::{Event, shard, signal},
    view::{View, view},
};

#[page]
pub async fn browse(cx: &Cx) -> Result<impl View> {
    let q = signal(cx, String::new);
    let category = signal(cx, String::new);
    let region = signal(cx, String::new);
    let min_price = signal(cx, String::new);
    let max_price = signal(cx, String::new);
    let page_no = signal(cx, || 1usize);

    Ok(view! {
        <h1 class="text-3xl font-semibold tracking-tight">"Catalog"</h1>
        <p class="mt-2 max-w-2xl text-sm text-stone-400">
            "Filters rerender the table on the server. Prices are whole cents. Name search matches a full word such as amber or maple."
        </p>
        <div class="mt-6 grid gap-4 md:grid-cols-3">
            <label class="block text-sm">
                "Name"
                <input
                    class="mt-1 w-full rounded border border-stone-700 bg-stone-900 px-3 py-2"
                    :value=$(q.get())
                    @input=$(|e: Event| { q.set(e.target.value); page_no.set(1); })
                >
            </label>
            <label class="block text-sm">
                "Category"
                <select
                    class="mt-1 w-full rounded border border-stone-700 bg-stone-900 px-3 py-2"
                    :value=$(category.get())
                    @change=$(|e: Event| { category.set(e.target.value); page_no.set(1); })
                >
                    <option value="">"Any"</option>
                    for name in CATEGORIES {
                        <option value=(name)>(name)</option>
                    }
                </select>
            </label>
            <label class="block text-sm">
                "Region"
                <select
                    class="mt-1 w-full rounded border border-stone-700 bg-stone-900 px-3 py-2"
                    :value=$(region.get())
                    @change=$(|e: Event| { region.set(e.target.value); page_no.set(1); })
                >
                    <option value="">"Any"</option>
                    for name in REGIONS {
                        <option value=(name)>(name)</option>
                    }
                </select>
            </label>
            <label class="block text-sm">
                "Min cents"
                <input
                    class="mt-1 w-full rounded border border-stone-700 bg-stone-900 px-3 py-2"
                    :value=$(min_price.get())
                    @input=$(|e: Event| { min_price.set(e.target.value); page_no.set(1); })
                >
            </label>
            <label class="block text-sm">
                "Max cents"
                <input
                    class="mt-1 w-full rounded border border-stone-700 bg-stone-900 px-3 py-2"
                    :value=$(max_price.get())
                    @input=$(|e: Event| { max_price.set(e.target.value); page_no.set(1); })
                >
            </label>
        </div>
        <p class="mt-4 text-sm text-stone-400">"Page " $(page_no.get())</p>
        <div class="mt-3 flex gap-3">
            <button
                type="button"
                class="rounded bg-stone-800 px-3 py-2 text-sm"
                @click=$(|_e| page_no.set(if page_no.get() > 1 { page_no.get() - 1 } else { 1 }))
            >
                "Previous"
            </button>
            <button
                type="button"
                class="rounded bg-amber-500 px-3 py-2 text-sm font-medium text-stone-950"
                @click=$(|_e| page_no.set(page_no.get() + 1))
            >
                "Next"
            </button>
        </div>
        product_results(
            q: $(q.get()),
            category: $(category.get()),
            region: $(region.get()),
            min_price: $(min_price.get()),
            max_price: $(max_price.get()),
            page_no: $(page_no.get()),
        )
    })
}

#[shard]
async fn product_results(
    cx: &Cx,
    q: String,
    category: String,
    region: String,
    min_price: String,
    max_price: String,
    page_no: usize,
) -> Result<impl View> {
    let parsed = parse_optional_u32(&min_price).and_then(|min_price| {
        parse_optional_u32(&max_price).and_then(|max_price| {
            let page = u32::try_from(page_no).unwrap_or(u32::MAX).max(1);
            product_lookup_from_parts(
                Some(q.as_str()),
                Some(category.as_str()),
                Some(region.as_str()),
                min_price,
                max_price,
                page,
                50,
            )
        })
    });
    let (message, total, rows) = match parsed {
        Err(message) => (message.to_owned(), 0, Vec::new()),
        Ok(lookup) => {
            let page = app_context::<Catalog>(cx).query_products(&lookup);
            (String::new(), page.total, page.items)
        }
    };
    Ok(view! {
        if !message.is_empty() {
            <p class="mt-6 text-red-400">(message)</p>
        } else {
            <p class="mt-6 text-sm text-stone-400">
                (total)
                " matches"
            </p>
            if rows.is_empty() {
                <p class="mt-4 text-stone-300">"No rows on this page."</p>
            } else {
                <table class="mt-4 w-full text-left text-sm">
                    <thead>
                        <tr class="border-b border-stone-800 text-stone-500">
                            <th class="py-2 pr-4 font-medium">"Id"</th>
                            <th class="py-2 pr-4 font-medium">"Name"</th>
                            <th class="py-2 pr-4 font-medium">"Category"</th>
                            <th class="py-2 pr-4 font-medium">"Region"</th>
                            <th class="py-2 pr-4 font-medium">"Price"</th>
                            <th class="py-2 pr-4 font-medium">"Stock"</th>
                            <th class="py-2 font-medium">"Rating"</th>
                        </tr>
                    </thead>
                    <tbody>
                        for item in rows {
                            <tr class="border-b border-stone-900">
                                <td class="py-2 pr-4">(item.id)</td>
                                <td class="py-2 pr-4" title=(item.blurb)>(item.name)</td>
                                <td class="py-2 pr-4">(item.category)</td>
                                <td class="py-2 pr-4">(item.region)</td>
                                <td class="py-2 pr-4">(format_cents(item.price_cents))</td>
                                <td class="py-2 pr-4">(item.stock)</td>
                                <td class="py-2">(format!("{:.2}", item.rating as f32 / 100.0))</td>
                            </tr>
                        }
                    </tbody>
                </table>
            }
        }
    })
}
