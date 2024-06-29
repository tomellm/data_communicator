use std::collections::HashMap;

use super::{actions::ActionType, KeyBounds, ValueBounds};

pub struct Data<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>
{
    pub data: HashMap<Key, Value>
}

#[derive(Clone)]
pub enum DataUpdate<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>
{
    Update(Vec<Value>),
    Delete(Vec<Key>),
}

impl<Key, Value> DataUpdate<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>
{
    pub fn value_keys(&self) -> Vec<&Key> {
        match self {
            Self::Update(values) => values.iter().map(Value::key).collect::<Vec<_>>(),
            Self::Delete(keys) => keys.iter().map(|v|v).collect::<Vec<_>>(),
        }
    }

    /// Internally decides how the data is mutated depending in the data update
    /// state
    pub fn update_data(self, data: &mut Data<Key, Value>) {
        match self {
            Self::Update(values) => Self::update(values, data),
            Self::Delete(keys) => Self::delete(keys, data)
        }
    }

    fn update(values: Vec<Value>, data: &mut Data<Key, Value>) {
        values.into_iter().for_each(|value| {
            data.data.insert(value.key().clone(), value);
        });
    }
    fn delete(keys: Vec<Key>, data: &mut Data<Key, Value>) {
        keys.into_iter().for_each(|key| {
            data.data.remove(&key);
        });
    }
}

impl<Key, Value> From<ActionType<Key, Value>> for DataUpdate<Key, Value>
where
    Key: KeyBounds,
    Value: ValueBounds<Key>,
{
    fn from(value: ActionType<Key, Value>) -> Self {
        match value {
            ActionType::Update(val) => Self::Update(vec![val]),
            ActionType::UpdateMany(vals) => Self::Update(vals),
            ActionType::Delete(key) => Self::Delete(vec![key]),
            ActionType::DeleteMany(keys) => Self::Delete(keys),
        }
    }
}
