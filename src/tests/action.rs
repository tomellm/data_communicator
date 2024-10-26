use futures::future::BoxFuture;

use super::{BoxFut, Comm};

type ActionFn = Box<dyn FnOnce(Comm) -> BoxFuture<'static, (usize, Comm)>>;

pub(super) struct ReadyAction {
    pub(super) which: usize,
    pub(super) action: ActionFn,
}

impl ReadyAction {
    pub(super) fn new<Function>(which: usize, action: Function) -> Self
    where
        Function: FnOnce(Comm) -> BoxFut<Comm> + Send + 'static,
    {
        Self {
            which,
            action: Box::new(move |comm| {
                Box::pin(async move {
                    let comm = action(comm).await;
                    (which, comm)
                })
            }),
        }
    }

    pub(super) fn start(self, communicator: Comm) -> BoxFut<(usize, Comm)> {
        (self.action)(communicator)
    }
}
