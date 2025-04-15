use crate::queues::NvmeCompletion;
use core::fmt;

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
    pub c_id: u16,
    pub r_id: usize,
}

impl Request {
    pub fn new(c_id: u16, r_id: usize) -> Self {
        Request {
            state: State::Submitted,
            c_id,
            r_id,
        }
    }

    pub fn complete(&mut self, completion: NvmeCompletion) {
        self.state = State::Completed(completion);
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
