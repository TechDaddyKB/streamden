pub fn sum(left: u32, right: u32) -> u32 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::sum;

    #[test]
    fn sums_values() {
        assert_eq!(sum(2, 3), 5);
    }
}
