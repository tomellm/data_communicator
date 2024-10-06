use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
};

use itertools::Itertools;
use lazy_async_promise::{
    BoxedSendError, DirectCacheAccess, ImmediateValuePromise, ImmediateValueState,
};
use tokio::sync::{
    mpsc::{self, error::TryRecvError, Receiver},
    oneshot,
};
use tracing::{debug, enabled, info, trace, warn, Level};
use uuid::Uuid;

use super::{
    change::{Change, ChangeResponse, ChangeResult},
    communicator::Communicator,
    data::{DataChange, FreshData},
    query::{DataQuery, QueryResponse, QueryResult},
    storage::Storage,
    utils::{DrainIf, PromiseUtilities},
    KeyBounds, ValueBounds,
};

pub struct DataContainer<Key, Value, Writer>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
    Writer: Storage<Key, Value>,
{
    uuid: Uuid,
    pub reciver: Reciver<Key, Value>,
    pub update_sender: UpdateSender<Key, Value>,
    pub index: DataToCommunicatorIndex<Key>,
    pub storage: Writer,
    running_actions: Vec<ResolvingAction<Key, Value>>,
}

impl<Key, Value, Writer> DataContainer<Key, Value, Writer>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
    Writer: Storage<Key, Value>,
{
    pub async fn new(storage_args: Writer::InitArgs) -> Self {
        let storage = Writer::init(storage_args).await;
        Self {
            uuid: Uuid::new_v4(),
            reciver: Reciver::default(),
            update_sender: UpdateSender::default(),
            index: DataToCommunicatorIndex::default(),
            storage,
            running_actions: Vec::default(),
        }
    }

    /// For the state a number of things need to be done:
    /// - Will collect any new actions sent by any of the Communicators and pass
    ///   them to the Storage to be processed. The `ImmediateValuePromises` created
    ///   by the storage will then
    pub fn state_update(&mut self) {
        self.update_sender.state_update();
        self.resolve_finished_actions()
            .into_iter()
            .for_each(|action| match action {
                ResolvedAction::Change(change) => {
                    trace!(
                        msg = format!("Finished change action, updating communicators."),
                        cont = self.uuid.to_string()
                    );
                    self.update_communicators(&change)
                }
                ResolvedAction::Query(query, uuid) => {
                    trace!(
                        msg = format!("Finished query action, returning result."),
                        cont = self.uuid.to_string()
                    );
                    self.return_query(uuid, query)
                }
            });
        self.recive_new_actions();
    }

    pub fn communicator(&mut self) -> Communicator<Key, Value> {
        let new_uuid = Uuid::new_v4();

        info!(
            msg = format!("Creating new Communicator with uuid: {}.", new_uuid),
            cont = self.uuid.to_string()
        );

        let (change_sender, query_sender) = self.reciver.senders();
        let (change_data_sender, change_data_reciver) = mpsc::channel(10);
        let (fresh_data_sender, fresh_data_reciver) = mpsc::channel(10);

        self.update_sender
            .register_senders(&new_uuid, change_data_sender, fresh_data_sender);

        Communicator::new(
            new_uuid,
            change_sender,
            query_sender,
            change_data_reciver,
            fresh_data_reciver,
        )
    }

    fn update_communicators(&mut self, update: &DataChange<Key, Value>) {
        let keys = update.value_keys();
        let number_of_keys = keys.len();
        let communicators = self.index.communicators_from_keys(&keys);
        debug!(
            msg = format!(
                "Recived data update will modify {} keys and go to {} communicators",
                number_of_keys,
                communicators.len()
            ),
            cont = self.uuid.to_string()
        );
        self.update_sender
            .send_change(&self.uuid, update, &communicators);
    }

    fn return_query(&mut self, communicator: Uuid, values: FreshData<Key, Value>) {
        let keys = values.keys().collect::<Vec<_>>();
        debug!(
            msg = format!(
                "Fresh data will send {} new values to communicator {}",
                keys.len(),
                communicator
            ),
            cont = self.uuid.to_string()
        );
        self.index.extend_index_with_query(communicator, keys);
        self.update_sender
            .send_fresh_data(&self.uuid, values, &communicator);
    }

    fn resolve_finished_actions(&mut self) -> Vec<ResolvedAction<Key, Value>> {
        // NOTE: the `is_done` function here will poll the interal state of the
        // promise. I think this is nessesary since otherwise no work will be
        // done on the function
        self.running_actions
            .drain_if_iter(|e| e.poll_and_finished())
            .filter_map(|resolving_action| {
                trace!(
                    msg = format!(
                        "Resolving action of type {} has finished and will be resolved",
                        resolving_action.action_type()
                    ),
                    cont = self.uuid.to_string()
                );
                resolving_action.resolve(&self.uuid)
            })
            .collect_vec()
    }

    fn recive_new_actions(&mut self) {
        let new_action = self
            .reciver
            .recive_new(&self.uuid)
            .into_iter()
            .map(|action| {
                debug!(
                    msg = format!("Recived new [{action}] action to work on."),
                    cont = self.uuid.to_string()
                );
                action.handle_action(&mut self.storage)
            })
            .collect::<Vec<_>>();

        if !new_action.is_empty() {
            info!(
                msg = format!("There are {} new actions to work on.", new_action.len()),
                cont = self.uuid.to_string()
            );
        }

        self.running_actions.extend(new_action);
    }
}

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
    fn senders(
        &self,
    ) -> (
        mpsc::Sender<Change<Key, Value>>,
        mpsc::Sender<DataQuery<Key, Value>>,
    ) {
        (self.bk_change_sender.clone(), self.bk_query_sender.clone())
    }

    fn recive_new(&mut self, cont_uuid: &Uuid) -> Vec<Action<Key, Value>> {
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

pub struct UpdateSender<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    change_senders: HashMap<Uuid, mpsc::Sender<DataChange<Key, Value>>>,
    query_senders: HashMap<Uuid, mpsc::Sender<FreshData<Key, Value>>>,
    sending_responses: Vec<ImmediateValuePromise<()>>,
}
impl<Key, Value> Default for UpdateSender<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    fn default() -> Self {
        Self {
            change_senders: HashMap::new(),
            query_senders: HashMap::new(),
            sending_responses: vec![],
        }
    }
}

impl<Key, Value> UpdateSender<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    /// Registeres the senders for a new communicator. These will then be used
    /// to send data back to the communicator after a query or change.
    fn register_senders(
        &mut self,
        communicator_uuid: &Uuid,
        change_sender: mpsc::Sender<DataChange<Key, Value>>,
        query_sender: mpsc::Sender<FreshData<Key, Value>>,
    ) {
        let existing_change_sender = self
            .change_senders
            .insert(*communicator_uuid, change_sender);
        assert!(existing_change_sender.is_none());

        let existing_query_sender = self.query_senders.insert(*communicator_uuid, query_sender);
        assert!(existing_query_sender.is_none());
    }

    pub fn state_update(&mut self) {
        let _ = self
            .sending_responses
            .drain_if(|e| !matches!(e.poll_state(), ImmediateValueState::Updating));
    }

    /// Sends the `DataChange` to the correct targets. To know who the targets
    /// are the index needs to be queried first. These targets should be all the
    /// communicators that have any of the values contained in the update.
    fn send_change(
        &mut self,
        cont_uuid: &Uuid,
        update: &DataChange<Key, Value>,
        targets: &[&Uuid],
    ) {
        trace!(
            msg = format!("Sending change data to {} targets", targets.len()),
            cont = cont_uuid.to_string()
        );

        let new_sending_responses = self
            .change_senders
            .iter()
            .filter_map(|(uuid, sender)| {
                if targets.contains(&uuid) {
                    Some((*uuid, sender.clone()))
                } else {
                    None
                }
            })
            .map(|(uuid, sender)| {
                let update = update.clone();
                let string_uuid = cont_uuid.to_string();
                ImmediateValuePromise::new(async move {
                    let send_res = sender.send(update).await.map_err(BoxedSendError::from);
                    debug!(
                        msg = format!("Sent off data change to communicator [{uuid}]."),
                        cont = string_uuid
                    );
                    send_res
                })
            })
            .collect_vec();
        if !new_sending_responses.is_empty() {
            debug!(
                msg = format!(
                    "Added {} new sending responses to the vec.",
                    new_sending_responses.len()
                ),
                cont = cont_uuid.to_string()
            );
            self.sending_responses.extend(new_sending_responses);
        }
    }

    /// Returns fresh data to the communicator that requested the data.
    fn send_fresh_data(
        &mut self,
        cont_uuid: &Uuid,
        fresh_data: FreshData<Key, Value>,
        target: &Uuid,
    ) {
        trace!(
            msg = format!("Sending fresh data to communicator [{}]", target),
            cont = cont_uuid.to_string()
        );

        let prev_len_sending_res = self.sending_responses.len();

        if let Some(sender) = self.query_senders.get(target) {
            let target = *target;
            let new_sender = sender.clone();
            let str_uuid = cont_uuid.to_string();
            let new_sending_response = ImmediateValuePromise::new(async move {
                let send_res = new_sender
                    .send(fresh_data)
                    .await
                    .map_err(BoxedSendError::from);
                debug!(
                    msg = format!("Sent off fresh data to communicator [{target}]."),
                    cont = str_uuid
                );
                send_res
            });
            self.sending_responses.push(new_sending_response);
        };

        if enabled!(Level::DEBUG) {
            let len_diff = self.sending_responses.len() - prev_len_sending_res;
            if len_diff > 0 {
                debug!(
                    msg = format!("Added {} new sending responses to the vec", len_diff),
                    cont = cont_uuid.to_string()
                );
            }
        }
    }
}

pub struct DataToCommunicatorIndex<Key> {
    pub val_to_comm: HashMap<Key, HashSet<Uuid>>,
}

impl<Key> Default for DataToCommunicatorIndex<Key>
where
    Key: KeyBounds,
{
    fn default() -> Self {
        Self {
            val_to_comm: HashMap::new(),
        }
    }
}

impl<Key: KeyBounds> DataToCommunicatorIndex<Key>
where
    Key: KeyBounds,
{
    pub fn communicators_from_keys(&self, keys: &[&Key]) -> Vec<&Uuid> {
        let mut vec_of_vecs = vec![];
        for key in keys {
            if let Some(uuids) = self.val_to_comm.get(key) {
                vec_of_vecs.extend(uuids);
            }
        }
        vec_of_vecs.into_iter().unique().collect()
    }

    pub fn extend_index_with_query(&mut self, communicator: Uuid, keys: Vec<&Key>) {
        for key in keys {
            let set = self.val_to_comm.entry(key.clone()).or_default();
            set.insert(communicator);
        }
    }
}

pub enum ResolvingAction<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    Change(
        ImmediateValuePromise<ChangeResponse<Key, Value>>,
        oneshot::Sender<ChangeResult>,
    ),
    Query(
        ImmediateValuePromise<QueryResponse<Key, Value>>,
        Uuid,
        oneshot::Sender<QueryResult>,
    ),
}

impl<Key, Value> ResolvingAction<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    fn poll_and_finished(&mut self) -> bool {
        match self {
            Self::Change(promise, _) => promise.poll_and_check_finished(),
            Self::Query(promise, _, _) => promise.poll_and_check_finished(),
        }
    }

    fn resolve(self, cont_uuid: &Uuid) -> Option<ResolvedAction<Key, Value>> {
        match self {
            ResolvingAction::Change(mut promise, sender) => {
                promise.take_value().map(|change_response| {
                    let (data_change, change_result) = change_response.into();
                    let _ = sender.send(change_result).map_err(|value| {
                        warn!(msg = format!("Change result could not be sent because reciver was dropped. Result was: [{value:?}]"), cont = cont_uuid.to_string())
                    });
                    debug!(msg = format!("Sent reponse of change result to communicator"), cont = cont_uuid.to_string());
                    data_change.map(|data| ResolvedAction::Change(data))
                })?
            }
            ResolvingAction::Query(mut promise, uuid, sender) => {
                promise.take_value().map(|query_response| {
                    let (fresh_data, result) = query_response.into();
                    let _ = sender.send(result).map_err(|value| {
                        warn!(msg = format!("Qeuery result could not be sent because reciver was dropped. Result was: [{value:?}]"), cont = cont_uuid.to_string())
                    });
                    debug!(msg = format!("Sent response of query result to communicator [{uuid}]"), cont = cont_uuid.to_string());
                    fresh_data.map(|data| ResolvedAction::Query(data, uuid))
                })?
            }
        }
    }

    fn action_type(&self) -> &str {
        match self {
            Self::Change(_, _) => "change",
            Self::Query(_, _, _) => "query",
        }
    }
}

pub enum ResolvedAction<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    Change(DataChange<Key, Value>),
    Query(FreshData<Key, Value>, Uuid),
}

pub enum Action<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    Change(Change<Key, Value>),
    Query(DataQuery<Key, Value>),
}

impl<Key, Value> Action<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    fn handle_action<Writer: Storage<Key, Value>>(
        self,
        writer: &mut Writer,
    ) -> ResolvingAction<Key, Value> {
        match self {
            Self::Change(change) => {
                ResolvingAction::Change(writer.handle_change(change.action), change.reponse_sender)
            }
            Self::Query(query) => ResolvingAction::Query(
                writer.handle_query(query.query_type),
                query.origin_uuid,
                query.response_sender,
            ),
        }
    }
}

impl<Key, Value> Display for Action<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Change(_) => "Change(..)",
                Self::Query(_) => "Query(..)",
            }
        )
    }
}

impl<Key, Value> From<Change<Key, Value>> for Action<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    fn from(value: Change<Key, Value>) -> Self {
        Self::Change(value)
    }
}

impl<Key, Value> From<DataQuery<Key, Value>> for Action<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    fn from(value: DataQuery<Key, Value>) -> Self {
        Self::Query(value)
    }
}
