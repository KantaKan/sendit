#![no_main]
use libfuzzer_sys::fuzz_target;
use std::path::{Component, Path};

fuzz_target!(|s: &str| {
    let p = sendit_core::sanitize::sanitize_relative(s);
    assert!(
        p.components().all(|c| matches!(c, Component::Normal(_))),
        "{p:?}"
    );
    assert!(Path::new("/dest").join(&p).starts_with("/dest"));
});
