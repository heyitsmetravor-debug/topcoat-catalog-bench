use std::env;
use std::process::ExitCode;
use std::sync::{Arc, OnceLock};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use catalog_bench::catalog::{CATEGORIES, REGIONS, WORDS};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Barrier;

struct Args {
    concurrency: usize,
    duration: Duration,
    host: String,
}

struct Stats {
    ok: AtomicU64,
    err: AtomicU64,
    buckets: [AtomicU64; 64],
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(args) => args,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(2);
        }
    };

    let products = match fetch_products(&args.host).await {
        Ok(count) if count > 0 => count,
        Ok(_) => {
            eprintln!("catalog has no products");
            return ExitCode::from(1);
        }
        Err(message) => {
            eprintln!("failed to read /api/meta: {message}");
            return ExitCode::from(1);
        }
    };

    let stats = Arc::new(Stats::new());
    let started = Arc::new(OnceLock::new());
    let barrier = Arc::new(Barrier::new(args.concurrency));
    let mut workers = Vec::with_capacity(args.concurrency);
    for worker in 0..args.concurrency {
        let host = args.host.clone();
        let stats = Arc::clone(&stats);
        let started = Arc::clone(&started);
        let barrier = Arc::clone(&barrier);
        let duration = args.duration;
        workers.push(tokio::spawn(async move {
            run_worker(worker, host, duration, products, stats, barrier, started).await;
        }));
    }
    for worker in workers {
        let _ = worker.await;
    }

    let elapsed = started
        .get()
        .map(|instant| instant.elapsed())
        .unwrap_or(args.duration);
    let ok = stats.ok.load(Ordering::Relaxed);
    let err = stats.err.load(Ordering::Relaxed);
    let elapsed_ms = elapsed.as_millis().max(1);
    let rps = ok as f64 / (elapsed.as_secs_f64().max(0.001));
    let counts: Vec<u64> = stats
        .buckets
        .iter()
        .map(|bucket| bucket.load(Ordering::Relaxed))
        .collect();
    let p50 = percentile(&counts, ok, 0.50);
    let p95 = percentile(&counts, ok, 0.95);
    let p99 = percentile(&counts, ok, 0.99);
    println!("requests: {ok}");
    println!("errors: {err}");
    println!("elapsed_ms: {elapsed_ms}");
    println!("rps: {rps:.1}");
    println!("p50_us: {p50}");
    println!("p95_us: {p95}");
    println!("p99_us: {p99}");
    println!(
        "result requests={ok} errors={err} elapsed_ms={elapsed_ms} rps={rps:.1} p50_us={p50} p95_us={p95} p99_us={p99}"
    );
    if ok == 0 { ExitCode::from(1) } else { ExitCode::SUCCESS }
}

async fn run_worker(
    seed: usize,
    host: String,
    duration: Duration,
    products: u32,
    stats: Arc<Stats>,
    barrier: Arc<Barrier>,
    started: Arc<OnceLock<Instant>>,
) {
    let mut rng = (seed as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15).max(1);
    let mut conn: Option<Conn> = None;
    if let Some(mut warmup) = connect(&host).await {
        let path = next_path(&mut rng, products);
        if warmup.roundtrip(&host, &path).await.is_err() {
            conn = None;
        } else {
            conn = Some(warmup);
        }
    }
    barrier.wait().await;
    let start = *started.get_or_init(Instant::now);
    let deadline = start + duration;

    while Instant::now() < deadline {
        if conn.is_none() {
            conn = connect(&host).await;
        }
        let Some(socket) = conn.as_mut() else {
            stats.err.fetch_add(1, Ordering::Relaxed);
            tokio::time::sleep(Duration::from_millis(5)).await;
            continue;
        };
        let path = next_path(&mut rng, products);
        let started = Instant::now();
        let result = tokio::time::timeout(
            Duration::from_secs(5),
            socket.roundtrip(&host, &path),
        )
        .await;
        match result {
            Ok(Ok(status)) if (200..300).contains(&status) => {
                let micros = u128::try_from(started.elapsed().as_micros()).unwrap_or(u128::MAX);
                stats.buckets[bucket_index(micros)].fetch_add(1, Ordering::Relaxed);
                stats.ok.fetch_add(1, Ordering::Relaxed);
            }
            _ => {
                stats.err.fetch_add(1, Ordering::Relaxed);
                conn = None;
            }
        }
    }
}

fn next_path(rng: &mut u64, products: u32) -> String {
    let roll = xorshift(rng) % 100;
    if roll < 70 {
        let id = xorshift(rng) % u64::from(products);
        format!("/api/products/{id}")
    } else if roll < 90 {
        let category = CATEGORIES[(xorshift(rng) as usize) % CATEGORIES.len()];
        let region = REGIONS[(xorshift(rng) as usize) % REGIONS.len()];
        let page = 1 + (xorshift(rng) % 5);
        format!("/api/products?category={category}&region={region}&page={page}&limit=20")
    } else {
        let word = WORDS[(xorshift(rng) as usize) % WORDS.len()];
        format!("/api/search?q={word}&limit=20")
    }
}

fn xorshift(state: &mut u64) -> u64 {
    let mut next = *state;
    next ^= next << 13;
    next ^= next >> 7;
    next ^= next << 17;
    *state = next;
    next
}

async fn fetch_products(host: &str) -> Result<u32, String> {
    let mut conn = connect(host).await.ok_or_else(|| "connection failed".to_string())?;
    let (status, body) = conn
        .exchange(host, "/api/meta")
        .await
        .map_err(|error| error.to_string())?;
    if !(200..300).contains(&status) {
        return Err(format!("status {status}"));
    }
    let value: serde_json::Value =
        serde_json::from_slice(&body).map_err(|error| error.to_string())?;
    let products = value
        .get("products")
        .and_then(|value| value.as_u64())
        .ok_or_else(|| "meta response has no products count".to_string())?;
    u32::try_from(products).map_err(|_| "product count does not fit u32".to_string())
}

async fn connect(host: &str) -> Option<Conn> {
    let stream = TcpStream::connect(host).await.ok()?;
    Some(Conn {
        stream,
        pending: Vec::new(),
    })
}

struct Conn {
    stream: TcpStream,
    pending: Vec<u8>,
}

impl Conn {
    async fn roundtrip(&mut self, host: &str, path: &str) -> Result<u16, &'static str> {
        let (status, _) = self.exchange(host, path).await?;
        Ok(status)
    }

    async fn exchange(&mut self, host: &str, path: &str) -> Result<(u16, Vec<u8>), &'static str> {
        let request = format!(
            "GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: keep-alive\r\n\r\n"
        );
        self.stream
            .write_all(request.as_bytes())
            .await
            .map_err(|_| "write failed")?;
        let header = self.read_headers().await?;
        let status = parse_status(&header)?;
        let length = content_length(&header)?;
        let body = self.read_exact(length).await?;
        Ok((status, body))
    }

    async fn read_headers(&mut self) -> Result<String, &'static str> {
        loop {
            if let Some(end) = find_header_end(&self.pending) {
                let header = self.pending.drain(..end).collect::<Vec<u8>>();
                return String::from_utf8(header).map_err(|_| "headers were not utf-8");
            }
            if self.pending.len() > 64 * 1024 {
                return Err("headers were too large");
            }
            self.read_more().await?;
        }
    }

    async fn read_exact(&mut self, length: usize) -> Result<Vec<u8>, &'static str> {
        while self.pending.len() < length {
            self.read_more().await?;
        }
        Ok(self.pending.drain(..length).collect())
    }

    async fn read_more(&mut self) -> Result<(), &'static str> {
        let mut buf = [0u8; 8192];
        let read = self.stream.read(&mut buf).await.map_err(|_| "read failed")?;
        if read == 0 {
            return Err("connection closed");
        }
        self.pending.extend_from_slice(&buf[..read]);
        Ok(())
    }
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes.windows(4).position(|window| window == b"\r\n\r\n").map(|index| index + 4)
}

fn parse_status(header: &str) -> Result<u16, &'static str> {
    let line = header.lines().next().ok_or("empty status line")?;
    let mut parts = line.split_whitespace();
    let _version = parts.next().ok_or("missing http version")?;
    parts
        .next()
        .ok_or("missing status code")?
        .parse()
        .map_err(|_| "status code was not a number")
}

fn content_length(header: &str) -> Result<usize, &'static str> {
    for line in header.split("\r\n").skip(1) {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.eq_ignore_ascii_case("content-length") {
            return value
                .trim()
                .parse()
                .map_err(|_| "content-length was not a number");
        }
    }
    Err("response had no content-length")
}

fn bucket_index(micros: u128) -> usize {
    if micros <= 1 {
        return 0;
    }
    let log = 63 - (micros as u64).leading_zeros();
    (log as usize).min(63)
}

fn percentile(buckets: &[u64], total: u64, quantile: f64) -> u128 {
    if total == 0 {
        return 0;
    }
    let target = ((total as f64) * quantile).ceil().max(1.0) as u64;
    let mut seen = 0u64;
    for (index, count) in buckets.iter().enumerate() {
        seen += count;
        if seen >= target {
            return 1u128 << (index + 1);
        }
    }
    1u128 << 64
}

fn parse_args() -> Result<Args, String> {
    let mut concurrency = 256usize;
    let mut duration = 10u64;
    let mut url = "http://127.0.0.1:3000".to_string();
    let mut args = env::args().skip(1);
    while let Some(flag) = args.next() {
        let value = args
            .next()
            .ok_or_else(|| format!("missing value for {flag}"))?;
        match flag.as_str() {
            "--concurrency" => {
                concurrency = value
                    .parse()
                    .map_err(|_| format!("concurrency must be a positive integer, got {value}"))?;
            }
            "--duration" => {
                duration = value
                    .parse()
                    .map_err(|_| format!("duration must be a positive integer, got {value}"))?;
            }
            "--url" => url = value,
            other => return Err(format!("unknown argument {other}")),
        }
    }
    if concurrency == 0 {
        return Err("concurrency must be at least 1".to_string());
    }
    if duration == 0 {
        return Err("duration must be at least 1".to_string());
    }
    let host = parse_origin(&url)?;
    Ok(Args {
        concurrency,
        duration: Duration::from_secs(duration),
        host,
    })
}

fn parse_origin(url: &str) -> Result<String, String> {
    let Some(rest) = url.strip_prefix("http://") else {
        return Err("url must start with http://".to_string());
    };
    let host = rest.trim_end_matches('/');
    if host.is_empty() || host.contains('/') {
        return Err("url must be an origin like http://127.0.0.1:3000".to_string());
    }
    Ok(host.to_string())
}

impl Stats {
    fn new() -> Self {
        Self {
            ok: AtomicU64::new(0),
            err: AtomicU64::new(0),
            buckets: std::array::from_fn(|_| AtomicU64::new(0)),
        }
    }
}
