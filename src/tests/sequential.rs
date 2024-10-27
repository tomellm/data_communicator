use itertools::Itertools;
use std::{collections::HashMap, time::Duration};

use tokio::{sync::mpsc, time::sleep};

use super::{action::ReadyAction, communicators::Communicators, lib_impls::TestStruct, Comm, Cont};

#[derive(Default)]
pub(super) struct SequentialBuilder {
    num_communicators: usize,
    actions: Vec<ReadyAction>,
}

impl SequentialBuilder {
    pub(super) fn new(num_communicators: usize) -> Self {
        Self {
            num_communicators,
            ..Default::default()
        }
    }
    pub(super) fn actions(mut self, actions: Vec<ReadyAction>) -> Self {
        self.actions = actions;
        self
    }
    pub(super) async fn run(mut self) -> CompletedSequential {
        let mut all = Communicators::init(self.num_communicators).await;

        let (sender, mut reciver) = mpsc::channel(5);
        let mut waiting = false;

        loop {
            all.state_update();
            if let Ok((num, comm)) = reciver.try_recv() {
                all.reinsert(num, comm);
                waiting = false;
            } else if !self.actions.is_empty() && !waiting {
                let next_action = self.actions.remove(0);
                let action_future = all.perform_action(next_action);
                let sender = sender.clone();
                tokio::task::spawn(async move {
                    let output = action_future.await;
                    let _ = sender.send(output).await;
                });
                waiting = true;
            } else if self.actions.is_empty() && !waiting {
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }

        all.into()
    }
}
pub(super) struct CompletedSequential {
    pub(super) container: Cont,
    pub(super) communicators: HashMap<usize, Comm>,
}

impl CompletedSequential {
    pub(super) fn all_equal_in<T: Eq>(&self, func: impl Fn(&Comm) -> T) -> bool {
        self.communicators.values().map(func).all_equal()
    }
    pub(super) fn all_contain(&self, values: Vec<&TestStruct>) -> bool {
        self.all_equal_in(|comm| {
            values
                .iter()
                .map(|val| comm.data().contains(val))
                .all(|cont| cont)
        })
    }
    pub(super) fn comm_contains(&self, num: usize, val: &TestStruct) -> bool {
        self.communicators.get(&num).unwrap().data().contains(&val)
    }
}

impl From<Communicators> for CompletedSequential {
    fn from(Communicators { container, map }: Communicators) -> Self {
        Self {
            container,
            communicators: map,
        }
    }
}
