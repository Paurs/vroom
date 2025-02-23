use crate::queues::NvmeCompletion;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

pub enum State {
    Submitted,
    Waiting(Waker),
    Completed(NvmeCompletion),
}

pub struct Request {
    pub state: State,
}

impl Future for Request {
    type Output = NvmeCompletion;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        println!("polling... ");

        match self.state {
            State::Submitted => {
                self.state = State::Waiting(cx.waker().clone());
                Poll::Pending
            }
            State::Waiting(_) => {
                self.state = State::Waiting(cx.waker().clone());
                Poll::Pending
            }
            State::Completed(completion) => Poll::Ready(completion.clone()),
        }
    }
}

impl Drop for Request {
    fn drop(&mut self) {
        match self.state {
            State::Completed(_) => {}
            _ => panic!("Request dropped before completion."),
        };
    }
}
