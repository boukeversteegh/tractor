// Conditional shape: `else if` chains collapse to flat <else_if> siblings of
// <if>; ternary expression keeps <then>/<else> wrappers via surgical field
// wrap (not via the global alternative->else wrap that was dropped).

function classify(n: number): string {
    if (n < 0) {
        return "neg";
    } else if (n === 0) {
        return "zero";
    } else if (n < 10) {
        return "small";
    } else {
        return "big";
    }
}

const label = (n: number) => n > 0 ? "positive" : "non-positive";
