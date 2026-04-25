// async / generator lifted to empty markers on <function> and <method>.
// Exhaustive (Principle #9 ish): every function element carries either
// neither or the applicable markers — <async/> and/or <generator/>.

async function fetchOne(): Promise<number> { return 1; }

function* counter(): Generator<number> { yield 1; }

async function* stream(): AsyncGenerator<number> { yield 1; }

class Service {
    async load(): Promise<void> {}
    *keys(): Generator<string> { yield "a"; }
}
