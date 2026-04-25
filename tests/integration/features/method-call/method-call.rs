// Both function calls and method calls render as <call>. Method calls
// are distinguished by a <member> child that names the receiver and method.

fn use_calls() {
    let v: Vec<i32> = Vec::new();
    let n = v.len();
    let s = "hi".to_string();
    s.to_uppercase();
    format!("{}", 1);
}
