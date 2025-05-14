use std::error::Error;
use std::time::{Duration, Instant};
use std::{env, process};

use vroom::driver::Driver;
use vroom::memory::{Dma, DmaSlice};
use vroom::HUGE_PAGE_SIZE;

use tracing_perfetto::PerfettoLayer;
use tracing_subscriber::prelude::*;

#[cfg(not(target_env = "msvc"))]
use tikv_jemallocator::Jemalloc;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

#[tokio::main(flavor = "multi_thread")]
pub async fn main() -> Result<(), Box<dyn Error>> {
    //env::set_var("RUST_BACKTRACE", "1");

    let layer = PerfettoLayer::new(std::sync::Mutex::new(
        std::fs::File::create("/tmp/test.pftrace").unwrap(),
    ));
    tracing_subscriber::registry().with(layer).init();

    let mut args = env::args();
    args.next();

    let pci_addr = match args.next() {
        Some(arg) => arg,
        None => {
            eprintln!("Usage: cargo run --example init <pci bus id> <number of queue pairs> <duration in seconds>");
            process::exit(1);
        }
    };

    let queue_num = match args.next() {
        Some(num) => num.parse()?,
        None => {
            eprintln!("Usage: cargo run --example init <pci bus id> <number of queue pairs> <duration in seconds>");
            process::exit(1);
        }
    };

    let batch_size = match args.next() {
        Some(num) => num.parse()?,
        None => {
            eprintln!("Usage: cargo run --example init <pci bus id> <number of queue pairs> <duration in seconds>");
            process::exit(1);
        }
    };

    let duration = args.next().map(|secs| {
        Duration::from_secs(secs.parse().expect(
    "Usage: cargo run --example init <pci bus id> <number of queue pairs> <duration in seconds>",
    ))
    });

    let bytes = 512 * 8;
    if (batch_size * bytes) >= HUGE_PAGE_SIZE {
        return Err(format!(
            "Error: batch_size * bytes ({} * {}) >= HUGE_PAGE_SIZE ({})",
            batch_size, bytes, HUGE_PAGE_SIZE
        )
        .into());
    }

    let driver = Driver::<Dma<u8>>::new(&pci_addr, queue_num)?;

    let time = duration.unwrap();

    //let buffer: Dma<u8> = Dma::allocate(HUGE_PAGE_SIZE).unwrap();

    let buffers: Vec<Dma<u8>> = (0..queue_num)
        .map(|_| Dma::allocate(HUGE_PAGE_SIZE).unwrap())
        .collect();

    let mut lba_vecs: Vec<Vec<u64>> = (0..queue_num)
        .map(|_| Vec::with_capacity(batch_size))
        .collect();

    let mut pending = Vec::with_capacity(queue_num * batch_size);
    let mut op_count = 0;
    let start = Instant::now();

    while start.elapsed() < time {
        for (i, buffer) in buffers.iter().enumerate().take(queue_num) {
            let mut data = Vec::with_capacity(batch_size);
            let lbas = &mut lba_vecs[i];
            lbas.clear();

            for j in 0..batch_size {
                let offset = j * bytes;
                data.push(buffer.slice(offset..(offset + bytes)));
                lbas.push((i * batch_size + j) as u64);
            }

            loop {
                let mut ftrs = driver.read_batch(i, &data, &lbas).await;
                if !ftrs.is_empty() {
                    op_count += ftrs.len();
                    pending.append(&mut ftrs);
                    break;
                } else if let Some(r) = pending.pop() {
                    let _ = r.await;
                } else {
                    tokio::task::yield_now().await;
                }
            }
        }

        if op_count % (queue_num * batch_size) == 0 {
            let drained: Vec<_> = pending.drain(..).collect();
            let _ = futures::future::join_all(drained).await;
        }
    }

    if !pending.is_empty() {
        let drained: Vec<_> = pending.drain(..).collect();
        let _ = futures::future::join_all(drained).await;
    }

    println!("{} iops", (op_count as f64 / time.as_secs_f64()));

    driver.cleanup().await
}
