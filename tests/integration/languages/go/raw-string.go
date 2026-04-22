// Go's raw string literals (backtick-quoted) render as <string> with a
// <raw/> marker (Principle #13: annotation follows node shape). A bare
// //string query catches both forms; //string[raw] is precise.

package main

var normal = "hello\nworld"
var raw = `hello
world`

var pattern = `^\d+$`
