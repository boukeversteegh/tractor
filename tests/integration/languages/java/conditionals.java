// Conditional shape: flat <if>/<else_if>/<else>. Ternary keeps
// <then>/<else> via surgical wrap.

class Conditionals {
    String classify(int n) {
        if (n < 0) {
            return "neg";
        } else if (n == 0) {
            return "zero";
        } else if (n < 10) {
            return "small";
        } else {
            return "big";
        }
    }

    String label(int n) {
        return n > 0 ? "positive" : "non-positive";
    }
}
