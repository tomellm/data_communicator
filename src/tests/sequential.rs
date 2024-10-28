use std::time::Duration;

use tokio::{sync::mpsc, time::sleep};

use super::{action::Action, communicators::Communicators};

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
        if let Some(Action::Action(_)) | Some(Action::Assert(_)) = self.actions.last() {
            self.actions.push(Action::End);
        };
        self
    }
    pub(super) async fn run(mut self) -> Communicators {
        let mut all = Communicators::init(self.num_communicators).await;

        let (action_sender, mut action_reciver) = mpsc::channel(5);
        let (result_sender, mut result_reciver) = mpsc::channel(5);

        tokio::task::spawn(async move {
            loop {
                let new_val = action_reciver.try_recv();
                if let Ok(Some(future)) = new_val {
                    let return_val = future.await;
                    let _ = result_sender.send(return_val).await;
                } else if let Ok(None) = new_val {
                    break;
                } else {
                    sleep(Duration::from_millis(10)).await;
                }
            }
        });

        let mut waiting = false;

        loop {
            all.state_update();
            if let Ok((num, comm)) = result_reciver.try_recv() {
                all.reinsert(num, comm);
                waiting = false;
            } else if !self.actions.is_empty() && !waiting {
                let next_action = self.actions.remove(0);

                if let Action::End = next_action {
                    let _ = action_sender.send(None).await;
                } else if let Some(action_future) = all.perform_action(next_action) {
                    let _ = action_sender.send(Some(action_future)).await;
                    waiting = true;
                }
            } else if self.actions.is_empty() && !waiting {
                break;
            }
            sleep(Duration::from_millis(5)).await;
        }

        all
    }
}
