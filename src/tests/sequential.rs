use itertools::Itertools;
use std::{collections::HashMap, time::Duration};

use tokio::{sync::mpsc, time::sleep};

use super::{action::{Action, ReadyAction}, communicators::Communicators, lib_impls::TestStruct, Comm, Cont};

#[derive(Default)]
pub(super) struct SequentialBuilder {
    num_communicators: usize,
    actions: Vec<Action>,
}

impl SequentialBuilder {
    pub(super) fn new(num_communicators: usize) -> Self {
        Self {
            num_communicators,
            ..Default::default()
        }
    }
    pub(super) fn actions(mut self, actions: Vec<Action>) -> Self {
        self.actions = actions;
        self
    }
    pub(super) async fn run(mut self) -> Communicators {
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
                if let Some(action_future) = all.perform_action(next_action) {
                    let sender = sender.clone();
                    tokio::task::spawn(async move {
                        let output = action_future.await;
                        let _ = sender.send(output).await;
                    });
                    waiting = true;
                }
            } else if self.actions.is_empty() && !waiting {
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }

        all
    }
}
