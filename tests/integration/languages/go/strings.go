// Go strings: interpreted (double-quoted, escapes) and raw
// (backtick, no escapes). Both render as <string>; raw strings
// carry a <raw/> marker (Principle #13). A bare //string query
// catches both forms; //string[raw] is precise.

package main

const normal = "hello\nworld"
const raw = `hello world`
const pattern = `^\d+$`
