mod action;
mod communicators;
mod lib_impls;

use std::{collections::HashMap, str::FromStr, time::Duration};

use action::ReadyAction;
use communicators::Communicators;
use futures::future::BoxFuture;
use itertools::Itertools;
use lib_impls::TestStruct;
use tokio::{sync::mpsc, task::JoinHandle, time::sleep};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

use crate::{communicator::Communicator, container::DataContainer, query::QueryType};

#[tokio::test]
async fn basic_setup() {
    let stdout_log = tracing_subscriber::fmt::layer();
    tracing_subscriber::registry()
        .with(stdout_log.with_filter(EnvFilter::from_str("info").unwrap()))
        .init();
    //console_subscriber::init();

    let mut all = Communicators::init(2).await;

    let mut actions = vec![
        ReadyAction::new(1, |comm: Comm| {
            Box::pin(async move {
                let _ = comm.query(QueryType::All).await;
                comm
            })
        }),
        ReadyAction::new(2, |comm: Comm| {
            Box::pin(async move {
                let _ = comm.query(QueryType::All).await;
                comm
            })
        }),
        ReadyAction::new(1, |comm: Comm| {
            Box::pin(async move {
                let _ = comm
                    .insert(TestStruct {
                        key: 1,
                        val: "Hello One".into(),
                    })
                    .await;
                comm
            })
        }),
        ReadyAction::new(2, |comm: Comm| {
            Box::pin(async move {
                let _ = comm
                    .insert(TestStruct {
                        key: 2,
                        val: "Hello Two".into(),
                    })
                    .await;
                comm
            })
        }),
        ReadyAction::new(1, |comm: Comm| {
            Box::pin(async move {
                let _ = comm
                    .insert(TestStruct {
                        key: 3,
                        val: "Some more Data".into(),
                    })
                    .await;
                comm
            })
        }),
    ];

    let (sender, mut reciver) = mpsc::channel(5);
    let mut waiting = false;

    loop {
        all.state_update();
        if let Ok(state) = reciver.try_recv() {
            all.consume_result(state);
            waiting = false;
        } else if !actions.is_empty() && !waiting {
            let next_action = actions.remove(0);
            let action_future = all.perform_action(next_action);
            let sender = sender.clone();
            tokio::task::spawn(async move {
                let output = action_future.await;
                let _ = sender.send(output).await;
            });
            waiting = true;
        } else if actions.is_empty() && !waiting {
            break;
        }
        sleep(Duration::from_millis(50)).await;
    }
    let all_equal = all.map.values().map(|comm| comm.data.len()).all_equal();
    assert!(all_equal)
}

type Comm = Communicator<usize, TestStruct>;
type Cont = DataContainer<usize, TestStruct, HashMap<usize, TestStruct>>;
type BoxFut<T> = BoxFuture<'static, T>;
