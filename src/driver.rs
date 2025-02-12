use std::{cell::RefCell, error::Error, rc::Rc};

use crate::{
    nvme_future::{Request, State},
    pci::*,
    NvmeDevice,
};

pub struct Driver {
    pub nvme: Rc<RefCell<NvmeDevice>>,
    pub requests: Vec<Request>,
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
            nvme: Rc::new(RefCell::new(nvme)),
            requests: Vec::new(),
        })
    }

    pub async fn listen(&mut self) {
        loop {
            self.poll_queue(1);
        }
    }

    pub fn write_copied(&mut self, data: &[u8], lba: u64) {
        let req = self.nvme.borrow_mut().write_copied_test(data, lba);
        self.requests.push(req);
    }

    pub fn read_copied(&mut self, dest: &mut [u8], lba: u64) {
        let req = self.nvme.borrow_mut().read_copied_test(dest, lba);
        self.requests.push(req);
    }

    fn poll_queue(&mut self, q_id: usize) {
        if q_id == 1 {
            while let Some((tail, c_entry, _)) = self.nvme.borrow_mut().complete() {
                unsafe {
                    std::ptr::write_volatile(
                        self.nvme.borrow().get_c_doorbell() as *mut u32,
                        tail as u32,
                    );
                }
                self.nvme.borrow_mut().set_sq_head(c_entry.sq_head as usize);
                let status = c_entry.status >> 1;
                if status != 0 {
                    eprintln!(
                        "Status: 0x{:x}, Status Code 0x{:x}, Status Code Type: 0x{:x}",
                        status,
                        status & 0xFF,
                        (status >> 8) & 0x7
                    );
                    eprintln!("{:?}", c_entry);
                }
                let mut req = self.requests.remove(c_entry.c_id as usize);
                match req.state {
                    State::Submitted => req.state = State::Completed(c_entry),
                    State::Waiting(_) => req.state = State::Completed(c_entry),
                    State::Completed(_) => println!("Request allready completed."),
                }
            }
        } else {
            // handle multiple queue pairs
            todo!()
        }
    }
}
