pub(crate) fn trimmed(value: f64) -> String {
    let text = format!("{:.2}", value);

    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trailing_zeros_and_a_bare_point_go() {
        assert_eq!(trimmed(13.333_333), "13.33");
        assert_eq!(trimmed(7.8), "7.8");
        assert_eq!(trimmed(20.0), "20");
        assert_eq!(trimmed(100.0), "100");
        assert_eq!(trimmed(0.0), "0");
    }
}
