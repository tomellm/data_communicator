//! Single point of thrugh that interacts with the Storage.
//!
//! All of the queries and changes are sent here to the [`DataContainer`] which
//! then processes them one by one and relays the changes back to every single 
//! communicator that was created.
//!
//! The queries the [`DataContainer`] knows which [`Communicator`][crate::communicator::Communicator] 
//! is interested in which data and will then also only send the imporant data
//! to every single communicator.
//!
//! ### Key Information
//! - Instantiate the container with [`init`][DataContainer::init]
//! - Create any number of communicators with either [`communicator`][DataContainer::communicator]
//!     or [`communicators`][DataContainer::communicators]
//! - Finally don't forget to call [`state_update`][DataContainer::state_update]
mod comm_info;
mod reciver;
mod resolving_actions;
pub mod storage;
mod update_sender;

use comm_info::CommunicatorInfo;
use itertools::Itertools;
use reciver::Reciver;
use resolving_actions::{Action, ResolvedAction, ResolvingAction};
use storage::Storage;
use tokio::sync::mpsc;
use tracing::{debug, info, trace};
use update_sender::UpdateSender;
use uuid::Uuid;

use crate::{change::DataChange, query::FreshData};

use super::{communicator::Communicator, utils::DrainIf, KeyBounds, ValueBounds};

pub struct DataContainer<Key, Value, Writer>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
    Writer: Storage<Key, Value>,
{
    uuid: Uuid,
    reciver: Reciver<Key, Value>,
    update_sender: UpdateSender<Key, Value>,
    storage: Writer,
    comm_info: CommunicatorInfo<Key, Value>,
    running_actions: Vec<ResolvingAction<Key, Value>>,
}

impl<Key, Value, Writer> DataContainer<Key, Value, Writer>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
    Writer: Storage<Key, Value>,
{
    pub fn init(
        storage_args: Writer::InitArgs,
    ) -> impl std::future::Future<Output = Self> + Send + 'static {
        let storage_future = Writer::init(storage_args);
        async move {
            Self {
                uuid: Uuid::new_v4(),
                reciver: Reciver::default(),
                update_sender: UpdateSender::default(),
                comm_info: CommunicatorInfo::default(),
                storage: storage_future.await,
                running_actions: Vec::default(),
            }
        }
    }

    /// Does the following things:
    /// - Updates the internal sender
    /// - Resolves any actions that might be finished. With the finished query
    ///     or change they either
    ///     - Change: update all communicators that are interested
    ///     - Query: return data to the respective communicator
    /// - Recieve any new Actions
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

        // WARNING: if a page is not visited in a while, these could easily fill up
        let (change_data_sender, change_data_reciver) = mpsc::channel(20);
        let (fresh_data_sender, fresh_data_reciver) = mpsc::channel(20);

        self.update_sender
            .register_senders(&new_uuid, change_data_sender, fresh_data_sender);
        self.comm_info.register_comm(&new_uuid);

        Communicator::new(
            new_uuid,
            change_sender,
            query_sender,
            change_data_reciver,
            fresh_data_reciver,
        )
    }

    pub fn communicators<const N: usize>(&mut self) -> [Communicator<Key, Value>; N] {
        std::array::from_fn(|_| self.communicator())
    }

    /// Takes a fresh [`DataChange`] which is then cloned and fitted to every
    /// interested communicator and finally sent to each communicator.
    fn update_communicators(&mut self, update: &DataChange<Key, Value>) {
        let keys = update.value_keys();
        let communicators = self.comm_info.get_interested_comm(update);
        communicators.iter().for_each(|(target, change)| {
            self.comm_info.update_info_from_change(target, change);
        });

        debug!(
            msg = format!(
                "Recived data update will modify {} keys and go to {} communicators",
                keys.len(),
                communicators.len()
            ),
            cont = self.uuid.to_string()
        );

        self.update_sender.send_change(&self.uuid, communicators);
    }

    /// Takes the [`FreshData`] object and retrives the keys of it to update which
    /// values the communicator is interested in and then finally sends the object
    /// to the communicator.
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
        self.comm_info
            .update_info_from_query(&communicator, &values);
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
                        "Resolving action of type [{}] has finished and will be resolved",
                        resolving_action.action_type()
                    ),
                    cont = self.uuid.to_string()
                );
                resolving_action.resolve(&self.uuid)
            })
            .collect_vec()
    }

    /// Revives any new actions from the Revicers and then calls the respective
    /// methods on the [`Storage`] implementation. The returned futures are then
    /// placed in a vector to be retrived once done.
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
                match action {
                    Action::Change(change) => ResolvingAction::Change(
                        self.storage.handle_change(change.action),
                        change.reponse_sender,
                    ),
                    Action::Query(query) => {
                        self.comm_info.update_query(&query);
                        ResolvingAction::Query(
                            self.storage.handle_query(query.query_type),
                            query.origin_uuid,
                            query.response_sender,
                        )
                    }
                }
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
