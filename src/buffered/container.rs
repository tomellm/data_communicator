use std::collections::{HashMap, HashSet, VecDeque};

use itertools::Itertools;
use lazy_async_promise::{DirectCacheAccess, ImmediateValuePromise, ImmediateValueState};
use tokio::sync::{mpsc, oneshot};
use tracing::{event, Level};
use uuid::Uuid;

use super::{
    change::{Change, ChangeResponse, ChangeResult},
    communicator::Communicator,
    data::{DataChange, FreshData},
    query::{DataQuery, QueryResponse, QueryResult},
    storage::Storage,
    utils::PromiseUtils,
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
            reciver: Reciver::default(),
            update_sender: UpdateSender::default(),
            index: DataToCommunicatorIndex::default(),
            storage,
            running_actions: Vec::default(),
        }
    }

    /// For the state a number of things need to be done:
    /// - Will collect any new actions sent by any of the Communicators and pass
    /// them to the Storage to be processed. The ImmediateValuePromises created
    /// by the storage will then
    pub fn state_update(&mut self) {
        self.resolve_finished_actions()
            .into_iter()
            .for_each(|action| match action {
                ResolvedAction::Change(change) => self.update_communicators(change),
                ResolvedAction::Query(query, uuid) => self.return_query(uuid, query),
            });
        self.recive_new_actions();
    }

    pub fn communicator(&mut self) -> Communicator<Key, Value> {
        let new_uuid = Uuid::new_v4();

        event!(
            Level::INFO,
            "Creating new Communicator with uuid: {}.",
            new_uuid
        );

        let (change_sender, query_sender) = self.reciver.senders();
        let (change_data_sender, change_data_reciver) = mpsc::channel(10);
        let (fresh_data_sender, fresh_data_reciver) = mpsc::channel(10);

        self.update_sender
            .register_senders(&new_uuid, change_data_sender, fresh_data_sender);

        let communicator = Communicator::new(
            new_uuid,
            change_sender,
            query_sender,
            change_data_reciver,
            fresh_data_reciver,
        );

        communicator
    }

    fn update_communicators(&mut self, update: DataChange<Key, Value>) {
        let keys = update.value_keys();
        let number_of_keys = keys.len();
        let communicators = self.index.communicators_from_keys(keys);
        event!(
            Level::DEBUG,
            "Recived data update will modify {} keys and go to {} communicators",
            number_of_keys,
            communicators.len()
        );
        self.update_sender.send_update(update, communicators);
    }

    fn return_query(&mut self, communicator: Uuid, values: FreshData<Key, Value>) {
        let keys = values.keys().collect::<Vec<_>>();
        event!(
            Level::DEBUG,
            "Fresh data will send {} new values to communicator {}",
            keys.len(),
            communicator
        );
        self.index.extend_index_with_query(communicator, keys);
        self.update_sender.send_fresh_data(values, &communicator);
    }

    fn resolve_finished_actions(&mut self) -> Vec<ResolvedAction<Key, Value>> {
        // NOTE: the `is_done` function here will poll the interal state of the
        // promise. I think this is nessesary since otherwise no work will be
        // done on the function
        self.running_actions
            .extract_if(ResolvingAction::poll_and_finished)
            .into_iter()
            .filter_map(|resolving_action| {
                event!(
                    Level::TRACE,
                    "Resolving action of type {} has finished and will be resolved",
                    resolving_action.action_type()
                );
                resolving_action.resolve()
            })
            .collect()
    }

    fn recive_new_actions(&mut self) {
        let new_action = self
            .reciver
            .recive_new()
            .into_iter()
            .map(|action| action.handle_action(&mut self.storage))
            .collect::<Vec<_>>();

        if !new_action.is_empty() {
            println!("there are {} new actions", new_action.len());
        }

        self.running_actions.extend(new_action);
    }
}

pub struct Reciver<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    action_reciver: mpsc::Receiver<Change<Key, Value>>,
    query_reciver: mpsc::Receiver<DataQuery<Key, Value>>,
    bk_action_sender: mpsc::Sender<Change<Key, Value>>,
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
        (self.bk_action_sender.clone(), self.bk_query_sender.clone())
    }
    fn recive_new(&mut self) -> Vec<Action<Key, Value>> {
        let mut new_actions: Vec<Action<Key, Value>> = vec![];
        while let Ok(val) = self.action_reciver.try_recv() {
            event!(Level::DEBUG, "Recived new change action");
            new_actions.push(val.into());
        }
        while let Ok(val) = self.query_reciver.try_recv() {
            event!(Level::DEBUG, "Recived new query action");
            new_actions.push(val.into());
        }
        new_actions
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
            bk_action_sender: action_sender,
            action_reciver,
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
        }
    }
}

impl<Key, Value> UpdateSender<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    fn register_senders(
        &mut self,
        communicator_uuid: &Uuid,
        change_sender: mpsc::Sender<DataChange<Key, Value>>,
        query_sender: mpsc::Sender<FreshData<Key, Value>>,
    ) {
        self.change_senders
            .insert(communicator_uuid.clone(), change_sender);
        self.query_senders
            .insert(communicator_uuid.clone(), query_sender);
    }
    fn send_update(&self, update: DataChange<Key, Value>, targets: Vec<&Uuid>) {
        event!(Level::TRACE, "Sending change data to {} targets", targets.len());
        self.change_senders
            .iter()
            .filter(|(uuid, _)| targets.contains(uuid))
            .for_each(|(_, sender)| {
                // FIXME: Not sure if this can be a blocking send
                let _ = sender.blocking_send(update.clone());
            });
    }

    fn send_fresh_data(&self, fresh_data: FreshData<Key, Value>, target: &Uuid) {
        event!(Level::TRACE, "Sending fresh data to communicator {}", target);
        self.query_senders
            .get(target)
            // FIXME: Not sure if this can be a blocking send
            .map(|sender| sender.blocking_send(fresh_data));
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
    pub fn communicators_from_keys(&self, keys: Vec<&Key>) -> Vec<&Uuid> {
        let mut vec_of_vecs = vec![];
        for key in keys.iter() {
            if let Some(uuids) = self.val_to_comm.get(key) {
                vec_of_vecs.extend(uuids);
            }
        }
        vec_of_vecs.into_iter().unique().collect()
    }

    pub fn extend_index_with_query(&mut self, communicator: Uuid, keys: Vec<&Key>) {
        keys.into_iter().for_each(|key| {
            let set = self
                .val_to_comm
                .entry(key.clone())
                .or_insert(HashSet::new());
            set.insert(communicator.clone());
        });
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

    fn resolve(self) -> Option<ResolvedAction<Key, Value>> {
        match self {
            ResolvingAction::Change(mut promise, sender) => {
                promise.take_value().map(|change_response| {
                    let (data_change, change_result) = change_response.into();
                    sender.send(change_result).unwrap();
                    data_change.map(|data| ResolvedAction::Change(data))
                })?
            }
            ResolvingAction::Query(mut promise, uuid, sender) => {
                promise.take_value().map(|query_response| {
                    let (fresh_data, result) = query_response.into();
                    sender.send(result).unwrap();
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
