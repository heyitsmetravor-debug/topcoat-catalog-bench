use std::mem::size_of;
use std::time::Instant;

use serde::{Deserialize, Serialize};

pub const WORDS: [&str; 24] = [
    "amber", "birch", "cedar", "drift", "ember", "flint", "grove", "harbor", "ivory", "juniper",
    "kelp", "linen", "maple", "north", "onyx", "pearl", "quartz", "ridge", "spruce", "timber",
    "umbra", "violet", "willow", "yarrow",
];

pub const CATEGORIES: [&str; 8] = [
    "hardware", "software", "office", "outdoor", "kitchen", "audio", "lighting", "tools",
];

pub const REGIONS: [&str; 6] = [
    "us-east", "us-west", "eu-central", "eu-west", "ap-south", "ap-northeast",
];

pub const STATUSES: [&str; 4] = ["pending", "paid", "shipped", "cancelled"];

pub const DEFAULT_PRODUCT_LIMIT: u32 = 50;
pub const DEFAULT_SEARCH_LIMIT: u32 = 20;
pub const DEFAULT_ORDER_LIMIT: u32 = 50;
const MAX_LIMIT: u32 = 100;
const BLURB: &str = "Packed for benchmark queries. ";

#[derive(Clone, Serialize, Deserialize)]
pub struct Meta {
    pub target_bytes: u64,
    pub resident_bytes: u64,
    pub products: u64,
    pub orders: u64,
    pub customers: u64,
    pub categories: u64,
    pub regions: u64,
    pub index_build_ms: u64,
}

#[derive(Serialize)]
pub struct ProductRow {
    pub id: u32,
    pub name: String,
    pub blurb: String,
    pub category: &'static str,
    pub region: &'static str,
    pub price_cents: u32,
    pub stock: u32,
    pub rating: u16,
    pub created: u32,
}

#[derive(Serialize)]
pub struct ProductPage {
    pub page: u32,
    pub limit: u32,
    pub total: u64,
    pub items: Vec<ProductRow>,
}

#[derive(Serialize)]
pub struct OrderRow {
    pub id: u32,
    pub customer_id: u32,
    pub product_id: u32,
    pub region: &'static str,
    pub status: &'static str,
    pub amount_cents: u32,
    pub created: u32,
}

#[derive(Serialize)]
pub struct OrderPage {
    pub page: u32,
    pub limit: u32,
    pub total: u64,
    pub items: Vec<OrderRow>,
}

#[derive(Serialize)]
pub struct CategoryReport {
    pub name: &'static str,
    pub products: u64,
}

#[derive(Serialize)]
pub struct RegionReport {
    pub region: &'static str,
    pub products: u64,
    pub orders: u64,
    pub revenue_cents: u64,
}

pub struct ProductLookup {
    pub words: Vec<usize>,
    pub unknown_word: bool,
    pub category: Option<usize>,
    pub region: Option<usize>,
    pub min_price: Option<u32>,
    pub max_price: Option<u32>,
    pub page: u32,
    pub limit: u32,
}

pub struct OrderLookup {
    pub region: Option<usize>,
    pub status: Option<usize>,
    pub page: u32,
    pub limit: u32,
}

struct Product {
    price_cents: u32,
    stock: u32,
    created: u32,
    name_off: u32,
    blurb_off: u32,
    category: u16,
    region: u16,
    rating: u16,
    name_len: u16,
    blurb_len: u16,
    _pad: u16,
}

struct Order {
    customer_id: u32,
    product_id: u32,
    amount_cents: u32,
    created: u32,
    region: u16,
    status: u16,
}

struct RegionTally {
    products: u64,
    orders: u64,
    revenue_cents: u64,
}

#[derive(Clone, Copy)]
enum Candidate {
    Empty,
    All,
    Category(usize),
    Region(usize),
    Price(usize, usize),
    Word(usize),
}

pub struct Catalog {
    products: Vec<Product>,
    orders: Vec<Order>,
    arena: String,
    pad: Vec<u8>,
    by_category: Vec<Vec<u32>>,
    by_region: Vec<Vec<u32>>,
    by_price: Vec<u32>,
    postings: Vec<Vec<u32>>,
    orders_by_customer: Vec<Vec<u32>>,
    orders_by_region: Vec<Vec<u32>>,
    orders_by_status: Vec<Vec<u32>>,
    region_stats: Vec<RegionTally>,
    target_bytes: usize,
    resident_bytes: usize,
    index_build_ms: u64,
}

impl Catalog {
    pub fn build() -> Self {
        Self::with_target_bytes(configured_target_bytes())
    }

    pub fn with_target_bytes(target: usize) -> Self {
        let started = Instant::now();
        let (products_n, orders_n) = plan_counts(target);
        let customers_n = orders_n / 32;
        let mut string_budget = target / 2;
        if products_n > 0 && string_budget / products_n < 64 {
            string_budget = products_n.saturating_mul(64);
        }
        if string_budget >= target {
            string_budget = target / 2;
        }

        if target >= 64 * 1024 * 1024 {
            eprintln!(
                "catalog: {products_n} products, {orders_n} orders, {string_budget} byte string arena"
            );
        }

        let mut arena = String::with_capacity(string_budget);
        let mut products = Vec::with_capacity(products_n);
        let mut by_category = vec![Vec::new(); CATEGORIES.len()];
        let mut by_region = vec![Vec::new(); REGIONS.len()];
        let mut postings = vec![Vec::new(); WORDS.len()];
        let mut region_stats = Vec::with_capacity(REGIONS.len());
        region_stats.resize_with(REGIONS.len(), || RegionTally {
            products: 0,
            orders: 0,
            revenue_cents: 0,
        });

        let per_category = products_n / CATEGORIES.len() + 1;
        let per_region = products_n / REGIONS.len() + 1;
        let per_word = products_n / WORDS.len() * 3 + 8;
        for group in &mut by_category {
            group.reserve(per_category);
        }
        for group in &mut by_region {
            group.reserve(per_region);
        }
        for group in &mut postings {
            group.reserve(per_word);
        }

        let slot = string_budget / products_n;
        for id in 0..products_n {
            let budget = if id + 1 == products_n {
                string_budget - arena.len()
            } else {
                slot
            };
            let (name_off, name_len, blurb_off, blurb_len) = write_slot(&mut arena, id as u32, budget);
            let category = (id % CATEGORIES.len()) as u16;
            let region = ((id / 7) % REGIONS.len()) as u16;
            let price_cents = price_of(id as u32);
            products.push(Product {
                price_cents,
                stock: 1 + (id as u32 % 500),
                created: 1_700_000_000 + (id as u32 % 50_000_000),
                name_off,
                blurb_off,
                category,
                region,
                rating: (100 + (id as u32 % 401)) as u16,
                name_len,
                blurb_len,
                _pad: 0,
            });
            by_category[category as usize].push(id as u32);
            by_region[region as usize].push(id as u32);
            region_stats[region as usize].products += 1;
            let mut seen = [false; WORDS.len()];
            for word in name_words(id as u32) {
                if !seen[word] {
                    seen[word] = true;
                    postings[word].push(id as u32);
                }
            }
        }

        let mut by_price: Vec<u32> = (0..products.len() as u32).collect();
        by_price.sort_unstable_by_key(|id| products[*id as usize].price_cents);

        let mut orders = Vec::with_capacity(orders_n);
        let mut orders_by_customer = vec![Vec::new(); customers_n];
        let mut orders_by_region = vec![Vec::new(); REGIONS.len()];
        let mut orders_by_status = vec![Vec::new(); STATUSES.len()];
        for group in &mut orders_by_customer {
            group.reserve(32);
        }
        let per_order_region = orders_n / REGIONS.len() + 1;
        for group in &mut orders_by_region {
            group.reserve(per_order_region);
        }
        let per_status = orders_n / STATUSES.len();
        for group in &mut orders_by_status {
            group.reserve(per_status);
        }

        for id in 0..orders_n {
            let product_id = (id % products_n) as u32;
            let product = &products[product_id as usize];
            let customer_id = (id % customers_n) as u32;
            let status = (id % STATUSES.len()) as u16;
            let region = product.region;
            let amount_cents = product.price_cents;
            orders.push(Order {
                customer_id,
                product_id,
                amount_cents,
                created: product.created,
                region,
                status,
            });
            orders_by_customer[customer_id as usize].push(id as u32);
            orders_by_region[region as usize].push(id as u32);
            orders_by_status[status as usize].push(id as u32);
            let tally = &mut region_stats[region as usize];
            tally.orders += 1;
            tally.revenue_cents += u64::from(amount_cents);
        }

        let mut catalog = Self {
            products,
            orders,
            arena,
            pad: Vec::new(),
            by_category,
            by_region,
            by_price,
            postings,
            orders_by_customer,
            orders_by_region,
            orders_by_status,
            region_stats,
            target_bytes: target,
            resident_bytes: 0,
            index_build_ms: 0,
        };
        catalog.shrink();
        let measured = catalog.measured();
        if measured < target {
            catalog.pad = vec![b'a'; target - measured];
        }
        catalog.resident_bytes = catalog.measured();
        catalog.index_build_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        catalog
    }

    pub fn meta(&self) -> Meta {
        Meta {
            target_bytes: self.target_bytes as u64,
            resident_bytes: self.resident_bytes as u64,
            products: self.products.len() as u64,
            orders: self.orders.len() as u64,
            customers: self.orders_by_customer.len() as u64,
            categories: CATEGORIES.len() as u64,
            regions: REGIONS.len() as u64,
            index_build_ms: self.index_build_ms,
        }
    }

    pub fn product(&self, id: u32) -> Option<ProductRow> {
        self.products.get(id as usize)?;
        Some(self.product_row(id))
    }

    pub fn order(&self, id: u32) -> Option<OrderRow> {
        self.orders.get(id as usize)?;
        Some(self.order_row(id))
    }

    pub fn query_products(&self, lookup: &ProductLookup) -> ProductPage {
        let candidate = self.product_candidate(lookup);
        if self.candidate_is_exact(candidate, lookup) {
            return match candidate {
                Candidate::Empty => empty_products(lookup),
                Candidate::All => self.slice_all_products(lookup),
                Candidate::Category(index) => self.slice_products(&self.by_category[index], lookup),
                Candidate::Region(index) => self.slice_products(&self.by_region[index], lookup),
                Candidate::Price(lo, hi) => self.slice_products(&self.by_price[lo..hi], lookup),
                Candidate::Word(word) => self.slice_products(&self.postings[word], lookup),
            };
        }
        let ids: &[u32] = match candidate {
            Candidate::Empty => &[],
            Candidate::All => &[],
            Candidate::Category(index) => &self.by_category[index],
            Candidate::Region(index) => &self.by_region[index],
            Candidate::Price(lo, hi) => &self.by_price[lo..hi],
            Candidate::Word(word) => &self.postings[word],
        };
        if matches!(candidate, Candidate::All) {
            return self.scan_all_products(lookup);
        }
        self.scan_product_ids(ids, lookup)
    }

    pub fn query_orders(&self, lookup: &OrderLookup) -> OrderPage {
        match (lookup.region, lookup.status) {
            (None, None) => self.slice_all_orders(lookup),
            (Some(region), None) => self.slice_orders(&self.orders_by_region[region], lookup),
            (None, Some(status)) => self.slice_orders(&self.orders_by_status[status], lookup),
            (Some(region), Some(status)) => {
                let region_ids = &self.orders_by_region[region];
                let status_ids = &self.orders_by_status[status];
                if region_ids.len() <= status_ids.len() {
                    self.scan_orders(region_ids, lookup, true)
                } else {
                    self.scan_orders(status_ids, lookup, false)
                }
            }
        }
    }

    pub fn customer_orders(&self, customer_id: u32, page: u32, limit: u32) -> Option<OrderPage> {
        let ids = self.orders_by_customer.get(customer_id as usize)?;
        Some(self.slice_orders(ids, &OrderLookup {
            region: None,
            status: None,
            page,
            limit,
        }))
    }

    pub fn categories(&self) -> Vec<CategoryReport> {
        CATEGORIES
            .iter()
            .enumerate()
            .map(|(index, name)| CategoryReport {
                name,
                products: self.by_category[index].len() as u64,
            })
            .collect()
    }

    pub fn region_report(&self, name: &str) -> Option<RegionReport> {
        let index = REGIONS.iter().position(|region| *region == name)?;
        let tally = &self.region_stats[index];
        Some(RegionReport {
            region: REGIONS[index],
            products: tally.products,
            orders: tally.orders,
            revenue_cents: tally.revenue_cents,
        })
    }

    fn product_candidate(&self, lookup: &ProductLookup) -> Candidate {
        if lookup.unknown_word {
            return Candidate::Empty;
        }
        if lookup.category.is_none()
            && lookup.region.is_none()
            && lookup.min_price.is_none()
            && lookup.max_price.is_none()
            && lookup.words.is_empty()
        {
            return Candidate::All;
        }

        let mut best_len = usize::MAX;
        let mut best = Candidate::Empty;
        let mut consider = |len: usize, candidate: Candidate| {
            if len < best_len {
                best_len = len;
                best = candidate;
            }
        };
        if let Some(category) = lookup.category {
            consider(self.by_category[category].len(), Candidate::Category(category));
        }
        if let Some(region) = lookup.region {
            consider(self.by_region[region].len(), Candidate::Region(region));
        }
        if lookup.min_price.is_some() || lookup.max_price.is_some() {
            let (lo, hi) = self.price_span(lookup.min_price, lookup.max_price);
            consider(hi.saturating_sub(lo), Candidate::Price(lo, hi));
        }
        if let Some(word) = lookup
            .words
            .iter()
            .copied()
            .min_by_key(|word| self.postings[*word].len())
        {
            consider(self.postings[word].len(), Candidate::Word(word));
        }
        best
    }

    fn candidate_is_exact(&self, candidate: Candidate, lookup: &ProductLookup) -> bool {
        let no_price = lookup.min_price.is_none() && lookup.max_price.is_none();
        let no_words = lookup.words.is_empty();
        match candidate {
            Candidate::Empty => true,
            Candidate::All => {
                lookup.category.is_none() && lookup.region.is_none() && no_price && no_words
            }
            Candidate::Category(_) => lookup.region.is_none() && no_price && no_words,
            Candidate::Region(_) => lookup.category.is_none() && no_price && no_words,
            Candidate::Price(_, _) => lookup.category.is_none() && lookup.region.is_none() && no_words,
            Candidate::Word(_) => {
                lookup.words.len() == 1
                    && lookup.category.is_none()
                    && lookup.region.is_none()
                    && no_price
            }
        }
    }

    fn price_span(&self, min: Option<u32>, max: Option<u32>) -> (usize, usize) {
        let lo = match min {
            Some(min) => self
                .by_price
                .partition_point(|id| self.products[*id as usize].price_cents < min),
            None => 0,
        };
        let hi = match max {
            Some(max) => self
                .by_price
                .partition_point(|id| self.products[*id as usize].price_cents <= max),
            None => self.by_price.len(),
        };
        if hi < lo { (0, 0) } else { (lo, hi) }
    }

    fn product_matches(&self, id: u32, lookup: &ProductLookup) -> bool {
        let product = &self.products[id as usize];
        if let Some(category) = lookup.category {
            if product.category as usize != category {
                return false;
            }
        }
        if let Some(region) = lookup.region {
            if product.region as usize != region {
                return false;
            }
        }
        if let Some(min) = lookup.min_price {
            if product.price_cents < min {
                return false;
            }
        }
        if let Some(max) = lookup.max_price {
            if product.price_cents > max {
                return false;
            }
        }
        if !lookup.words.is_empty() {
            let parts = name_words(id);
            if !lookup.words.iter().all(|word| parts.contains(word)) {
                return false;
            }
        }
        true
    }

    fn slice_all_products(&self, lookup: &ProductLookup) -> ProductPage {
        let (start, end) = window(lookup.page, lookup.limit, self.products.len());
        let items = (start..end).map(|id| self.product_row(id as u32)).collect();
        ProductPage {
            page: lookup.page,
            limit: lookup.limit,
            total: self.products.len() as u64,
            items,
        }
    }

    fn slice_products(&self, ids: &[u32], lookup: &ProductLookup) -> ProductPage {
        let (start, end) = window(lookup.page, lookup.limit, ids.len());
        let items = ids[start..end].iter().map(|id| self.product_row(*id)).collect();
        ProductPage {
            page: lookup.page,
            limit: lookup.limit,
            total: ids.len() as u64,
            items,
        }
    }

    fn scan_all_products(&self, lookup: &ProductLookup) -> ProductPage {
        let start = (lookup.page as usize)
            .saturating_sub(1)
            .saturating_mul(lookup.limit as usize);
        let limit = lookup.limit as usize;
        let mut total = 0u64;
        let mut items = Vec::new();
        for id in 0..self.products.len() as u32 {
            if !self.product_matches(id, lookup) {
                continue;
            }
            if total as usize >= start && items.len() < limit {
                items.push(self.product_row(id));
            }
            total += 1;
        }
        ProductPage {
            page: lookup.page,
            limit: lookup.limit,
            total,
            items,
        }
    }

    fn scan_product_ids(&self, ids: &[u32], lookup: &ProductLookup) -> ProductPage {
        let start = (lookup.page as usize)
            .saturating_sub(1)
            .saturating_mul(lookup.limit as usize);
        let limit = lookup.limit as usize;
        let mut total = 0u64;
        let mut items = Vec::new();
        for id in ids {
            if !self.product_matches(*id, lookup) {
                continue;
            }
            if total as usize >= start && items.len() < limit {
                items.push(self.product_row(*id));
            }
            total += 1;
        }
        ProductPage {
            page: lookup.page,
            limit: lookup.limit,
            total,
            items,
        }
    }

    fn slice_all_orders(&self, lookup: &OrderLookup) -> OrderPage {
        let (start, end) = window(lookup.page, lookup.limit, self.orders.len());
        let items = (start..end).map(|id| self.order_row(id as u32)).collect();
        OrderPage {
            page: lookup.page,
            limit: lookup.limit,
            total: self.orders.len() as u64,
            items,
        }
    }

    fn slice_orders(&self, ids: &[u32], lookup: &OrderLookup) -> OrderPage {
        let (start, end) = window(lookup.page, lookup.limit, ids.len());
        let items = ids[start..end].iter().map(|id| self.order_row(*id)).collect();
        OrderPage {
            page: lookup.page,
            limit: lookup.limit,
            total: ids.len() as u64,
            items,
        }
    }

    fn scan_orders(&self, ids: &[u32], lookup: &OrderLookup, base_is_region: bool) -> OrderPage {
        let start = (lookup.page as usize)
            .saturating_sub(1)
            .saturating_mul(lookup.limit as usize);
        let limit = lookup.limit as usize;
        let mut total = 0u64;
        let mut items = Vec::new();
        for id in ids {
            let order = &self.orders[*id as usize];
            if base_is_region {
                if let Some(status) = lookup.status {
                    if order.status as usize != status {
                        continue;
                    }
                }
            } else if let Some(region) = lookup.region {
                if order.region as usize != region {
                    continue;
                }
            }
            if total as usize >= start && items.len() < limit {
                items.push(self.order_row(*id));
            }
            total += 1;
        }
        OrderPage {
            page: lookup.page,
            limit: lookup.limit,
            total,
            items,
        }
    }

    fn product_row(&self, id: u32) -> ProductRow {
        let product = &self.products[id as usize];
        let name_start = product.name_off as usize;
        let name = self.arena[name_start..name_start + product.name_len as usize].to_owned();
        let blurb_start = product.blurb_off as usize;
        let blurb_len = (product.blurb_len as usize).min(80);
        let blurb = self.arena[blurb_start..blurb_start + blurb_len].to_owned();
        ProductRow {
            id,
            name,
            blurb,
            category: CATEGORIES[product.category as usize],
            region: REGIONS[product.region as usize],
            price_cents: product.price_cents,
            stock: product.stock,
            rating: product.rating,
            created: product.created,
        }
    }

    fn order_row(&self, id: u32) -> OrderRow {
        let order = &self.orders[id as usize];
        OrderRow {
            id,
            customer_id: order.customer_id,
            product_id: order.product_id,
            region: REGIONS[order.region as usize],
            status: STATUSES[order.status as usize],
            amount_cents: order.amount_cents,
            created: order.created,
        }
    }

    fn shrink(&mut self) {
        self.products.shrink_to_fit();
        self.orders.shrink_to_fit();
        self.arena.shrink_to_fit();
        self.by_price.shrink_to_fit();
        shrink_groups(&mut self.by_category);
        shrink_groups(&mut self.by_region);
        shrink_groups(&mut self.postings);
        shrink_groups(&mut self.orders_by_customer);
        shrink_groups(&mut self.orders_by_region);
        shrink_groups(&mut self.orders_by_status);
        self.region_stats.shrink_to_fit();
    }

    fn measured(&self) -> usize {
        vec_bytes(&self.products)
            + vec_bytes(&self.orders)
            + self.arena.capacity()
            + self.pad.capacity()
            + vec_bytes(&self.by_price)
            + nested_bytes(&self.by_category)
            + nested_bytes(&self.by_region)
            + nested_bytes(&self.postings)
            + nested_bytes(&self.orders_by_customer)
            + nested_bytes(&self.orders_by_region)
            + nested_bytes(&self.orders_by_status)
            + vec_bytes(&self.region_stats)
    }
}

#[cfg(test)]
impl ProductLookup {
    fn base(page: u32, limit: u32) -> Self {
        Self {
            words: Vec::new(),
            unknown_word: false,
            category: None,
            region: None,
            min_price: None,
            max_price: None,
            page,
            limit,
        }
    }
}

pub fn product_lookup_from_parts(
    q: Option<&str>,
    category: Option<&str>,
    region: Option<&str>,
    min_price: Option<u32>,
    max_price: Option<u32>,
    page: u32,
    limit: u32,
) -> Result<ProductLookup, &'static str> {
    let (page, limit) = normalize_window(page, limit)?;
    let category = lookup_name(category, &CATEGORIES, "unknown category")?;
    let region = lookup_name(region, &REGIONS, "unknown region")?;
    if let (Some(min), Some(max)) = (min_price, max_price) {
        if min > max {
            return Err("min_price must be less than or equal to max_price");
        }
    }
    let (words, unknown_word) = match q.map(str::trim).filter(|value| !value.is_empty()) {
        None => (Vec::new(), false),
        Some(q) => resolve_words(q)?,
    };
    Ok(ProductLookup {
        words,
        unknown_word,
        category,
        region,
        min_price,
        max_price,
        page,
        limit,
    })
}

pub fn order_lookup_from_parts(
    region: Option<&str>,
    status: Option<&str>,
    page: u32,
    limit: u32,
) -> Result<OrderLookup, &'static str> {
    let (page, limit) = normalize_window(page, limit)?;
    Ok(OrderLookup {
        region: lookup_name(region, &REGIONS, "unknown region")?,
        status: lookup_name(status, &STATUSES, "unknown status")?,
        page,
        limit,
    })
}

pub fn normalize_window(page: u32, limit: u32) -> Result<(u32, u32), &'static str> {
    if page == 0 {
        return Err("page must be at least 1");
    }
    if limit == 0 || limit > MAX_LIMIT {
        return Err("limit must be from 1 to 100");
    }
    Ok((page, limit))
}

pub fn parse_optional_u32(raw: &str) -> Result<Option<u32>, &'static str> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(None);
    }
    raw.parse::<u32>()
        .map(Some)
        .map_err(|_| "price filters must be whole numbers of cents")
}

pub fn format_bytes(bytes: u64) -> String {
    let mib = bytes as f64 / (1024.0 * 1024.0);
    format!("{mib:.1} MiB")
}

pub fn format_cents(cents: u32) -> String {
    format!("${}.{:02}", cents / 100, cents % 100)
}

fn configured_target_bytes() -> usize {
    configured_mb().saturating_mul(1024 * 1024)
}

fn configured_mb() -> usize {
    if let Ok(raw) = std::env::var("CATALOG_MB") {
        let mb = raw.trim().parse::<usize>().unwrap_or_else(|_| {
            panic!("CATALOG_MB must be a positive integer, got {raw}");
        });
        if mb == 0 {
            panic!("CATALOG_MB must be at least 1");
        }
        return mb;
    }
    if cfg!(debug_assertions) { 32 } else { 1024 }
}

fn plan_counts(target: usize) -> (usize, usize) {
    let fixed_per_product = size_of::<Product>()
        + 2 * size_of::<Order>()
        + 6 * size_of::<u32>()
        + 2 * 3 * size_of::<u32>();
    let mut products_n = (target * 2 / 5 / fixed_per_product.max(1)).max(64);
    products_n = (products_n / 16).max(4) * 16;
    let string_budget = target / 2;
    while products_n > 64 && string_budget / products_n < 64 {
        products_n = (products_n / 2 / 16).max(4) * 16;
    }
    (products_n, products_n * 2)
}

fn resolve_words(q: &str) -> Result<(Vec<usize>, bool), &'static str> {
    if q.chars().count() > 80 {
        return Err("q must be at most 80 characters");
    }
    let mut words = Vec::new();
    for token in q.split_whitespace() {
        let token = token.to_ascii_lowercase();
        let Some(index) = WORDS.iter().position(|word| *word == token) else {
            return Ok((Vec::new(), true));
        };
        if !words.contains(&index) {
            words.push(index);
        }
    }
    Ok((words, false))
}

fn lookup_name(
    value: Option<&str>,
    choices: &[&str],
    unknown: &'static str,
) -> Result<Option<usize>, &'static str> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    choices
        .iter()
        .position(|item| *item == value)
        .map(Some)
        .ok_or(unknown)
}

fn name_words(id: u32) -> [usize; 3] {
    let n = WORDS.len();
    let id = id as usize;
    [id % n, (id / n) % n, (id / (n * n)) % n]
}

fn price_of(id: u32) -> u32 {
    100 + (id.wrapping_mul(2_654_435_761) % 49_900)
}

fn write_slot(arena: &mut String, id: u32, budget: usize) -> (u32, u16, u32, u16) {
    let slot_start = arena.len();
    push_name(arena, id);
    let name_len_full = arena.len() - slot_start;
    if name_len_full > budget {
        arena.truncate(slot_start + budget);
        return (slot_start as u32, budget as u16, slot_start as u32, 0);
    }
    let blurb_off = arena.len() as u32;
    let blurb_budget = budget - name_len_full;
    let mut filled = 0;
    while filled < blurb_budget {
        let take = (blurb_budget - filled).min(BLURB.len());
        arena.push_str(&BLURB[..take]);
        filled += take;
    }
    debug_assert_eq!(arena.len(), slot_start + budget);
    let blurb_len = blurb_budget.min(u16::MAX as usize) as u16;
    (slot_start as u32, name_len_full as u16, blurb_off, blurb_len)
}

fn push_name(arena: &mut String, id: u32) {
    let [a, b, c] = name_words(id);
    arena.push_str(WORDS[a]);
    arena.push(' ');
    arena.push_str(WORDS[b]);
    arena.push(' ');
    arena.push_str(WORDS[c]);
}

fn window(page: u32, limit: u32, len: usize) -> (usize, usize) {
    let start = (page as usize)
        .saturating_sub(1)
        .saturating_mul(limit as usize);
    let end = start.saturating_add(limit as usize).min(len);
    if start >= len { (len, len) } else { (start, end) }
}

fn empty_products(lookup: &ProductLookup) -> ProductPage {
    ProductPage {
        page: lookup.page,
        limit: lookup.limit,
        total: 0,
        items: Vec::new(),
    }
}

fn shrink_groups(groups: &mut Vec<Vec<u32>>) {
    for group in groups.iter_mut() {
        group.shrink_to_fit();
    }
    groups.shrink_to_fit();
}

fn vec_bytes<T>(values: &Vec<T>) -> usize {
    values.capacity() * size_of::<T>()
}

fn nested_bytes(groups: &Vec<Vec<u32>>) -> usize {
    groups.capacity() * size_of::<Vec<u32>>()
        + groups
            .iter()
            .map(|group| group.capacity() * size_of::<u32>())
            .sum::<usize>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_pages_and_covers_its_target() {
        let target = 2 * 1024 * 1024;
        let catalog = Catalog::with_target_bytes(target);
        assert!(catalog.resident_bytes >= target);
        assert!(catalog.resident_bytes - target < target / 5 + 4096);

        let page = catalog.query_products(&ProductLookup::base(1, 5));
        assert_eq!(page.total, catalog.meta().products);
        assert_eq!(page.items.len(), 5);
        assert_eq!(page.items[0].id, 0);
        assert_eq!(page.items[4].id, 4);

        let second = catalog.query_products(&ProductLookup::base(2, 5));
        assert_eq!(second.items[0].id, 5);

        let mut by_category = ProductLookup::base(1, 10);
        by_category.category = Some(0);
        let category_page = catalog.query_products(&by_category);
        assert!(category_page.total > 0);
        assert!(category_page.items.iter().all(|row| row.category == CATEGORIES[0]));

        let summed: u64 = catalog.categories().iter().map(|row| row.products).sum();
        assert_eq!(summed, catalog.meta().products);

        let mut priced = ProductLookup::base(1, 10);
        let price = page.items[0].price_cents;
        priced.min_price = Some(price);
        priced.max_price = Some(price);
        let priced_page = catalog.query_products(&priced);
        assert!(priced_page.total > 0);
        assert!(priced_page.items.iter().all(|row| row.price_cents == price));

        let mut named = ProductLookup::base(1, 10);
        named.words = vec![0];
        let named_page = catalog.query_products(&named);
        assert!(named_page.total > 0);
        assert!(named_page.items.iter().all(|row| {
            row.name.split_whitespace().any(|part| part == WORDS[0])
        }));

        let mut missing = ProductLookup::base(1, 10);
        missing.unknown_word = true;
        assert_eq!(catalog.query_products(&missing).total, 0);

        assert!(catalog.product(0).is_some());
        assert!(catalog.product(u32::MAX).is_none());
        assert!(catalog.customer_orders(0, 1, 10).is_some());
        assert!(catalog.customer_orders(u32::MAX, 1, 10).is_none());
        assert!(catalog.region_report("us-east").is_some());
        assert!(catalog.region_report("nowhere").is_none());

        let orders = catalog.query_orders(&OrderLookup {
            region: Some(0),
            status: Some(1),
            page: 1,
            limit: 10,
        });
        assert!(orders.items.iter().all(|row| row.region == REGIONS[0] && row.status == STATUSES[1]));
    }
}
