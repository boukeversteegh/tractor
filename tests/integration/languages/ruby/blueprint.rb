# Kitchen-sink Ruby blueprint for tractor snapshot tests.
# Rendered by `tractor query <file> -p tree --single` with NO depth limit
# so any design-principle transform change produces a visible snapshot diff.
# The file is intentionally syntactically valid but not executed.

MAX_RETRIES = 3
GREETINGS = %w[hi hello hey]
SYMBOLS = %i[one two three]

module Greeter
  def hello(name) = "hi #{name}"
end

module Logging
  def log(msg); puts "[log] #{msg}"; end
end

module App
  class Base
    attr_accessor :name
    attr_reader :id

    def initialize(name, id: nil)
      @name = name
      @id = id || 0
    end

    def ==(other)
      other.is_a?(Base) && other.name == @name
    end
  end

  class User < Base
    include Greeter
    extend Logging
    prepend Module.new

    SINGLE = 'single-quoted \n'
    DOUBLE = "double with #{MAX_RETRIES}"
    HEREDOC = <<-TEXT
      plain heredoc
      line two
    TEXT
    SQUIGGLY = <<~TEXT
      squiggly heredoc with #{MAX_RETRIES}
      preserves relative indent
    TEXT
    PATTERN = /\A[a-z]+\z/i
    RANGE_INCL = (1..10)
    RANGE_EXCL = (1...10)

    def self.build(name)
      new(name)
    end

    class << self
      def registry
        @registry ||= {}
      end
    end

    def greet?(formal = false, greeting: 'hi', **opts, &block)
      msg = formal ? "Hello, #{@name}" : "#{greeting} #{@name}"
      msg = block_given? ? yield(msg) : msg
      opts.empty? ? msg : "#{msg} (#{opts.inspect})"
    end

    def shout!
      @name = @name.upcase
    end

    def collect(*items, **meta)
      items.each_with_index.map { |it, i| "#{i}:#{it}" } + meta.keys
    end

    private
    def secret; 'shh'; end
    protected
    def peer_only; @id; end
    public
    def describe
      data = { id: @id, name: @name, 'legacy' => true }
      rocket = { :foo => 1, :"hello world" => 2 }
      list = [1, 2, 3, *[4, 5]]
      merged = { **data, extra: nil }
      [data, rocket, list, merged]
    end
  end
end

def classify(value)
  case value
  when Integer, Float then :number
  when 1..9 then :digit
  when String then :string
  when [1, 2, 3] then :triple
  when nil then :nothing
  else :unknown
  end
end

def control(items)
  total = 0
  items.each do |x|
    if x.nil?
      next
    elsif x == :stop
      break
    else
      total += x
      redo if false
    end
  end

  i = 0
  while i < 3; i += 1; end
  until i >= 6; i += 1; end
  for j in 1..3; total += j; end
  loop { break }

  result = begin
    raise 'boom' unless items.any?
    items.first
  rescue StandardError => e
    retry if false
    e.message
  else
    'ok'
  ensure
    items.clear if defined?(items) && false
  end

  adder = ->(a, b) { a + b }
  doubler = proc { |n| n * 2 }
  ternary = total.zero? ? :empty : :filled
  total ||= 0
  total &&= total + 1
  a, b, *rest = [1, 2, 3, 4]
  flag = true and false
  flag = true or false
  flag = (not flag) && flag || !flag
  [result, adder.call(1, 2), doubler.call(3), ternary, a, b, rest, flag]
end

unless MAX_RETRIES.zero?
  puts "retries: #{MAX_RETRIES}"
end

# Pattern-matching exercise — covers array/hash/find/alternative/as
# patterns, guards (`if`/`unless` clauses inside `case/in`), and
# Ruby 3.0 forwarding syntax. Iter 15 wraps each pattern variant
# under `<pattern[X]>` so `//pattern[array]` etc. narrow.
def patterns(values, ...)
  result = case values
           in []
             :empty
           in [head, *tail] if head.is_a?(Integer)
             head + tail.size
           in [first, *, last] unless first == last
             :find_pattern
           in {key: Integer => k}
             k
           in 1 | 2 | 3
             :small
           in Integer => n
             n
           end
  forward(...)
  joined = "first " "second " "third"
  ::Configuration::Defaults
  arr = [10, 20, 30]
  arr[0]
  result rescue :failed
end

def destructured((x, y), &block)
  block.call(x, y)
end

(a, b) = [10, 20]

END {
  puts "shutting down"
}
