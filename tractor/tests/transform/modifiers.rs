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

    claim("public static final field marks all 3 modifiers",
        &mut tree, "//field[public and static and final][declarator/name='PUB']", 1);

    claim("private field carries <private/>",
        &mut tree, "//field[private]", 1);

    claim("protected field carries <protected/>",
        &mut tree, "//field[protected]", 1);

    claim("implicit package-private surfaces as <package/>",
        &mut tree, "//field[package]", 1);

    claim("synchronized method also marks public",
        &mut tree, "//method[public and synchronized][name='sync']", 1);

    claim("nested class composes public + abstract + static markers",
        &mut tree, "//class[public and abstract and static][name='AbsStatic']", 1);

    claim("first marker on outer class is <public/> (source order)",
        &mut tree, "//class[name='Modifiers']/*[1][self::public]", 1);

    claim("second marker on outer class is <abstract/> (source order)",
        &mut tree, "//class[name='Modifiers']/*[2][self::abstract]", 1);

    claim("third marker on outer class is <static/> (source order)",
        &mut tree, "//class[name='Modifiers']/*[3][self::static]", 1);

    claim("source keywords preserved as dangling text (source-reversibility)",
        &mut tree, "//class[name='Modifiers'][contains(., 'public abstract static')]", 1);
}
