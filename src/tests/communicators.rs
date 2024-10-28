use std::collections::HashMap;

use futures::future::BoxFuture;
use itertools::Itertools;

use crate::container::DataContainer;

use super::{
    action::{self, Action, AssertAction, ReadyAction}, lib_impls::TestStruct, Comm, Cont
};

pub(super) struct Communicators {
    pub(super) container: Cont,
    pub(super) communicators: HashMap<usize, Comm>,
}

impl Communicators {
    pub async fn init(num: usize) -> Self {
        let mut container = DataContainer::init(()).await;
        let map = (1..=num)
            .map(|i| (i, container.communicator()))
            .collect::<HashMap<_, _>>();
        Self {
            container,
            communicators: map,
        }
    }

    pub fn state_update(&mut self) {
        self.container.state_update();
        self.communicators
            .values_mut()
            .for_each(|comm| comm.state_update());
    }

    pub fn perform_action(&mut self, action: Action) -> Option<BoxFuture<'static, (usize, Comm)>> {
        match action {
            Action::Action(ready_action) => Some(self.perform_ready_action(ready_action)),
            Action::Assert(assert_action) => {
                self.perform_assert(assert_action);
                None
            }
        }
    }

    pub fn perform_ready_action(
        &mut self,
        action: ReadyAction,
    ) -> BoxFuture<'static, (usize, Comm)> {
        let (num, comm) = self.communicators.remove_entry(&action.which).unwrap();
        Box::pin(async move {
            let comm = action.start(comm).await;
            (num, comm)
        })
    }

    pub fn perform_assert(&self, action: AssertAction) {
        (action.assert)(self);
    }

    pub fn reinsert(&mut self, num: usize, comm: Comm) {
        self.communicators.insert(num, comm);
    }

    pub(super) fn all_equal_in<T: Eq>(&self, func: impl Fn(&Comm) -> T) -> bool {
        self.communicators.values().map(func).all_equal()
    }
    pub(super) fn all_contain(&self, values: Vec<&TestStruct>) -> bool {
        self.all_true_in(|comm| {
            values
                .iter()
                .map(|val| comm.data().contains(val))
                .all(|cont| cont)
        })
    }
    pub(super) fn all_true_in(&self, func: impl Fn(&Comm) -> bool) -> bool {
        self.communicators.values().map(func).all(|val|val)
    }

    pub(super) fn comm_contains(&self, num: usize, val: &TestStruct) -> bool {
        self.communicators.get(&num).unwrap().data().contains(&val)
    }
}
