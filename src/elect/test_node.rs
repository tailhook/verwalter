//! Tests for the case of single node
//!
use time::SteadyTime;

use super::{Node, Machine};
use super::action::Action;
use super::test_util::TimeScale;


#[test]
fn test_starting() {
    let mut scale = TimeScale::new();
    let node = Node::new("one", scale.now());
    assert!(matches!(node.machine, Machine::Starting { .. }));
}

#[test]
fn test_alone() {
    let mut scale = TimeScale::new();
    let node = Node::new("one", scale.now());
    assert!(matches!(node.machine, Machine::Starting { .. }));
    scale.advance_ms(100);  // Small time, just continue starting
    let (node, act) = node.time_passed(scale.now());
    assert!(matches!(node.machine, Machine::Starting { .. }));
    assert!(act.action == None);
    scale.advance_ms(10000);  // Large timeout, should already become a leader
    let (node, act) = node.time_passed(scale.now());
    assert!(matches!(node.machine, Machine::Leader { .. }));
    assert!(act.action == Some(Action::LeaderPing));
}
