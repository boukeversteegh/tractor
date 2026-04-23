# Principle #12: `expression_list` (tuple-like return/yield expressions)
# is a pure grouping node; drop it so the expressions become direct
# children of the enclosing <return>/<yield>/<assign>. Parallel with Go.

def pair():
    return 1, 2

def triple():
    return "a", "b", "c"

def unpack():
    a, b = pair()
    return a + b
