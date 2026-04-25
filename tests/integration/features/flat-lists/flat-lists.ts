// Principle #12: parameters / arguments / generics render as flat siblings,
// not nested <parameters><parameter/></parameters> wrappers. Each sibling
// carries a field="..." attribute for grouping in queries.

function first<T, U>(a: T, b: U, c: number): T {
    return a;
}

first<string, number>("x", 1, 2);
