use std::collections::HashMap;

use itertools::Itertools;
use lazy_async_promise::{DirectCacheAccess, ImmediateValuePromise, ImmediateValueState};
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use super::{
    actions::Action,
    data::DataUpdate,
    responses::{ActionResponse, ActionResult},
    storage::Storage,
    KeyBounds, ValueBounds,
};

pub struct DataContainer<Key, Value, Writer>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
    Writer: Storage<Key, Value>,
{
    pub reciver: Reciver<Key, Value>,
    pub update_sender: UpdateSender<Key, Value>,
    pub index: DataToCommunicatorIndex<Key>,
    pub storage: Writer,
    running_actions: Vec<(
        ImmediateValuePromise<ActionResponse<Key, Value>>,
        oneshot::Sender<ActionResult>,
    )>,
}

impl<Key, Value, Writer> DataContainer<Key, Value, Writer>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
    Writer: Storage<Key, Value>,
{
    pub async fn new<T>(storage_args: T) -> Self {
        let storage = Writer::init(storage_args).await;
        Self { 
            reciver: Reciver::default(),
            update_sender: UpdateSender::default(),
            index: DataToCommunicatorIndex::default(),
            storage,
            running_actions: Vec::default()
        }
    }

    /// For the state a number of things need to be done:
    /// - Will collect any new actions sent by any of the Communicators and pass
    /// them to the Storage to be processed. The ImmediateValuePromises created
    /// by the storage will then
    pub fn state_update(&mut self) {
        let finished_actions = self.resolve_finished_actions();
        self.update_communicators(finished_actions);
        self.recive_new_actions();
    }

    fn update_communicators(&mut self, updates: Vec<DataUpdate<Key, Value>>) {
        updates.into_iter().for_each(|update| {
            let keys = update.value_keys();
            let communicators = self.index.communicators_from_keys(keys);
            self.update_sender.send_update(update, communicators);
        });
    }

    fn resolve_finished_actions(&mut self) -> Vec<DataUpdate<Key, Value>> {
        self.running_actions
            .extract_if(|(promise, _)| {
                matches!(promise.poll_state(), ImmediateValueState::Success(_))
            })
            .filter_map(|(mut promise, sender)| {
                let Some(response) = promise.poll_state_mut().take_value() else {
                    unreachable!();
                };
                let (opt_data, result) = response.into();
                sender.send(result).unwrap();
                opt_data
            })
            .collect::<Vec<_>>()
    }

    fn recive_new_actions(&mut self) {
        self.running_actions.extend(
            self.reciver
                .recive_new()
                .into_iter()
                .map(|action| {
                    (
                        self.storage.handle_action(action.action),
                        action.reponse_sender,
                    )
                })
                .collect::<Vec<_>>(),
        );
    }
}

pub struct Reciver<Key: KeyBounds, Value: ValueBounds<Key>> {
    sender: mpsc::Sender<Action<Key, Value>>,
    pub reciver: mpsc::Receiver<Action<Key, Value>>,
}

impl<Key: KeyBounds, Value: ValueBounds<Key>> Reciver<Key, Value> {
    fn recive_new(&mut self) -> Vec<Action<Key, Value>> {
        let mut new_actions = vec![];
        while let Ok(val) = self.reciver.try_recv() {
            new_actions.push(val);
        }
        new_actions
    }
}

impl<Key: KeyBounds, Value: ValueBounds<Key>> Default for Reciver<Key, Value> {
    fn default() -> Self {
        let (sender, reciver) = mpsc::channel(10);
        Self { sender, reciver }
    }
}

pub struct UpdateSender<Key: KeyBounds, Value: ValueBounds<Key>> {
    pub senders: HashMap<Uuid, mpsc::Sender<DataUpdate<Key, Value>>>,
}
impl<Key: KeyBounds, Value: ValueBounds<Key>> Default for UpdateSender<Key, Value> {
    fn default() -> Self { Self { senders: HashMap::new() } }
}


impl<Key, Value> UpdateSender<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    pub fn send_update(&self, update: DataUpdate<Key, Value>, targets: Vec<&Uuid>) {
        self.senders
            .iter()
            .filter(|(uuid, _)| targets.contains(uuid))
            .for_each(|(_, sender)| {
                let _ = sender.blocking_send(update.clone());
            });
    }
}

pub struct DataToCommunicatorIndex<Key> {
    pub val_to_comm: HashMap<Key, Vec<Uuid>>,
}

impl<Key: KeyBounds> Default for DataToCommunicatorIndex<Key> {
    fn default() -> Self { Self { val_to_comm: HashMap::new() } }
}

impl<Key: KeyBounds> DataToCommunicatorIndex<Key> {
    pub fn communicators_from_keys(&self, keys: Vec<&Key>) -> Vec<&Uuid> {
        let mut vec_of_vecs = vec![];
        for key in keys.iter() {
            if let Some(uuids) = self.val_to_comm.get(key) {
                vec_of_vecs.extend(uuids);
            }
        }
        vec_of_vecs.into_iter().unique().collect()
    }
}
