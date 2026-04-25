# Ruby's nested elsif / else chain collapses to flat <else_if> / <else>
# siblings of <if> (post-walk collapse_else_if_chain). `elsif` is
# renamed to <else_if> (underscore per Principle #1).

def classify(n)
  if n < 0
    "neg"
  elsif n == 0
    "zero"
  elsif n < 10
    "small"
  else
    "big"
  end
end

def label(n)
  n > 0 ? "positive" : "non-positive"
end
