use std::{
    cell::RefCell,
    error::Error,
    io,
    rc::{Rc, Weak},
    task::Poll,
};

use crate::{driver::Driver, memory::DmaSlice, queues::NvmeCompletion, NvmeQueuePair};

pub struct Handle {
    pub inner: Rc<RefCell<Driver>>,
}

pub struct WeakHandle {
    inner: Weak<RefCell<Driver>>,
}

impl Handle {
    pub fn new(driver: Driver) -> io::Result<Self> {
        Ok(Self {
            inner: Rc::new(RefCell::new(driver)),
        })
    }

    pub fn create_io_queue_pair(&mut self, len: usize) -> Result<(), Box<dyn Error>> {
        self.inner.borrow_mut().create_io_queue_pair(len)
    }

    pub fn delete_io_queue_pair(&mut self, qpair: NvmeQueuePair) -> Result<(), Box<dyn Error>> {
        self.inner.borrow_mut().delete_io_queue_pair(qpair)
    }

    pub fn poll_op(&self, c_id: usize) -> Poll<NvmeCompletion> {
        self.inner.borrow_mut().poll_op(c_id)
    }

    pub fn submit_io(
        &mut self,
        qpair: &mut NvmeQueuePair,
        data: &impl DmaSlice,
        lba: u64,
        write: bool,
    ) -> io::Result<()> {
        self.inner.borrow_mut().submit_io(qpair, data, lba, write)
    }
}

impl WeakHandle {
    pub fn upgrade(&self) -> Option<Handle> {
        Some(Handle {
            inner: self.inner.upgrade()?,
        })
    }
}

impl From<Driver> for Handle {
    fn from(driver: Driver) -> Self {
        Self {
            inner: Rc::new(RefCell::new(driver)),
        }
    }
}

impl From<Handle> for WeakHandle {
    fn from(handle: Handle) -> Self {
        Self {
            inner: Rc::downgrade(&handle.inner),
        }
    }
}
