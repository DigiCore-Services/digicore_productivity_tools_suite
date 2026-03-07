//! Custom DSL evaluator (SE-25): {dsl:expr} for simple math/formatting.
//!
//! SRP: Single module for expression evaluation. Hexagonal: port-style interface.
//! Supports: numbers, +, -, *, /, round(), min(), max(), etc. via meval.


/// Evaluate DSL expression. Returns string result or error message.
pub fn evaluate(expr: &str) -> String {
    let expr = expr.trim();
    if expr.is_empty() {
        return String::new();
    }
    match evalexpr::eval(expr) {
        Ok(v) => match v.as_float() {
            Ok(f) => format_number(f),
            Err(_) => match v.as_int() {
                Ok(i) => format_number(i as f64),
                Err(_) => format!("[DSL Error: Result is not a number]"),
            },
        },
        Err(e) => format!("[DSL Error: {}]", e),
    }
}

fn format_number(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{}", v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dsl_simple() {
        assert_eq!(evaluate("1 + 2"), "3");
        assert_eq!(evaluate("10 * 20"), "200");
    }

    #[test]
    fn test_dsl_float() {
        assert_eq!(evaluate("3.14 * 2"), "6.28");
    }

    #[test]
    fn test_dsl_round() {
        assert_eq!(evaluate("round(3.7)"), "4");
    }
}
