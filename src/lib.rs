#![cfg_attr(target_arch = "aarch64", feature(stdarch_arm_hints))]
#[allow(unused)]
mod cmd;
#[allow(dead_code)]
pub mod driver;
#[allow(dead_code)]
pub mod memory;
#[allow(dead_code)]
mod nvme;
#[allow(dead_code)]
mod pci;
#[allow(dead_code)]
mod queues;
#[allow(dead_code)]
pub mod request;

pub use memory::HUGE_PAGE_SIZE;
pub use nvme::{NvmeDevice, NvmeQueuePair};
pub use queues::QUEUE_LENGTH;
use std::error::Error;

pub fn init(_pci_addr: &str) -> Result<(), Box<dyn Error>> {
    Ok(())
}

#[derive(Debug, Clone, Copy)]
pub struct NvmeNamespace {
    pub id: u32,
    pub blocks: u64,
    pub block_size: u64,
}

#[derive(Debug, Clone, Default)]
pub struct NvmeStats {
    pub completions: u64,
    pub submissions: u64,
}
