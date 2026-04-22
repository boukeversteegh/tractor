// Every <param> carries an exhaustive marker: <required/> or <optional/>.
// Covers required, optional (? suffix), defaulted, and rest parameters.

function call(
    required: string,
    optional?: number,
    defaulted: boolean = true,
    ...rest: string[]
): void {}

// Naked JS-style identifier param (via .js grammar) should also normalize
// to <param><required/><name>…</name></param>.
function noTypes(x, y) {}
