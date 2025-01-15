use crate::queues::{NvmeCompQueue, NvmeCompletion};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

pub struct NvmeFuture<'a> {
    io_cq: &'a mut NvmeCompQueue,
    reqs: i32,
}

impl<'a> NvmeFuture<'a> {
    pub fn new(io_cq: &'a mut NvmeCompQueue, reqs: i32) -> Self {
        Self { io_cq, reqs }
    }
}

impl Future for NvmeFuture<'_> {
    type Output = (usize, NvmeCompletion, usize);

    fn poll(self: Pin<&mut Self>, _context: &mut Context) -> Poll<(usize, NvmeCompletion, usize)> {
        let mut this = self;
        match this.io_cq.complete() {
            None => Poll::Pending,
            Some(val) => {
                unsafe {
                    std::ptr::write_volatile(this.io_cq.doorbell as *mut u32, val.0 as u32);
                }
                Poll::Ready(val)
            }
        }
    }
}
