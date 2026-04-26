//! Cross-language accessor shape.
//!
//! C# property accessors are named directly by their accessor keyword
//! (`<get>`, `<set>`, `<init>`). TypeScript `get value()` / `set
//! value(v)` are still methods, with accessor-kind markers on the
//! `<method>` node.

use crate::support::semantic::*;

#[test]
fn csharp_property_accessors_are_specific_nodes() {
    let mut tree = parse_src("csharp", r#"
        class Accessors
        {
            public int AutoProp { get; set; }

            private int _backing;
            public int Manual
            {
                get { return _backing; }
                set { _backing = value; }
            }

            public int ReadOnly { get; }
            public int WriteOnly { set { _backing = value; } }
        }
    "#);

    claim("AutoProp shape is two direct accessor keyword nodes",
        &mut tree,
        &multi_xpath(r#"
            //property[name='AutoProp']
                [get[not(body)]]
                [set[not(body)]]
                [count(get)=1]
                [count(set)=1]
                [not(init)]
        "#),
        1);

    claim("Manual shape is two direct bodied accessor keyword nodes",
        &mut tree,
        &multi_xpath(r#"
            //property[name='Manual']
                [get/body/block]
                [set/body/block]
                [count(get)=1]
                [count(set)=1]
                [not(init)]
        "#),
        1);

    claim("ReadOnly shape is a single get accessor",
        &mut tree,
        &multi_xpath(r#"
            //property[name='ReadOnly']
                [get]
                [count(get)=1]
                [not(set|init)]
        "#),
        1);

    claim("WriteOnly shape is a single bodied set accessor",
        &mut tree,
        &multi_xpath(r#"
            //property[name='WriteOnly']
                [set/body/block]
                [count(set)=1]
                [not(get|init)]
        "#),
        1);
}

#[test]
fn typescript_accessor_methods_carry_kind_markers() {
    let mut tree = parse_src("typescript", r#"
        class Counter {
            private _value = 0;

            get value(): number { return this._value; }
            set value(v: number) { this._value = v; }
            static get singleton(): Counter { return new Counter(); }
        }
    "#);

    claim("value getter shape is a public method with a get marker and body",
        &mut tree,
        &multi_xpath(r#"
            //method[name='value']
                [get]
                [public]
                [body/block]
                [not(set)]
        "#),
        1);

    claim("value setter shape is a public method with a set marker, parameter, and body",
        &mut tree,
        &multi_xpath(r#"
            //method[name='value']
                [set]
                [public]
                [parameter[name='v']]
                [body/block]
                [not(get)]
        "#),
        1);

    claim("singleton getter shape is a public method with a get marker and body",
        &mut tree,
        &multi_xpath(r#"
            //method[name='singleton']
                [get]
                [public]
                [body/block]
                [not(set)]
        "#),
        1);

    claim("Counter has exactly three accessor methods",
        &mut tree, "//method[get or set]", 3);
}
