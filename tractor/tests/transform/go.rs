//! Go-specific transformation tests.

#[path = "go/spec_flattening.rs"]        pub mod spec_flattening;
#[path = "go/struct_interface_hoist.rs"] pub mod struct_interface_hoist;
#[path = "go/switch_markers.rs"]         pub mod switch_markers;
#[path = "go/type_declaration.rs"]       pub mod type_declaration;
