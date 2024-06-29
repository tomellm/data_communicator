
use tokio::sync::oneshot;

use super::{responses::ActionResult, KeyBounds, ValueBounds};

pub struct Action<Key: KeyBounds, Value: ValueBounds<Key>> {
    pub reponse_sender: oneshot::Sender<ActionResult>,
    pub action: ActionType<Key, Value>
}

impl<Key: KeyBounds, Value: ValueBounds<Key>> Action<Key, Value> {
    pub fn from_type(
        action_type: ActionType<Key, Value>
    ) -> (Self, oneshot::Receiver<ActionResult>) {
        let (sender, reciver) = oneshot::channel::<ActionResult>();

        (
            Self { reponse_sender: sender, action: action_type },
            reciver
        )
    }

    pub fn all_keys(&self) -> Vec<&Key> {
        match &self.action {
            ActionType::Update(val) => vec![val.key()],
            ActionType::UpdateMany(vals) => vals.iter().map(|v|v.key()).collect(),
            ActionType::Delete(key) => vec![key],
            ActionType::DeleteMany(keys) => keys.into_iter().collect()
        }
    }
}

pub enum ActionType<Key: KeyBounds, Value: ValueBounds<Key>> {
    Update(Value),
    UpdateMany(Vec<Value>),
    Delete(Key),
    DeleteMany(Vec<Key>)
}

