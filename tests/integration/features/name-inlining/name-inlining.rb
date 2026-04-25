# Two invariants:
#
# 1. Every Ruby `identifier` becomes <name> unconditionally (no
#    type_identifier in Ruby's grammar, so identifiers are always
#    value references). Matches Python / TS / Java / Rust on the
#    value-namespace side (Principle #14).
#
# 2. When a <name> wrapper sits inside method / class / module and
#    contains a single identifier, the transform inlines its text
#    directly: <method><name>foo</name>… not
#    <method><name><identifier>foo</identifier></name>…

class Calculator
  def add(a, b)
    a + b
  end
end

module Utils
  def self.greet(name)
    "hi, #{name}"
  end
end
