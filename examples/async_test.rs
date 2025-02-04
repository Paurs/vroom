use rand::{thread_rng, Rng};
use std::{
    env,
    error::Error,
    process,
    time::{Duration, Instant},
};
use vroom::{
    memory::{Dma, DmaSlice},
    NvmeDevice, HUGE_PAGE_SIZE, QUEUE_LENGTH,
};

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args();
    args.next();

    let pci_addr = match args.next() {
        Some(arg) => arg,
        None => {
            eprintln!("Usage: cargo run --example init <pci bus id>");
            process::exit(1);
        }
    };

    let duration = match args.next() {
        Some(secs) => Some(Duration::from_secs(secs.parse().expect(
            "Usage: cargo run --example init <pci bus id> <duration in seconds>",
        ))),
        None => None,
    };

    let driver = vroom::init(&pci_addr);

    let _ = submit_loop(driver.nvme, false, true, duration).await;

    Ok(())
}

async fn submit_loop(
    mut nvme: NvmeDevice,
    write: bool,
    random: bool,
    time: Option<Duration>,
) -> Result<NvmeDevice, Box<dyn Error>> {
    let mut qpair = nvme.create_io_queue_pair(QUEUE_LENGTH)?;

    let mut buffer: Dma<u8> = Dma::allocate(HUGE_PAGE_SIZE)?;

    let blocks = 8;
    let bytes = 512 * blocks as usize;
    let ns_blocks = nvme.namespaces.get(&1).unwrap().blocks / blocks - 1; // - blocks - 1;

    let mut rng = thread_rng();

    let rand_block = &(0..bytes).map(|_| rand::random::<u8>()).collect::<Vec<_>>()[..];
    buffer[..rand_block.len()].copy_from_slice(rand_block);

    let mut total = Duration::ZERO;

    let mut ctr = 0;
    if let Some(time) = time {
        let mut ios = 0;
        let lba = 0;
        while total < time {
            let lba = if random {
                rng.gen_range(0..ns_blocks)
            } else {
                (lba + 1) % ns_blocks
            };

            let before = Instant::now();

            //println!("{ctr}");

            let data = &buffer.slice((ctr * bytes)..(ctr + 1) * bytes);
            let _ = qpair.submit_async(data, lba * blocks, write);

            total += before.elapsed();
            ctr += 1;
            let elapsed = before.elapsed();
            total += elapsed;
            ios += 1;
        }
        println!(
            "IOP: {ios}, total {} iops: {:?}",
            if write { "write" } else { "read" },
            ios as f64 / total.as_secs_f64()
        );
    }

    nvme.delete_io_queue_pair(qpair)?;

    Ok(nvme)
}
