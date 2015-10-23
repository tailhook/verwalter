//! Tests for the case of single node
//!
use time::SteadyTime;

use super::{Node, Machine};
use super::action::Action;
use super::test_util::Environ;


#[test]
fn test_starting() {
    let mut scale = Environ::new();
    let node = Node::new("one", scale.now());
    assert!(matches!(node.machine, Machine::Starting { .. }));
}

#[test]
fn test_alone() {
    let mut scale = Environ::new();
    let node = Node::new("one", scale.now());
    assert!(matches!(node.machine, Machine::Starting { .. }));
    scale.sleep(100);  // Small time, just continue starting
    let (node, act) = node.time_passed(scale.now());
    assert!(matches!(node.machine, Machine::Starting { .. }));
    assert!(act.action == None);
    scale.sleep(10000);  // Large timeout, should already become a leader
    let (node, act) = node.time_passed(scale.now());
    assert!(matches!(node.machine, Machine::Leader { .. }));
    assert!(act.action == Some(Action::PingAll));
}

#[test]
fn test_start_vote() {
    let mut env = Environ::new();
    let mut node = Node::new("one", env.now());
    assert!(matches!(node.machine, Machine::Starting { .. }));

    env.add_another_for(&mut node);
    env.sleep(10000);  // Large timeout, should start_election
    let (node, act) = node.time_passed(env.now());
    assert!(matches!(node.machine, Machine::Electing { .. }));
    assert!(act.action == Some(Action::Vote));
}
