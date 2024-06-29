
use lazy_async_promise::ImmediateValuePromise;
use tokio::sync::mpsc;
use uuid::Uuid;

use super::{
    actions::{Action, ActionType}, data::{Data, DataUpdate}, responses::{ActionError, ActionResult}, KeyBounds, ValueBounds
};

pub struct Communicator<Key: KeyBounds, Value: ValueBounds<Key>> {
    pub uuid: Uuid,
    pub sender: Sender<Key, Value>,
    pub reciver: Reciver<Key, Value>,
    pub data: Data<Key, Value>,
}

impl<Key: KeyBounds, Value: ValueBounds<Key>> Communicator<Key, Value> {
    /// Recives any new updates and then updates the internal data accordingly
    pub fn state_update(&mut self) {
        self.reciver.recive_new().into_iter()
            .for_each(|update| update.update_data(&mut self.data));
    }
    /// Sends out an action to update a single element
    pub fn update(&self, val: Value) -> ImmediateValuePromise<ActionResult> {
        self.sender.send(ActionType::Update(val))
    }
    /// Sends out an action to delete a single element
    pub fn delete(&self, key: Key) -> ImmediateValuePromise<ActionResult> {
        self.sender.send(ActionType::Delete(key))
    }
}

pub struct Sender<Key: KeyBounds, Value: ValueBounds<Key>> {
    pub sender: mpsc::Sender<Action<Key, Value>>,
}

impl<Key: KeyBounds, Value: ValueBounds<Key>> Sender<Key, Value> {
    /// Returns a ImmediateValuePromise that will resolve to the result of the
    /// action but not to the actual data. The Data will be automatically updated
    /// if the result is a success
    pub fn send(&self, action_type: ActionType<Key, Value>) -> ImmediateValuePromise<ActionResult> {
        let new_sender = self.sender.clone();
        ImmediateValuePromise::new(async move {
            let (action, reciver) = Action::from_type(action_type);
            let response = match new_sender.send(action).await {
                Ok(_) => ActionResult::from(reciver.await),
                Err(err) => ActionResult::Error(ActionError::send_err(err)),
            };
            Ok(response)
        })
    }
}

pub struct Reciver<Key: KeyBounds, Value: ValueBounds<Key>> {
    pub reciver: mpsc::Receiver<DataUpdate<Key, Value>>,
}

impl<Key: KeyBounds, Value: ValueBounds<Key>> Reciver<Key, Value> {
    /// Tries to recive all new Updates
    fn recive_new(&mut self) -> Vec<DataUpdate<Key, Value>> {
        let mut new_updates = vec![];
        while let Ok(val) = self.reciver.try_recv() {
            new_updates.push(val);
        }
        new_updates
    }
}


