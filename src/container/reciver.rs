
use tokio::sync::mpsc::{self, error::TryRecvError, Receiver};
use tracing::trace;
use uuid::Uuid;

use crate::{change::Change, query::DataQuery, KeyBounds, ValueBounds};

use super::resolving_actions::Action;

pub struct Reciver<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    change_reciver: mpsc::Receiver<Change<Key, Value>>,
    query_reciver: mpsc::Receiver<DataQuery<Key, Value>>,
    bk_change_sender: mpsc::Sender<Change<Key, Value>>,
    bk_query_sender: mpsc::Sender<DataQuery<Key, Value>>,
}

impl<Key, Value> Reciver<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    pub fn senders(
        &self,
    ) -> (
        mpsc::Sender<Change<Key, Value>>,
        mpsc::Sender<DataQuery<Key, Value>>,
    ) {
        (self.bk_change_sender.clone(), self.bk_query_sender.clone())
    }

    pub fn recive_new(&mut self, cont_uuid: &Uuid) -> Vec<Action<Key, Value>> {
        let mut new_actions: Vec<Action<Key, Value>> = vec![];
        new_actions.extend(Self::loop_recive_all(cont_uuid, &mut self.change_reciver));
        new_actions.extend(Self::loop_recive_all(cont_uuid, &mut self.query_reciver));
        new_actions
    }

    fn loop_recive_all<T: Into<Action<Key, Value>>>(
        cont_uuid: &Uuid,
        reciver: &mut Receiver<T>,
    ) -> Vec<Action<Key, Value>> {
        let mut actions = vec![];
        loop {
            match reciver.try_recv() {
                Ok(val) => {
                    let action = val.into();
                    trace!(
                        msg = format!("Recived new [{action}] from Reciver."),
                        cont = cont_uuid.to_string()
                    );
                    actions.push(action);
                }
                Err(err) => match err {
                    TryRecvError::Empty => break,
                    TryRecvError::Disconnected => {
                        unreachable!("One of the recivers of a container has been disconnected, should not be possible.");
                    }
                },
            }
        }
        actions
    }
}

impl<Key, Value> Default for Reciver<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    fn default() -> Self {
        let (action_sender, action_reciver) = mpsc::channel(10);
        let (query_sender, query_reciver) = mpsc::channel(10);

        Self {
            bk_change_sender: action_sender,
            change_reciver: action_reciver,
            bk_query_sender: query_sender,
            query_reciver,
        }
    }
}
