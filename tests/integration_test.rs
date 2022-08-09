//! Example for writing an integration test.
//! More information: https://doc.rust-lang.org/book/testing-rust.html#integration-tests

use tmp_lib;

#[test]
fn it_add_one() {
    assert_eq!(2, tmp_lib::add_one(1));
}
