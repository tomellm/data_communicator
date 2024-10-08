use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
};
use tokio::sync::watch::{self, Ref};

use super::{changer, storage::GetKey};

#[derive(Clone)]
pub struct Communicator<Key, Value>
where
    Key: Clone + Send + Sync,
    Value: GetKey<Key> + Clone + Send + Sync,
{
    viewer: watch::Receiver<HashMap<Key, Value>>,
    changer: changer::Sender<Key, Value>,
}

impl<Key, Value> Communicator<Key, Value>
where
    Key: Clone + Send + Sync,
    Value: GetKey<Key> + Clone + Send + Sync,
{
    #[must_use]
    pub(crate) fn new(
        viewer: watch::Receiver<HashMap<Key, Value>>,
        changer: changer::Sender<Key, Value>,
    ) -> Self {
        Self { viewer, changer }
    }
    #[must_use]
    pub fn view(&self) -> Ref<'_, HashMap<Key, Value>> {
        self.viewer.borrow()
    }
    pub fn update(&mut self) {
        self.changer.update_sender();
    }
    #[must_use]
    pub fn viewer(&self) -> watch::Receiver<HashMap<Key, Value>> {
        self.viewer.clone()
    }
}

impl<Key, Value> Deref for Communicator<Key, Value>
where
    Key: Clone + Send + Sync,
    Value: GetKey<Key> + Clone + Send + Sync,
{
    type Target = changer::Sender<Key, Value>;
    fn deref(&self) -> &Self::Target {
        &self.changer
    }
}

impl<Key, Value> DerefMut for Communicator<Key, Value>
where
    Key: Clone + Send + Sync,
    Value: GetKey<Key> + Clone + Send + Sync,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.changer
    }
}
