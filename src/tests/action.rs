use futures::future::BoxFuture;

use super::Comm;

type ActionFn = Box<dyn FnOnce(Comm) -> BoxFuture<'static, Comm> + Send>;

pub(super) struct ReadyAction {
    pub(super) which: usize,
    pub(super) action: ActionFn,
}

impl ReadyAction {
    pub(super) fn new<F>(which: usize, action: F) -> Self
    where
        F: FnOnce(Comm) -> BoxFuture<'static, Comm> + Send + 'static
    {
        Self {
            which,
            action: Box::new(action),
        }
    }

    pub(super) fn start(self, communicator: Comm) -> BoxFuture<'static, Comm> {
        (self.action)(communicator)
    }
}


#[macro_export]
macro_rules! action {
    ($num: expr, $fun: expr) => {
        ReadyAction::new($num, $fun)
    };
}


#[macro_export]
macro_rules! pin_fut {
    ($func: expr) => {
        Box::pin(async move { $func })
    };
}
