//! Shared fixtures for tower's crates.
//!
//! Its own crate rather than fufu's `ff-testsupport`, which is
//! `publish = false` and stopped being reachable the moment tower left that
//! workspace. Nothing here yet; what it owes tower's tests is a repository
//! with fufu armed on it, a real `ff` on PATH to spawn, and a tower log
//! with flights already in it.
