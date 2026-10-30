//! jev-route: a deterministic routing-policy engine as a library + CLI.
//!
//! `policy` holds the scoring rules, `bench` runs the checked-in benchmark,
//! `json` is a std-only reader for the input files. The binary in `main.rs`
//! wires them to argv.

pub mod bench;
pub mod json;
pub mod policy;

#[cfg(test)]
mod unit_tests {
    use super::json::parse;
    use super::policy::*;

    #[test]
    fn json_round_shapes() {
        let v = parse(r#"{"a": [1, -2.5, "x\n", true, null], "b": {"c": 1e2}}"#).unwrap();
        assert_eq!(v.get("a").unwrap().as_arr().unwrap().len(), 5);
        assert_eq!(
            v.get("a").unwrap().as_arr().unwrap()[1].as_f64(),
            Some(-2.5)
        );
        assert_eq!(
            v.get("a").unwrap().as_arr().unwrap()[2].as_str(),
            Some("x\n")
        );
        assert_eq!(v.get("b").unwrap().get("c").unwrap().as_f64(), Some(100.0));
    }

    #[test]
    fn json_rejects_trailing_garbage() {
        assert!(parse("{} {}").is_err());
        assert!(parse("{\"a\"}").is_err());
    }

    #[test]
    fn unicode_strings_survive() {
        let v = parse(r#"{"k": "aurora 🦞 \u0041"}"#).unwrap();
        assert_eq!(v.get("k").unwrap().as_str(), Some("aurora 🦞 A"));
    }

    #[test]
    fn tier_parses_case_insensitively() {
        assert_eq!(Tier::parse("DEEP"), Some(Tier::Deep));
        assert_eq!(Tier::parse("nope"), None);
    }
}
