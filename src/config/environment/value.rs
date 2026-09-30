//! Typed coercion of one raw environment-variable string into one config field.
//!
//! The scope is deliberately narrow: every function here takes `Option<String>`
//! straight from the environment lookup and assigns it to a single field, and an
//! absent or blank value always leaves the field at its TOML/default value.
//! Nothing that knows *which* variable maps to *which* field belongs here — that
//! is each `environment/<section>.rs`'s job. Do not grow this into a grab bag.

use std::{path::PathBuf, str::FromStr};

pub(super) fn nonempty(value: Option<String>) -> Option<String> {
    value.filter(|candidate| !candidate.trim().is_empty())
}

pub(super) fn set_string(target: &mut String, value: Option<String>) {
    if let Some(value) = nonempty(value) {
        *target = value;
    }
}

pub(super) fn set_path(target: &mut PathBuf, value: Option<String>) {
    if let Some(value) = nonempty(value) {
        *target = value.into();
    }
}

pub(super) fn set_optional_string(target: &mut Option<String>, value: Option<String>) {
    if let Some(value) = nonempty(value) {
        *target = Some(value);
    }
}

pub(super) fn set_json(target: &mut Option<serde_json::Value>, value: Option<String>) {
    if let Some(value) = nonempty(value).and_then(|value| serde_json::from_str(&value).ok()) {
        *target = Some(value);
    }
}

/// Silently ignores an unparsable value: a typo in one variable must not stop
/// the server, it falls back to the configured value like an absent variable.
pub(super) fn set_parsed<T: FromStr>(target: &mut T, value: Option<String>) {
    if let Some(value) = value.and_then(|value| value.parse::<T>().ok()) {
        *target = value;
    }
}
