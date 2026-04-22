# When a <name> wrapper sits inside method / class / module and contains
# a single <identifier> child, the transform inlines the identifier's
# text directly: <method><name>foo</name> … not
# <method><name><identifier>foo</identifier></name>…

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
