use std::collections::HashMap;

use crate::container::DataContainer;

use super::{action::ReadyAction, BoxFut, Comm, Cont};

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

    pub fn perform_action(&mut self, action: ReadyAction) -> BoxFut<(usize, Comm)> {
        let comm = self.map.remove(&action.which).unwrap();
        action.start(comm)
    }

    pub fn consume_result(&mut self, finished_action: (usize, Comm)) {
        self.map.insert(finished_action.0, finished_action.1);
    }
}
