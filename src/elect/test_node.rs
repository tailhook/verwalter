//! Tests for the case of single node
//!
use time::SteadyTime;

use super::{Info, Message};
use super::action::Action;
use super::machine::Machine;
use super::test_util::Environ;


#[test]
fn test_starting() {
    let mut env = Environ::new();
    let info = Info::new("one");
    let node: Machine = Machine::new(env.now());
    assert!(matches!(node, Machine::Starting { .. }));
}

#[test]
fn test_alone() {
    let mut env = Environ::new();
    let info = Info::new("one");
    let node = Machine::new(env.now());
    assert!(matches!(node, Machine::Starting { .. }));
    env.sleep(100);  // Small time, just continue starting
    let (node, act) = node.time_passed(&info, env.now());
    assert!(matches!(node, Machine::Starting { .. }));
    assert!(act.action == None);
    env.sleep(10000);  // Large timeout, should already become a leader
    let (node, act) = node.time_passed(&info, env.now());
    assert!(matches!(node, Machine::Leader { .. }));
    assert!(act.action == Some(Action::PingAll));
}

#[test]
fn test_start_vote() {
    let mut env = Environ::new();
    let mut info = Info::new("one");
    let node = Machine::new(env.now());
    let id = info.id.clone();
    assert!(matches!(node, Machine::Starting { .. }));

    env.add_another_for(&mut info);
    env.sleep(10000);  // Large timeout, should start_election
    let (node, act) = node.time_passed(&info, env.now());
    assert!(matches!(node, Machine::Electing { .. }));
    assert!(act.action == Some(Action::Vote(id.clone())));
}

#[test]
fn test_vote_approved() {
    let mut env = Environ::new();
    let mut info = Info::new("one");
    let node = Machine::new(env.now());
    let id = info.id.clone();
    assert!(matches!(node, Machine::Starting { .. }));

    let two = env.add_another_for(&mut info);
    env.sleep(10000);  // Large timeout, should start_election
    let (node, act) = node.time_passed(&info, env.now());
    assert!(matches!(node, Machine::Electing { .. }));
    assert!(act.action == Some(Action::Vote(id.clone())));

    let (node, act) = node.message(&info,
        (two.clone(), 1, Message::Vote(id.clone())), env.now());
    println!("Node {:?}", node);
    assert!(matches!(node, Machine::Leader { .. }));
    assert!(act.action == Some(Action::PingAll));
}

#[test]
fn test_election_expired() {
    let mut env = Environ::new();
    let mut info = Info::new("one");
    let node = Machine::new(env.now());
    let id = info.id.clone();
    assert!(matches!(node, Machine::Starting { .. }));

    let two = env.add_another_for(&mut info);
    env.sleep(10000);  // Large timeout, should start_election
    let (node, act) = node.time_passed(&info, env.now());
    assert!(matches!(node, Machine::Electing { epoch: 1, .. }));
    assert!(act.action == Some(Action::Vote(id.clone())));

    env.sleep(3000);  // Large timeout, should be enough for new election
    let (node, act) = node.time_passed(&info, env.now());
    assert!(matches!(node, Machine::Electing { epoch: 2, .. }));
    assert!(act.action == Some(Action::Vote(id.clone())));
}
