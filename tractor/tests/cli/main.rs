//! Black-box integration tests for the `tractor` CLI binary.
//!
//! Each test runs `tractor` as an external subprocess, avoiding bash entirely
//! for cross-platform reliability. Tests are organized to mirror the bash
//! test suites they replace.

mod common;

mod formats;
mod languages;
mod run_cmd;
mod set;
mod string_input;
mod update;
mod view_modifiers;
mod xpath_expressions;
