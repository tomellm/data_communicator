mod action;
mod communicators;
mod lib_impls;
mod sequential;

use std::collections::HashMap;

use itertools::Itertools;
use lib_impls::TestStruct;
use sequential::SequentialBuilder;

use crate::{
    assert_action,
    communicator::Communicator,
    container::DataContainer,
    query::QueryType,
    query_action, ready_action,
};

type Comm = Communicator<usize, TestStruct>;
type Cont = DataContainer<usize, TestStruct, HashMap<usize, TestStruct>>;

fn sequential(len: usize) -> SequentialBuilder {
    SequentialBuilder::new(len)
}

fn multiply<const N: usize, T: Clone>(val: T) -> [T; N] {
    std::array::from_fn(|_| val.clone())
}

fn n_objects(number: usize, str: &str) -> Vec<TestStruct> {
    (0..number)
        .map(|n| TestStruct::new(n, str))
        .collect_vec()
}

#[tokio::test]
async fn data_should_be_shared_to_everyone() {
    let [first_val_1, first_val_2] = multiply(TestStruct::new(1, "Hello One"));
    let [second_val_1, second_val_2] = multiply(TestStruct::new(2, "Hello Two"));
    let [third_val_1, third_val_2] = multiply(TestStruct::new(3, "Some more Data"));

    let actions = vec![
        query_action!(1, QueryType::All),
        query_action!(2, QueryType::All),
        ready_action!(1, |comm: Comm| async move {
            let _ = comm.insert(first_val_1).await;
            comm
        }),
        ready_action!(2, |comm: Comm| async move {
            let _ = comm.insert(second_val_1).await;
            comm
        }),
        ready_action!(1, |comm: Comm| async move {
            let _ = comm.insert(third_val_1).await;
            comm
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
        query_action!(1, QueryType::All),
        query_action!(
            2,
            QueryType::predicate(|val: &TestStruct| { val.val.contains(&String::from("Hello")) })
        ),
        ready_action!(1, |comm: Comm| async move {
            let _ = comm.insert(first_val_1).await;
            comm
        }),
        ready_action!(1, |comm: Comm| async move {
            let _ = comm.insert(second_val_1).await;
            comm
        }),
        ready_action!(1, |comm: Comm| async move {
            let _ = comm.insert(third_val_1).await;
            comm
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
        query_action!(1, QueryType::All),
        query_action!(2, QueryType::All),
        ready_action!(1, |comm: Comm| async move {
            let _ = comm.insert(first_val_1).await;
            comm
        }),
        ready_action!(2, |comm: Comm| async move {
            let _ = comm.update(updated_val_1).await;
            comm
        }),
    ];

    let final_state = sequential(2).actions(actions).run().await;

    assert!(final_state.comm_contains(1, &updated_val_2));
    assert!(!final_state.comm_contains(1, &first_val_2));
    assert!(final_state.comm_contains(2, &updated_val_2));
    assert!(!final_state.comm_contains(2, &first_val_2));
}

#[tokio::test]
async fn deleting_value_should_propagate_change() {
    let [first_val_1, first_val_2] = multiply(TestStruct::new(1, "Hello One"));
    let [to_delete_1, to_delete_2] = multiply(TestStruct::new(2, "Hello Two"));

    let actions = vec![
        query_action!(1, QueryType::All),
        query_action!(2, QueryType::All),
        ready_action!(1, |comm: Comm| async move {
            let _ = comm.insert(first_val_1).await;
            comm
        }),
        ready_action!(2, |comm: Comm| async move {
            let _ = comm.insert(to_delete_1).await;
            comm
        }),
        assert_action!(|data| { assert!(data.all_equal_in(|comm| comm.data.len() == 2)) }),
        ready_action!(1, |comm: Comm| async move {
            let _ = comm.delete(2).await;
            comm
        }),
    ];

    let final_state = sequential(2).actions(actions).run().await;

    assert!(final_state.comm_contains(1, &first_val_2));
    assert!(!final_state.comm_contains(1, &to_delete_2));
    assert!(final_state.comm_contains(2, &first_val_2));
    assert!(!final_state.comm_contains(2, &to_delete_2));
}

#[tokio::test]
async fn new_insert_should_change_has_changed_flag() {
    let [val] = multiply(TestStruct::new(1, "Hello One"));

    let actions = vec![
        query_action!(1, QueryType::All),
        assert_action!(|data| {
            let comm = data.communicators.get(&1).unwrap();
            assert!(comm.is_empty());
            assert!(comm.has_changed());
        }),
        ready_action!(1, |mut comm: Comm| async move {
            comm.set_viewed();
            comm
        }),
        assert_action!(|data| { assert!(!data.communicators.get(&1).unwrap().has_changed()) }),
        ready_action!(1, |comm: Comm| async move {
            let _ = comm.insert(val).await;
            comm
        }),
        assert_action!(|data| { assert!(data.communicators.get(&1).unwrap().has_changed()) }),
    ];

    sequential(1).actions(actions).run().await;
}

#[tokio::test]
async fn sort_data_should_sort_data_accordingly() {
    let [a_1, a_2] = multiply(TestStruct::new(1, "A"));
    let [b_1, b_2] = multiply(TestStruct::new(2, "B"));
    let [c_1, c_2] = multiply(TestStruct::new(3, "C"));

    let actions = vec![
        ready_action!(1, |comm: Comm| async move {
            let _ = comm.query(QueryType::All).await;
            comm
        }),
        ready_action!(1, |mut comm: Comm| async move {
            let _ = comm.insert_many(vec![b_1, c_1, a_1]).await;
            comm.sort(|a, b| a.val.cmp(&b.val));
            comm
        }),
        assert_action!(|data| {
            assert!(data
                .communicators
                .get(&1)
                .unwrap()
                .data
                .sorted_iter()
                .cloned()
                .eq(vec![a_2, b_2, c_2]));
        }),
    ];

    sequential(1).actions(actions).run().await;
}

#[tokio::test]
async fn pagination_sould_return_correct_page_and_page_size() {
    let [values] = multiply(n_objects(30, "test"));

    let actions = vec![
        query_action!(1, QueryType::All),
        ready_action!(1, |comm: Comm| async move {
            let _ = comm.insert_many(values).await;
            comm
        }),
        assert_action!(move |data| {
            let comm = data.get(1);
            let values = (0..10).map(|n| *comm.data.sorted().get(n).unwrap()).collect_vec();
            assert_eq!(comm.data.page(0, 10), Some(values));
            let values = (20..30).map(|n| *comm.data.sorted().get(n).unwrap()).collect_vec();
            assert_eq!(comm.data.page(1, 20), Some(values));
        })
    ];

    sequential(1).actions(actions).run().await;
}
