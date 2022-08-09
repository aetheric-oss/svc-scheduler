//! Example of a rust library.
//!
//! Learn more about how to create a library [here](https://doc.rust-lang.org/book/rust-libs.html).

/// Prints "Hello, {name}!"
///
/// # Arguments
///
/// * `name` - A string slice that holds the name of a person or entity
///
pub fn hello(name: &str) {
    print!("Hello, {name}!");
}

/// Adds one to a number
///
/// # Arguments
///
/// * `val` - Any U8 number
///
pub fn add_one(val: u8) -> u8 {
    val + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ut_add_one() {
        let val: u8 = 1;
        assert_eq!(val + 1, add_one(val));
    }
}
