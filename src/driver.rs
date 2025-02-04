use std::{error::Error, io, task::Poll};

use crate::{memory::DmaSlice, nvme_future::Request, pci::*, queues::NvmeCompletion, NvmeDevice};

pub struct Driver {
    pub nvme: NvmeDevice,
    requests: Vec<Request>,
}

impl Driver {
    pub fn new(pci_addr: &str) -> Result<Self, Box<dyn Error>> {
        let mut vendor_file = pci_open_resource_ro(pci_addr, "vendor").expect("wrong pci address");
        let mut device_file = pci_open_resource_ro(pci_addr, "device").expect("wrong pci address");
        let mut config_file = pci_open_resource_ro(pci_addr, "config").expect("wrong pci address");

        let _vendor_id = read_hex(&mut vendor_file)?;
        let _device_id = read_hex(&mut device_file)?;
        let class_id = read_io32(&mut config_file, 8)? >> 16;

        // 0x01 -> mass storage device class id
        // 0x08 -> nvme subclass
        if class_id != 0x0108 {
            return Err(format!("device {} is not a block device", pci_addr).into());
        }

        let mut nvme = NvmeDevice::init(pci_addr)?;
        nvme.identify_controller()?;
        let ns = nvme.identify_namespace_list(0);
        for n in ns {
            println!("ns_id: {n}");
            nvme.identify_namespace(n);
        }

        Ok(Driver {
            nvme: nvme,
            requests: Vec::new(),
        })
    }

    pub fn poll_op(&self, c_id: usize) -> Poll<NvmeCompletion> {
        let request = self.requests.get(c_id);

        todo!()
    }

    pub fn submit_io(&mut self, data: &impl DmaSlice, mut lba: u64, write: bool) -> io::Result<()> {
        todo!()
    }
}
