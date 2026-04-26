//! Cross-language: modifier ordering and source-order markers.
//!
//! Modifiers lift as empty markers on the declaration. Every
//! access modifier is exhaustive — package-private gets an
//! explicit <package/> marker. Markers appear in source order
//! (source-reversibility), and the source keywords also survive
//! as text so the enclosing node's string-value still reads like
//! the source.

use crate::support::semantic::*;

#[test]
fn csharp() {
    claim("C# extension-method this parameter keeps this as an empty modifier marker",
        &mut parse_src("csharp", r#"
            public static class Mapper {
                public static UserDto Map(this User user) { return new UserDto(); }
            }
        "#),
        &multi_xpath(r#"
            //method[name='Map']
                [public]
                [static]
                [parameter
                    [this]
                    [contains(., 'this')]
                    [type/name='User']
                    [name='user']]
        "#),
        1);
}

#[test]
fn java() {
    let mut tree = parse_src("java", r#"
        public abstract static class Modifiers {
            public static final int PUB = 1;
            private int priv = 2;
            protected int prot = 3;
            int pkg = 4;
            public synchronized void sync() {}
            public abstract static class AbsStatic {}
        }
    "#);

    claim("Modifiers body composes field, method, and nested-class modifier shapes",
        &mut tree,
        &multi_xpath(r#"
            //class[name='Modifiers']/body
                [field[declarator/name='PUB']
                    [public]
                    [static]
                    [final]
                ]
                [field[declarator/name='priv']
                    [private]]
                [field[declarator/name='prot']
                    [protected]]
                [field[declarator/name='pkg']
                    [package]]
                [method[name='sync']
                    [public]
                    [synchronized]
                ]
                [class[name='AbsStatic']
                    [public]
                    [abstract]
                    [static]
                ]
        "#),
        1);

    claim("first marker on outer class is <public/> (source order)",
        &mut tree, "//class[name='Modifiers']/*[1][self::public]", 1);

    claim("second marker on outer class is <abstract/> (source order)",
        &mut tree, "//class[name='Modifiers']/*[2][self::abstract]", 1);

    claim("third marker on outer class is <static/> (source order)",
        &mut tree, "//class[name='Modifiers']/*[3][self::static]", 1);

    claim("source keywords preserved as dangling text (source-reversibility)",
        &mut tree,
        &multi_xpath(r#"
            //class[name='Modifiers']
                [contains(., 'public abstract static')]
        "#),
        1);
}
