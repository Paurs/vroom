use crate::queues::NvmeCompletion;
use core::fmt;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

pub enum State {
    Submitted,
    Waiting,
    Completed(NvmeCompletion),
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            State::Submitted => write!(f, "Submitted"),
            State::Waiting => write!(f, "Waiting"),
            State::Completed(_) => write!(f, "Completed"),
        }
    }
}

pub struct Request {
    pub state: State,
    waker: Option<Waker>,
    pub c_id: u16,
    pub r_id: usize,
}

impl Request {
    pub fn new(c_id: u16, r_id: usize) -> Self {
        Request {
            state: State::Submitted,
            waker: None,
            c_id,
            r_id,
        }
    }

    pub fn complete(&mut self, completion: NvmeCompletion) {
        self.state = State::Completed(completion);
        if let Some(waker) = self.waker.take() {
            waker.wake();
        }
    }
}

impl Future for Request {
    type Output = NvmeCompletion;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.state {
            State::Submitted => {
                self.state = State::Waiting;
                self.waker = Some(cx.waker().clone());
                Poll::Pending
            }
            State::Waiting => Poll::Pending,
            State::Completed(completion) => {
                println!("completed {}", self.c_id);
                Poll::Ready(completion.clone())
            }
        }
    }
}

impl Drop for Request {
    fn drop(&mut self) {
        match self.state {
            State::Completed(_) => {}
            _ => panic!(
                "Request dropped before completion with state {}",
                self.state
            ),
        };
    }
}
