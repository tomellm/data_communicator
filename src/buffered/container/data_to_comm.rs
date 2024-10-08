use std::collections::{HashMap, HashSet};

use itertools::Itertools;
use uuid::Uuid;

use crate::buffered::KeyBounds;

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

impl<Key> DataToCommunicatorIndex<Key>
where
    Key: KeyBounds,
{
    pub fn communicators_from_keys(&self, keys: &[&Key]) -> Vec<&Uuid> {
        let mut vec_of_comms = vec![];
        for key in keys {
            if let Some(new_comm) = self.val_to_comm.get(key) {
                vec_of_comms.extend(new_comm);
            }
        }
        vec_of_comms.into_iter().unique().collect()
    }

    pub fn extend_index_with_query(&mut self, communicator: Uuid, keys: Vec<&Key>) {
        for key in keys {
            let set = self.val_to_comm.entry(key.clone()).or_default();
            set.insert(communicator);
        }
    }
}
