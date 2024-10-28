use futures::future::BoxFuture;

use super::{communicators::Communicators, Comm};

pub(super) enum Action {
    Action(ReadyAction),
    Assert(AssertAction),
}

pub(super) struct AssertAction {
    pub(super) assert: AssertFn,
}

type AssertFn = Box<dyn FnOnce(&Communicators)>;

pub(super) struct ReadyAction {
    pub(super) which: usize,
    pub(super) action: ActionFn,
}

type ActionFn = Box<dyn FnOnce(Comm) -> BoxFuture<'static, Comm> + Send>;

impl ReadyAction {
    pub(super) fn new<F>(which: usize, action: F) -> Self
    where
        F: FnOnce(Comm) -> BoxFuture<'static, Comm> + Send + 'static,
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


/// Macor to easily create [Action::Action]
/// Is shorthand for:
/// ```
/// Action::Action(
///     ReadyAction::new($num, |comm: Comm| Box::pin(async move {
///         $fun(comm).await
///     }))
/// )
/// ```
/// $num : Is the index for the communicator to work on.
/// $func : A function defining the change to make. Important is that it returns
/// the communicator.
/// ```
/// |comm: Comm| async move {
///     // -- your change to the communicator
///     comm
/// }
/// ```
#[macro_export]
macro_rules! ready_action {
    ($num: expr, $func: expr) => {
        $crate::tests::action::Action::Action(
            $crate::tests::action::ReadyAction::new($num, |comm: Comm| Box::pin(async move {
                $func(comm).await
            }))
        )
    };
}

/// Macro to more easily create [Action::Assert]
/// Is a shorthand for:
/// ```
/// Action::Assert(
///     AssertAction {
///         assert: Box::new($assert)
///     }
/// )
/// ```
/// The function passed to the macro should look like this:
/// ```
/// |data: &Communicators| {
///     // -- your asserts
/// }
/// ```
#[macro_export]
macro_rules! assert_action {
    ($assert: expr) => {
        $crate::tests::action::Action::Assert(
            $crate::tests::action::AssertAction {
                assert: Box::new($assert)
            }
        )
    };
}
