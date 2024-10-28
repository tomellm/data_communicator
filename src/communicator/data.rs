use std::{cmp::Ordering, collections::HashMap};

use itertools::Itertools;
use permutation::Permutation;
use tracing::{trace, warn};

use crate::{change::DataChange, query::FreshData, KeyBounds, ValueBounds};

type SortingFn<Value> = Box<dyn FnMut(&Value, &Value) -> Ordering + Send + 'static>;

pub struct Data<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    pub(super) data: HashMap<Key, Value>,
    pub(super) sorted: Permutation,
    sorting_fn: SortingFn<Value>,
}

impl<Key, Value> Data<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    #[must_use]
    pub(super) fn new() -> Self {
        let data = HashMap::new();
        let sorting_fn = |a: &Value, b: &Value| a.key().cmp(b.key());
        Self {
            data,
            sorted: permutation::sort_by(Vec::<Value>::new(), sorting_fn),
            sorting_fn: Box::new(sorting_fn),
        }
    }
    pub(super) fn add_fresh_data(&mut self, data: FreshData<Key, Value>) {
        self.extend(data.into());
    }
    /// Internally decides how the data is mutated depending in the data update
    /// state
    pub(super) fn update_data(&mut self, change: DataChange<Key, Value>) {
        match change {
            DataChange::Insert(values) => self.insert(values),
            DataChange::Update(values) => self.update(values),
            DataChange::Delete(keys) => self.delete(keys),
        }
    }
    pub(super) fn extend(&mut self, extend: HashMap<Key, Value>) {
        trace!(
            "About to extend this data object with {} values",
            extend.len()
        );
        self.data.extend(extend);
        self.resort();
    }
    pub(super) fn insert(&mut self, insert: Vec<Value>) {
        trace!(
            "About to insert {} new values in this data object",
            insert.len()
        );
        self.data
            .extend(insert.into_iter().map(|v| (v.key().clone(), v)));
        self.resort();
    }
    pub(super) fn update(&mut self, update: Vec<Value>) {
        trace!(
            "About to update {} values in this data object",
            update.len()
        );
        for value in update {
            let Some(old_value) = self.data.get_mut(value.key()) else {
                warn!("The value with id [{:?}] tried to be inserted through a update action, which is not correct. Use the insert action for insertion", value.key());
                continue;
            };
            *old_value = value;
        }
        self.resort();
    }
    pub(super) fn delete(&mut self, keys: Vec<Key>) {
        let mut count = 0;
        for key in keys.iter() {
            if self.data.remove(key).is_some() {
                count += 1;
            }
        }
        trace!("Delete {count} value from this data object");
        self.resort();
    }
    pub(super) fn resort(&mut self) {
        self.sorted = permutation::sort_by(self.data.values().collect_vec(), |a, b| {
            (self.sorting_fn)(*a, *b)
        })
    }
    pub(super) fn new_sorting_fn<F: FnMut(&Value, &Value) -> Ordering + Send + 'static>(
        &mut self,
        sorting_fn: F,
    ) {
        self.sorting_fn = Box::new(sorting_fn);
        self.resort();
    }
    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn keys(&self) -> Vec<&Key> {
        self.data.keys().collect_vec()
    }
    pub fn keys_cloned(&self) -> Vec<Key> {
        self.data.keys().cloned().collect_vec()
    }
    pub fn keys_iter(&self) -> impl Iterator<Item = &Key> {
        self.data.keys()
    }
    pub fn touples(&self) -> Vec<(&Key, &Value)> {
        self.data.iter().collect_vec()
    }
    pub fn map(&self) -> &HashMap<Key, Value> {
        &self.data
    }
    pub fn map_cloned(&self) -> HashMap<Key, Value> {
        self.data.clone()
    }
    pub fn iter(&self) -> impl Iterator<Item = &Value> + Clone {
        self.data.values()
    }
    pub fn cloned(&self) -> Vec<Value> {
        self.data.values().cloned().collect_vec()
    }
    pub fn sorted(&self) -> Vec<&Value> {
        self.sorted.apply_slice(self.data.values().collect_vec())
    }
    pub fn sorted_iter(&self) -> impl Iterator<Item = &Value> {
        self.sorted
            .apply_slice(self.data.values().collect_vec())
            .into_iter()
    }
    /// This has to take the data as sorted otherwise the pagination will make
    /// little sense and is potentially inconsistent
    pub fn page(&self, page: usize, per_page: usize) -> Option<Vec<&Value>> {
        self.sorted()
            .chunks(per_page)
            .nth(page)
            .map(|chunk| chunk.to_vec())
    }
}

impl<Key, Value> Default for Data<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    fn default() -> Self {
        Self::new()
    }
}
