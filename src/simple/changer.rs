use std::fmt::Debug;

use tokio::sync::{mpsc, watch};

use super::storage::GetKey;


pub struct Action<Key, Value>
where
    Key: Clone + Send + Sync,
    Value: GetKey<Key> + Clone + Send + Sync,
{
    action_type: ActionType<Key, Value>,
    responder: watch::Sender<Response<Key, Value>>,
}

impl<Key, Value> Action<Key, Value>
where
    Key: Clone + Send + Sync,
    Value: GetKey<Key> + Clone + Send + Sync,
{
    #[must_use]
    pub fn set(value: Value, responder: watch::Sender<Response<Key, Value>>) -> Self {
        let action_type = ActionType::Set(value);
        Self {
            action_type,
            responder,
        }
    }
    #[must_use]
    pub fn set_many(values: Vec<Value>, responder: watch::Sender<Response<Key, Value>>) -> Self {
        let action_type = ActionType::SetMany(values);
        Self {
            action_type,
            responder,
        }
    }
    #[must_use]
    pub fn update(value: Value, responder: watch::Sender<Response<Key, Value>>) -> Self {
        Self {
            action_type: ActionType::Update(value),
            responder,
        }
    }
    #[must_use]
    pub fn update_many(values: Vec<Value>, responder: watch::Sender<Response<Key, Value>>) -> Self {
        Self {
            action_type: ActionType::UpdateMany(values),
            responder,
        }
    }
    #[must_use]
    pub fn delete(key: Key, responder: watch::Sender<Response<Key, Value>>) -> Self {
        Self {
            action_type: ActionType::Delete(key),
            responder,
        }
    }
    #[must_use]
    pub fn delete_many(keys: Vec<Key>, responder: watch::Sender<Response<Key, Value>>) -> Self {
        Self {
            action_type: ActionType::DeleteMany(keys),
            responder,
        }
    }
    #[must_use]
    pub fn getall(responder: watch::Sender<Response<Key, Value>>) -> Self {
        Self {
            action_type: ActionType::GetAll(vec![]),
            responder,
        }
    }
    pub fn action(&self) -> &ActionType<Key, Value> {
        &self.action_type
    }
    /// # Errors
    ///
    /// Will error if the `tokio::sync::watch::Sender` fails to send the value
    /// to the container.
    pub fn respond(
        &self,
        response: Response<Key, Value>,
    ) -> Result<(), watch::error::SendError<Response<Key, Value>>> {
        //TODO: dont know if this should be async send or blocking send
        self.responder.send(response)
    }
    pub fn get(self) -> (ActionType<Key, Value>, watch::Sender<Response<Key, Value>>) {
        (self.action_type, self.responder)
    }
}

#[derive(Clone)]
pub enum ActionType<Key, Value>
where
    Key: Clone + Send + Sync,
    Value: GetKey<Key> + Clone + Send + Sync,
{
    Set(Value),
    SetMany(Vec<Value>),
    Update(Value),
    UpdateMany(Vec<Value>),
    Delete(Key),
    DeleteMany(Vec<Key>),
    GetAll(Vec<Value>),
}

#[derive(Clone)]
pub enum Response<Key, Value>
where
    Key: Clone + Send + Sync,
    Value: GetKey<Key> + Clone + Send + Sync,
{
    Loading,
    Worked(ActionType<Key, Value>),
    Error(ActionType<Key, Value>, String),
}

impl<Key, Value> Response<Key, Value>
where
    Key: Clone + Send + Sync,
    Value: GetKey<Key> + Clone + Send + Sync,
{
    pub fn ok(action_type: &ActionType<Key, Value>) -> Self {
        Self::Worked(action_type.clone())
    }
    pub fn err(action_type: &ActionType<Key, Value>, err: String) -> Self {
        Self::Error(action_type.clone(), err)
    }
    pub fn from_result<ResultValue, ResultError: Debug>(
        query_result: Result<ResultValue, ResultError>,
        action: &ActionType<Key, Value>,
    ) -> Self {
        match query_result {
            Ok(_) => Response::ok(action),
            Err(err) => Response::err(action, format!("{err:?}")),
        }
    }
}

#[derive(Clone)]
pub struct Sender<Key, Value>
where
    Key: Clone + Send + Sync,
    Value: GetKey<Key> + Clone + Send + Sync,
{
    sender: mpsc::Sender<Action<Key, Value>>,
    response_recivers: Vec<watch::Receiver<Response<Key, Value>>>,
}

type Responder<Key, Value> = (
    watch::Sender<Response<Key, Value>>,
    watch::Receiver<Response<Key, Value>>,
);

impl<Key, Value> Sender<Key, Value>
where
    Key: Clone + Send + Sync,
    Value: GetKey<Key> + Clone + Send + Sync,
{
    pub fn update_sender(&mut self) {
        self.response_recivers
            .retain(|reciver| match reciver.has_changed() {
                Ok(_) => matches!(*reciver.borrow(), Response::Loading),
                Err(_) => false,
            });
    }
    pub fn new(sender: mpsc::Sender<Action<Key, Value>>) -> Self {
        Self {
            sender,
            response_recivers: vec![],
        }
    }

    pub fn set(&mut self, value: Value) -> watch::Receiver<Response<Key, Value>> {
        let (sender, reciver) = self.new_responder();
        tokio::task::block_in_place(|| {
            let _ = self.sender.blocking_send(Action::set(value, sender));
        });
        reciver
    }
    pub fn set_action(&mut self) -> impl FnMut(Value) -> watch::Receiver<Response<Key, Value>> {
        let action_sender = self.sender.clone();
        move |value: Value| {
            let (sender, reciver) = watch::channel(Response::Loading);
            tokio::task::block_in_place(|| {
                let _ = action_sender.blocking_send(Action::set(value, sender));
            });
            reciver
        }
    }
    pub fn set_many(&mut self, values: Vec<Value>) -> watch::Receiver<Response<Key, Value>> {
        let (sender, reciver) = self.new_responder();
        tokio::task::block_in_place(|| {
            let _ = self.sender.blocking_send(Action::set_many(values, sender));
        });
        reciver
    }
    pub fn set_many_action(
        &mut self,
    ) -> impl FnMut(Vec<Value>) -> watch::Receiver<Response<Key, Value>> {
        let action_sender = self.sender.clone();
        move |values: Vec<Value>| {
            let (sender, reciver) = watch::channel(Response::Loading);
            tokio::task::block_in_place(|| {
                let _ = action_sender.blocking_send(Action::set_many(values, sender));
            });
            reciver
        }
    }

    pub fn update(&mut self, value: Value) -> watch::Receiver<Response<Key, Value>> {
        let (sender, reciver) = self.new_responder();
        tokio::task::block_in_place(|| {
            let _ = self.sender.blocking_send(Action::update(value, sender));
        });
        reciver
    }
    pub fn update_action(&mut self) -> impl FnMut(Value) -> watch::Receiver<Response<Key, Value>> {
        let action_sender = self.sender.clone();
        move |value: Value| {
            let (sender, reciver) = watch::channel(Response::Loading);
            tokio::task::block_in_place(|| {
                let _ = action_sender.blocking_send(Action::update(value, sender));
            });
            reciver
        }
    }
    pub fn update_many(&mut self, values: Vec<Value>) -> watch::Receiver<Response<Key, Value>> {
        let (sender, reciver) = self.new_responder();
        tokio::task::block_in_place(|| {
            let _ = self
                .sender
                .blocking_send(Action::update_many(values, sender));
        });
        reciver
    }
    pub fn update_many_action(
        &mut self,
    ) -> impl FnMut(Vec<Value>) -> watch::Receiver<Response<Key, Value>> {
        let action_sender = self.sender.clone();
        move |values: Vec<Value>| {
            let (sender, reciver) = watch::channel(Response::Loading);
            tokio::task::block_in_place(|| {
                let _ = action_sender.blocking_send(Action::update_many(values, sender));
            });
            reciver
        }
    }

    pub fn delete(&mut self, key: Key) -> watch::Receiver<Response<Key, Value>> {
        let (sender, reciver) = self.new_responder();
        tokio::task::block_in_place(|| {
            let _ = self.sender.blocking_send(Action::delete(key, sender));
        });
        reciver
    }
    pub fn delete_action(&mut self) -> impl FnMut(Key) -> watch::Receiver<Response<Key, Value>> {
        let action_sender = self.sender.clone();
        move |key: Key| {
            let (sender, reciver) = watch::channel(Response::Loading);
            tokio::task::block_in_place(|| {
                let _ = action_sender.blocking_send(Action::delete(key, sender));
            });
            reciver
        }
    }
    pub fn delete_many(&mut self, keys: Vec<Key>) -> watch::Receiver<Response<Key, Value>> {
        let (sender, reciver) = self.new_responder();
        tokio::task::block_in_place(|| {
            let _ = self.sender.blocking_send(Action::delete_many(keys, sender));
        });
        reciver
    }
    pub fn delete_many_action(
        &mut self,
    ) -> impl FnMut(Vec<Key>) -> watch::Receiver<Response<Key, Value>> {
        let action_sender = self.sender.clone();
        move |keys: Vec<Key>| {
            let (sender, reciver) = watch::channel(Response::Loading);
            tokio::task::block_in_place(|| {
                let _ = action_sender.blocking_send(Action::delete_many(keys, sender));
            });
            reciver
        }
    }

    pub fn getall(&mut self) -> watch::Receiver<Response<Key, Value>> {
        let (sender, reciver) = self.new_responder();
        self.send(Action::getall(sender));
        reciver
    }

    fn send(&self, action: Action<Key, Value>) {
        let _ = self.sender.blocking_send(action);
    }

    fn new_responder(&mut self) -> Responder<Key, Value> {
        let (sender, reciver) = watch::channel(Response::Loading);
        self.response_recivers.push(reciver.clone());
        (sender, reciver)
    }

    //TODO: currently will never return anything since the `self.reponse_recivers`
    // vector is never pushed to and thus never contains values.
    /*pub fn responses(&self) -> Option<Vec<Response<Key, Value>>> {
        let (loading, done) = self.response_recivers.into_iter().fold(
            (vec![], vec![]),
            |(mut loading, mut done), reciver| {
                let response = reciver.borrow();
                if let Response::Loading = *response {
                    loading.push(reciver);
                } else {
                    done.push((*response).clone());
                }
                (loading, done)
            }
        );
        self.response_recivers = loading;
        if done.is_empty() { None } else { Some(done) }
    }*/
}

pub(super) struct Reciver<Key, Value>
where
    Key: Clone + Send + Sync,
    Value: GetKey<Key> + Clone + Send + Sync,
{
    reciver: mpsc::Receiver<Action<Key, Value>>,
    bk_sender: mpsc::Sender<Action<Key, Value>>,
}

impl<Key, Value> Reciver<Key, Value>
where
    Key: Clone + Send + Sync,
    Value: GetKey<Key> + Clone + Send + Sync,
{
    pub fn new(
        sender: mpsc::Sender<Action<Key, Value>>,
        reciver: mpsc::Receiver<Action<Key, Value>>,
    ) -> Self {
        Self {
            reciver,
            bk_sender: sender,
        }
    }
    pub(super) fn get_sender(&self) -> Sender<Key, Value> {
        Sender::new(self.bk_sender.clone())
    }
    pub(super) fn recive(&mut self) -> Option<Vec<Action<Key, Value>>> {
        Self::recive_vals(&mut self.reciver, None)
    }
    fn recive_vals(
        reciver: &mut mpsc::Receiver<Action<Key, Value>>,
        actions: Option<Vec<Action<Key, Value>>>,
    ) -> Option<Vec<Action<Key, Value>>> {
        match reciver.try_recv() {
            Ok(val) => {
                let mut actions = actions.unwrap_or_default();
                actions.push(val);
                Self::recive_vals(reciver, Some(actions))
            }
            Err(err) if err.eq(&mpsc::error::TryRecvError::Empty) => actions,
            Err(err) => panic!(
                r#"

            Reciving change actions led to an error:

            {err}

            "#
            ),
        }
    }
}
