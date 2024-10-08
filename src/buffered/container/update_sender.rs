use std::collections::HashMap;

use itertools::Itertools;
use lazy_async_promise::{BoxedSendError, ImmediateValuePromise, ImmediateValueState};
use tokio::sync::mpsc;
use tracing::{debug, enabled, trace, Level};
use uuid::Uuid;

use crate::buffered::{
    data::{DataChange, FreshData},
    utils::DrainIf,
    KeyBounds, ValueBounds,
};

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
    pub fn register_senders(
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
    pub fn send_change(&mut self, cont_uuid: &Uuid, targets: Vec<(Uuid, DataChange<Key, Value>)>) {
        trace!(
            msg = format!("Sending change data to {} targets", targets.len()),
            cont = cont_uuid.to_string()
        );

        let new_sending_responses = targets
            .into_iter()
            .map(|(target, change)| {
                let sender = self.change_senders.get(&target).unwrap().clone();
                let string_uuid = cont_uuid.to_string();
                ImmediateValuePromise::new(async move {
                    let send_res = sender.send(change).await.map_err(BoxedSendError::from);
                    debug!(
                        msg = format!("Sent off data change to communicator [{target}]."),
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
    pub fn send_fresh_data(
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
