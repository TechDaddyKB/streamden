//! Saturating arithmetic probe for the DEN-11 CI fixture.

/// Saturating addition.
///
/// ```
/// assert_eq!(b4_probe::add(2, 3), 5);
/// assert_eq!(b4_probe::add(i64::MAX, 1), i64::MAX);
/// ```
pub fn add(left: i64, right: i64) -> i64 {
    left.saturating_add(right)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_small_values() {
        assert_eq!(add(20, 22), 42);
    }

    #[test]
    fn saturates_at_the_top() {
        assert_eq!(add(i64::MAX, 5), i64::MAX);
    }
}
