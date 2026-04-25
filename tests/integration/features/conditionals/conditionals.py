# Conditional shape: `elif` renders as <else_if> (underscore naming per
# Principle #1), collapsed as a flat sibling of <if>. Ternary expression
# keeps <then>/<else> wrappers via surgical field wrap.

def classify(n):
    if n < 0:
        return "neg"
    elif n == 0:
        return "zero"
    elif n < 10:
        return "small"
    else:
        return "big"


def label(n):
    return "positive" if n > 0 else "non-positive"
