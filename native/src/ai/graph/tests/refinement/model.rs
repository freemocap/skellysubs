//! Finite specification, independent of native graph types and reducer helpers.
//! See docs/notes/ai-graph-refinement.md for the boundary and exclusions.

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Failure {
    Provider,
    Abandoned,
    Interrupted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Producer {
    Queued,
    Running,
    Success,
    Failed(Failure),
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Consumer {
    Queued,
    Running,
    Available,
    Adopted,
    Failed(Failure),
    Unknown,
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct State {
    pub producer: Producer,
    pub consumers: [Consumer; 2],
    pub paused: [bool; 2],
    pub active: [bool; 2],
    pub capacity: bool,
}

#[derive(Clone, Copy, Debug)]
pub enum Action {
    Pause(usize, bool),
    Cancel(usize),
    Adopt(usize),
    Advance(bool),
    Dispatch,
    Settle(bool),
    Recover,
}

pub const ACTIONS: [Action; 14] = [
    Action::Pause(0, false),
    Action::Pause(0, true),
    Action::Pause(1, false),
    Action::Pause(1, true),
    Action::Cancel(0),
    Action::Cancel(1),
    Action::Adopt(0),
    Action::Adopt(1),
    Action::Advance(false),
    Action::Advance(true),
    Action::Dispatch,
    Action::Settle(false),
    Action::Settle(true),
    Action::Recover,
];

impl State {
    pub const INITIAL: Self = Self {
        producer: Producer::Queued,
        consumers: [Consumer::Queued; 2],
        paused: [false; 2],
        active: [true; 2],
        capacity: true,
    };

    pub fn eligible(&self) -> bool {
        (0..2).any(|i| self.active[i] && !self.paused[i])
    }

    /// None means rejection, hence no state change or work exposure.
    /// The count is prepared work for Advance, dispatched work for Dispatch.
    pub fn step(mut self, action: Action) -> Option<(Self, usize)> {
        let mut work = 0;
        match action {
            Action::Pause(i, paused) => self.paused[i] = paused,
            Action::Cancel(i) => {
                self.active[i] = false;
                if matches!(
                    self.consumers[i],
                    Consumer::Queued | Consumer::Running | Consumer::Available
                ) {
                    self.consumers[i] = Consumer::Cancelled;
                }
                if self.producer == Producer::Queued && self.active == [false; 2] {
                    self.producer = Producer::Failed(Failure::Abandoned);
                }
            }
            Action::Adopt(i) => {
                if !self.active[i] || self.consumers[i] != Consumer::Available {
                    return None;
                }
                self.consumers[i] = Consumer::Adopted;
            }
            Action::Advance(capacity) => {
                self.capacity = capacity;
                work = usize::from(self.producer == Producer::Queued && self.eligible());
            }
            Action::Dispatch => {
                if self.producer != Producer::Queued || !self.eligible() || !self.capacity {
                    return None;
                }
                self.producer = Producer::Running;
                self.replace(Consumer::Queued, Consumer::Running);
                work = 1;
            }
            Action::Settle(success) => {
                if self.producer != Producer::Running {
                    return None;
                }
                let consumer = if success {
                    self.producer = Producer::Success;
                    Consumer::Available
                } else {
                    self.producer = Producer::Failed(Failure::Provider);
                    Consumer::Failed(Failure::Provider)
                };
                self.replace(Consumer::Running, consumer);
            }
            Action::Recover => {
                self.paused = [true; 2];
                match self.producer {
                    Producer::Queued => {
                        self.producer = Producer::Failed(Failure::Interrupted);
                        self.replace(Consumer::Queued, Consumer::Failed(Failure::Interrupted));
                    }
                    Producer::Running => {
                        self.producer = Producer::Unknown;
                        self.replace(Consumer::Running, Consumer::Unknown);
                    }
                    _ => (),
                }
            }
        }
        Some((self, work))
    }

    fn replace(&mut self, from: Consumer, to: Consumer) {
        for state in &mut self.consumers {
            if *state == from {
                *state = to;
            }
        }
    }
}
