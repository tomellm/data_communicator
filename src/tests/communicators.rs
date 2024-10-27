use std::collections::HashMap;

use futures::future::BoxFuture;

use crate::container::DataContainer;

use super::{action::ReadyAction, Comm, Cont};

pub(super) struct Communicators {
    pub(super) container: Cont,
    pub(super) map: HashMap<usize, Comm>,
}

impl Communicators {
    pub async fn init(num: usize) -> Self {
        let mut container = DataContainer::init(()).await;
        let map = (1..=num)
            .map(|i| (i, container.communicator()))
            .collect::<HashMap<_, _>>();
        Self { container, map }
    }

    pub fn state_update(&mut self) {
        self.container.state_update();
        self.map.values_mut().for_each(|comm| comm.state_update());
    }

    pub fn perform_action(
        &mut self,
        action: ReadyAction,
    ) -> BoxFuture<'static, (usize, Comm)> {
        let (num, comm) = self.map.remove_entry(&action.which).unwrap();
        Box::pin(async move {
            let comm = action.start(comm).await;
            (num, comm)
        })
    }

    pub fn reinsert(
        &mut self,
        num: usize,
        comm: Comm
    ) {
        self.map.insert(num, comm);
    }

}
