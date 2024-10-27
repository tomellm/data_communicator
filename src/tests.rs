mod action;
mod communicators;
mod lib_impls;
mod sequential;

use std::{collections::HashMap, str::FromStr};

use action::ReadyAction;
use lib_impls::TestStruct;
use sequential::SequentialBuilder;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

use crate::{
    action, communicator::Communicator, container::DataContainer, pin_fut, query::QueryType,
};

type Comm = Communicator<usize, TestStruct>;
type Cont = DataContainer<usize, TestStruct, HashMap<usize, TestStruct>>;

fn sequential(len: usize) -> SequentialBuilder {
    SequentialBuilder::new(len)
}

fn multiply<const N: usize, T: Clone>(val: T) -> [T; N] {
    std::array::from_fn(|_| val.clone())
}

#[tokio::test]
async fn data_should_be_shared_to_everyone() {
    let [first_val_1, first_val_2] = multiply(TestStruct::new(1, "Hello One"));
    let [second_val_1, second_val_2] = multiply(TestStruct::new(2, "Hello Two"));
    let [third_val_1, third_val_2] = multiply(TestStruct::new(3, "Some more Data"));

    let actions = vec![
        action!(1, |comm| {
            pin_fut!({
                let _ = comm.query(QueryType::All).await;
                comm
            })
        }),
        action!(2, |comm: Comm| {
            pin_fut!({
                let _ = comm.query(QueryType::All).await;
                comm
            })
        }),
        action!(1, |comm: Comm| {
            pin_fut!({
                let _ = comm.insert(first_val_1).await;
                comm
            })
        }),
        action!(2, |comm: Comm| {
            pin_fut!({
                let _ = comm.insert(second_val_1).await;
                comm
            })
        }),
        action!(1, |comm: Comm| {
            pin_fut!({
                let _ = comm.insert(third_val_1).await;
                comm
            })
        }),
    ];

    let final_state = sequential(2).actions(actions).run().await;

    assert!(final_state.all_equal_in(|comm| comm.data.len()));
    assert!(final_state.all_contain(vec![&first_val_2, &second_val_2, &third_val_2]));
}

#[tokio::test]
async fn only_interesting_data_should_be_shared() {
    let [first_val_1] = multiply(TestStruct::new(1, "Hello One"));
    let [second_val_1] = multiply(TestStruct::new(2, "Hello Two"));
    let [third_val_1, third_val_2] = multiply(TestStruct::new(3, "Some more Data"));

    let actions = vec![
        action!(1, |comm| {
            pin_fut!({
                let _ = comm.query(QueryType::All).await;
                comm
            })
        }),
        action!(2, |comm: Comm| {
            pin_fut!({
                let _ = comm
                    .query(QueryType::predicate(|val: &TestStruct| {
                        val.val.contains(&String::from("Hello"))
                    }))
                    .await;
                comm
            })
        }),
        action!(1, |comm: Comm| {
            pin_fut!({
                let _ = comm.insert(first_val_1).await;
                comm
            })
        }),
        action!(1, |comm: Comm| {
            pin_fut!({
                let _ = comm.insert(second_val_1).await;
                comm
            })
        }),
        action!(1, |comm: Comm| {
            pin_fut!({
                let _ = comm.insert(third_val_1).await;
                comm
            })
        }),
    ];

    let final_state = sequential(2).actions(actions).run().await;

    assert!(final_state.comm_contains(1, &third_val_2));
    assert!(!final_state.comm_contains(2, &third_val_2));
}

#[tokio::test]
async fn update_should_change_values() {
    let [first_val_1, first_val_2] = multiply(TestStruct::new(1, "Hello One"));
    let [updated_val_1, updated_val_2] = multiply(TestStruct::new(1, "Hello Two"));

    let actions = vec![
        action!(1, |comm| {
            pin_fut!({
                let _ = comm.query(QueryType::All).await;
                comm
            })
        }),
        action!(2, |comm: Comm| {
            pin_fut!({
                let _ = comm.query(QueryType::All).await;
                comm
            })
        }),
        action!(1, |comm: Comm| {
            pin_fut!({
                let _ = comm.insert(first_val_1).await;
                comm
            })
        }),
        action!(2, |comm: Comm| {
            pin_fut!({
                let _ = comm.update(updated_val_1).await;
                comm
            })
        }),
    ];

    let final_state = sequential(2).actions(actions).run().await;

    assert!(final_state.comm_contains(1, &updated_val_2));
    assert!(!final_state.comm_contains(1, &first_val_2));
    assert!(final_state.comm_contains(2, &updated_val_2));
    assert!(!final_state.comm_contains(2, &first_val_2));
}
