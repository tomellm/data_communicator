use std::collections::{HashMap, HashSet};

use itertools::Itertools;
use uuid::Uuid;

use crate::buffered::{
    data::{DataChange, FreshData},
    query::{DataQuery, QueryType},
    GetKeys, KeyBounds, ValueBounds,
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

    /// Takes in a proposed data change. Will then use the info it stores to figure
    /// out which communicators are interested inthat change.
    ///
    /// If the change is only an update or a delete then the it will only compare
    /// the changing values to the already stored values. If the change is an
    /// insert then it will also check if the values mach the last performed query.
    pub fn get_interested_comm(
        &self,
        update: &DataChange<Key, Value>,
    ) -> Vec<(Uuid, DataChange<Key, Value>)> {
        self.comm_to_info
            .iter()
            .filter_map(|(comm, info)| {
                let comm_update = match update {
                    DataChange::Insert(values) => {
                        let new_values = values
                            .into_iter()
                            .filter(|value| {
                                let was_last_queried = info.value_keys.contains(&value.key());
                                let new_data_matches_query =
                                    if let Some(query_type) = &info.last_query {
                                        query_type.apply(value)
                                    } else {
                                        false
                                    };
                                was_last_queried || new_data_matches_query
                            })
                            .cloned()
                            .collect::<Vec<_>>();
                        DataChange::Insert(new_values)
                    }
                    DataChange::Update(values) => DataChange::Update(
                        values
                            .into_iter()
                            .filter(|value| info.value_keys.contains(&value.key()))
                            .cloned()
                            .collect::<Vec<_>>(),
                    ),
                    DataChange::Delete(keys) => {
                        let new_keys = keys
                            .into_iter()
                            .filter(|key| info.value_keys.contains(&key))
                            .cloned()
                            .collect::<Vec<_>>();
                        DataChange::Delete(new_keys)
                    }
                };
                match comm_update.len() {
                    0 => None,
                    _ => Some((*comm, comm_update)),
                }
            })
            .collect_vec()
    }

    /// Update the internal info object to reflect the data each communicator
    /// contains. Performed when any change action is taken.
    ///
    /// Ignores the Update case since that doesnt add or remove new keys.
    pub fn update_info_from_change(&mut self, target: &Uuid, update: &DataChange<Key, Value>) {
        let value_keys = &mut self.comm_to_info.get_mut(target).unwrap().value_keys;
        match update {
            DataChange::Insert(values) => {
                value_keys.extend(values.keys().into_iter().cloned().collect_vec());
            }
            DataChange::Delete(keys) => keys.into_iter().for_each(|key| {
                value_keys.remove(key);
            }),
            DataChange::Update(_) => (),
        };
    }

    /// Update the internal info object to reflect the data each communicator
    /// contains. Perfomed when the communicator queries for data.
    pub fn update_info_from_query(&mut self, target: &Uuid, fresh_data: &FreshData<Key, Value>) {
        let value_keys = &mut self.comm_to_info.get_mut(target).unwrap().value_keys;
        value_keys.clear();
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
