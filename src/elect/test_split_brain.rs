//! Tests for partitioned network (split-brain)
//!
//! Note that unlike in most other leader election systems, we still elect a
//! leader even in minority partition. Becase we don't care strong consistency.
