use std::{
    cmp::Ordering, collections::HashMap, ops::{Deref, DerefMut}
};

use itertools::Itertools;
use permutation::Permutation;
use tracing::{trace, warn};

use super::{change::ChangeType, GetKeys, KeyBounds, ValueBounds};

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
    pub fn new() -> Self {
        let data = HashMap::new();
        let sorting_fn = |a: &Value, b: &Value| a.key().cmp(b.key());
        Self {
            data,
            sorted: permutation::sort_by(Vec::<Value>::new(), sorting_fn),
            sorting_fn: Box::new(sorting_fn),
        }
    }
    pub fn extend(&mut self, extend: HashMap<Key, Value>) {
        trace!(
            "About to extend this data object with {} values",
            extend.len()
        );
        self.data.extend(extend);
        self.resort();
    }
    pub fn insert(&mut self, insert: Vec<Value>) {
        trace!(
            "About to insert {} new values in this data object",
            insert.len()
        );
        self.data
            .extend(insert.into_iter().map(|v| (v.key().clone(), v)));
        self.resort();
        
    }
    pub fn update(&mut self, update: Vec<Value>) {
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
    pub fn delete(&mut self, keys: Vec<Key>) {
        let mut count = 0;
        for key in keys.iter() {
            if self.data.remove(key).is_some() {
                count += 1;
            }
        }
        trace!("Delete {count} value from this data object");
        self.resort();
    }
    pub fn resort(&mut self) {
        self.sorted = permutation::sort_by(self.data.values().collect_vec(), |a, b| {
            (self.sorting_fn)(*a, *b)
        })
    }
    pub fn new_sorting_fn<F: FnMut(&Value, &Value) -> Ordering + Send + 'static>(
        &mut self,
        sorting_fn: F,
    ) {
        self.sorting_fn = Box::new(sorting_fn);
        self.resort();
    }
    pub fn len(&self) -> usize {
        self.data.len()
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
        self.sorted.apply_slice(self.data.values().collect_vec()).into_iter()
    }
    /// This has to take the data as sorted otherwise the pagination will make 
    /// little sense and is potentially inconsistent
    pub fn page(&mut self, page: usize, per_page: usize) -> Option<Vec<&Value>> {
        self.sorted().chunks(per_page).nth(page).map(|chunk| chunk.to_vec())
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

#[derive(Clone)]
pub struct FreshData<Key, Value>(HashMap<Key, Value>);

impl<Key, Value> FreshData<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    pub fn add_fresh_data(self, data: &mut Data<Key, Value>) {
        data.extend(HashMap::from(self));
    }
}

impl<Key, Value> Deref for FreshData<Key, Value> {
    type Target = HashMap<Key, Value>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<Key, Value> DerefMut for FreshData<Key, Value> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<Key, Value> From<Value> for FreshData<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    fn from(value: Value) -> Self {
        let mut map = HashMap::new();
        map.insert(value.key().clone(), value);
        FreshData(map)
    }
}

impl<Key, Value> From<Vec<Value>> for FreshData<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    fn from(value: Vec<Value>) -> Self {
        FreshData(
            value
                .into_iter()
                .map(|item| (item.key().clone(), item))
                .collect(),
        )
    }
}

impl<Key, Value> From<HashMap<Key, Value>> for FreshData<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    fn from(value: HashMap<Key, Value>) -> Self {
        Self(value)
    }
}

#[allow(clippy::implicit_hasher)]
impl<Key, Value> From<FreshData<Key, Value>> for HashMap<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    fn from(value: FreshData<Key, Value>) -> Self {
        value.0
    }
}

#[derive(Clone)]
pub enum DataChange<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    // TODO: maybe add a None option, for when no change is made
    // not sure tho if that should rather be a wrapping of this object
    // in a Option
    Insert(Vec<Value>),
    Update(Vec<Value>),
    Delete(Vec<Key>),
}

impl<Key, Value> DataChange<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    pub fn empty_insert() -> Self {
        Self::Insert(vec![])
    }
    pub fn empty_update() -> Self {
        Self::Update(vec![])
    }
    pub fn empty_delete() -> Self {
        Self::Delete(vec![])
    }
    pub fn value_keys(&self) -> Vec<&Key> {
        match self {
            Self::Insert(values) => values.keys(),
            Self::Update(values) => values.keys(),
            Self::Delete(keys) => keys.keys(),
        }
    }

    /// Internally decides how the data is mutated depending in the data update
    /// state
    pub fn update_data(self, data: &mut Data<Key, Value>) {
        match self {
            Self::Insert(values) => data.insert(values),
            Self::Update(values) => data.update(values),
            Self::Delete(keys) => data.delete(keys),
        }
    }
    pub fn len(&self) -> usize {
        match self {
            Self::Insert(values) => values.len(),
            Self::Update(values) => values.len(),
            Self::Delete(keys) => keys.len(),
        }
    }

    pub fn is_insert(&self) -> bool {
        match self {
            Self::Insert(_) => true,
            _ => false
        }
    }

    pub fn is_update(&self) -> bool {
        match self {
            Self::Update(_) => true,
            _ => false
        }
    }

    pub fn is_delete(&self) -> bool {
        match self {
            Self::Delete(_) => true,
            _ => false
        }
    }
}

impl<Key, Value> From<ChangeType<Key, Value>> for DataChange<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    fn from(value: ChangeType<Key, Value>) -> Self {
        match value {
            ChangeType::Insert(val) => Self::Insert(vec![val]),
            ChangeType::InsertMany(vals) => Self::Insert(vals),
            ChangeType::Update(val) => Self::Update(vec![val]),
            ChangeType::UpdateMany(vals) => Self::Update(vals),
            ChangeType::Delete(key) => Self::Delete(vec![key]),
            ChangeType::DeleteMany(keys) => Self::Delete(keys),
        }
    }
}
