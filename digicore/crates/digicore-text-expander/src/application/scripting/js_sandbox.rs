//! JS sandbox (SE-21): Reject code containing eval(, Function(, new Function.
//!
//! Does not block "function name(" declarations (lowercase); blocks eval and Function constructor.

/// Returns Err if code contains forbidden sandbox patterns.
pub fn check_sandbox(code: &str) -> Result<(), String> {
    let lower = code.to_lowercase();
    if lower.contains("eval(") {
        return Err("[JS Error: sandbox rejected 'eval' - dynamic code execution not allowed]".to_string());
    }
    if code.contains("Function(") || code.contains("new Function") {
        return Err("[JS Error: sandbox rejected 'Function' - dynamic code execution not allowed]".to_string());
    }
    Ok(())
}
