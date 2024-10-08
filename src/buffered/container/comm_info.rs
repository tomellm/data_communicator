use std::collections::{HashMap, HashSet};

use itertools::Itertools;
use uuid::Uuid;

use crate::buffered::{
    data::{DataChange, FreshData},
    query::{DataQuery, QueryType},
    KeyBounds, ValueBounds,
};

pub struct CommunicatorInfo<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    comm_to_info: HashMap<Uuid, Info<Key, Value>>,
}

impl<Key, Value> Default for CommunicatorInfo<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    fn default() -> Self {
        Self {
            comm_to_info: HashMap::new(),
        }
    }
}

impl<Key, Value> CommunicatorInfo<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    pub fn register_comm(&mut self, comm_uuid: &Uuid) {
        self.comm_to_info.insert(*comm_uuid, Info::default());
    }
    pub fn update_query(&mut self, query: &DataQuery<Key, Value>) {
        let Some(info) = self.comm_to_info.get_mut(&query.origin_uuid) else {
            unreachable!();
        };
        info.last_query = Some(query.query_type.clone());
    }

    /// Takes in a target communicator and the proposed data change. Will then
    /// use the info it stores about the communicators to evaluate every single
    /// data point in the data change and if it should be sent to the respective
    /// communicator. The two important things to note here are:
    ///     - The data change can contain more data then the communicator is
    ///     interested in meaning all of the unnessesary data should be stripped
    ///     away
    ///     - Newly created data will be evaluated and passed on to the
    ///     communicator if the last performed query deems it to match.
    pub fn get_interested_comm(
        &self,
        update: &DataChange<Key, Value>,
    ) -> Vec<(Uuid, DataChange<Key, Value>)> {
        self.comm_to_info
            .iter()
            .filter_map(|(comm, Info { value_keys, last_query })| {
                let comm_update = match update {
                    DataChange::Update(values) => {
                        let new_values = values
                            .into_iter()
                            .filter(|value| {
                                let was_last_queried = value_keys.contains(&value.key());
                                let new_data_matches_query = if let Some(query_type) = last_query {
                                    query_type.apply(value)
                                } else {
                                    false
                                };
                                was_last_queried || new_data_matches_query
                            })
                        .cloned()
                            .collect::<Vec<_>>();
                        DataChange::Update(new_values)
                    }
                    DataChange::Delete(keys) => {
                        let new_keys = keys
                            .into_iter()
                            .filter(|key| value_keys.contains(&key))
                            .cloned()
                            .collect::<Vec<_>>();
                        DataChange::Delete(new_keys)
                    }
                };
                match comm_update.len() {
                    0 => None,
                    _ => Some((*comm, comm_update))
                }
            })
        .collect_vec()
        
    }

    /// The data in the data change object will eventually get to the communicator
    /// dictating what data that communicator will view. Here we update the
    /// containers view on what data which communicator has. Specifically we add
    /// any new values that the communicator gets or remove any from the list
    /// which the communicator will also delete.
    pub fn update_info_from_change(&mut self, target: &Uuid, update: &DataChange<Key, Value>) {
        let value_keys = &mut self.comm_to_info.get_mut(target).unwrap().value_keys;
        match update {
            DataChange::Update(values) => value_keys.extend(
                values
                    .into_iter()
                    .map(|value| value.key())
                    .cloned()
                    .collect_vec(),
            ),
            DataChange::Delete(keys) => keys.into_iter().for_each(|key| {
                value_keys.remove(key);
            }),
        };
    }
    pub fn update_info_from_query(&mut self, target: &Uuid, fresh_data: &FreshData<Key, Value>) {
        let value_keys = &mut self.comm_to_info.get_mut(target).unwrap().value_keys;
        value_keys.extend(fresh_data.keys().cloned());
    }
}

pub struct Info<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    value_keys: HashSet<Key>,
    last_query: Option<QueryType<Key, Value>>,
}

impl<Key, Value> Default for Info<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    fn default() -> Self {
        Self {
            value_keys: HashSet::new(),
            last_query: None,
        }
    }
}
