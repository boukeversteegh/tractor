// blueprint.ts — TypeScript kitchen-sink fixture.
// Rendered by tractor's snapshot system so design-principle changes
// to the TypeScript transform show up as visible snapshot diffs.
// Covers most major TS constructs: classes, interfaces, generics,
// unions, conditional/mapped types, async/generators, decorators of syntax.

import defaultExport, { namedA, namedB as aliasedB } from "./barrel";
import * as ns from "./namespace";
import type { OnlyAtCompileTime } from "./types";

export { namedA } from "./barrel";
export type Re<T> = T extends Array<infer U> ? U : never;

/** A shape with an optional label and readonly id. */
export interface Shape<T = number> {
    readonly id: string;
    label?: string;
    value: T;
    describe(prefix: string): string;
}

export type Point = { x: number; y: number };
export type Pair = [string, number];
export type Status = "idle" | "loading" | "ready" | "error";
export type Mapper<T> = (input: T) => T;
export type Partialized<T> = { [K in keyof T]?: T[K] };
export type Greeting = `hello, ${string}`;

export enum Color {
    Red = "RED",
    Green = "GREEN",
    Blue = "BLUE",
}

/** Abstract base using visibility + readonly + static. */
export abstract class Base<T extends Shape> {
    public static kind: string = "base";
    protected readonly createdAt: number = Date.now();
    private _cache: Map<string, T> = new Map();

    constructor(public name: string, private readonly seed?: number) {}

    abstract render(): string;

    get cacheSize(): number { return this._cache.size; }
    set primary(value: T) { this._cache.set("primary", value); }

    public store(key: string, value: T): void {
        this._cache.set(key, value);
    }
}

export class Widget<T extends Shape = Shape<number>> extends Base<T> {
    static #counter = 0;

    constructor(name: string, private items: T[] = []) {
        super(name);
        Widget.#counter++;
    }

    override render(): string {
        const first = this.items[0]?.label ?? "unnamed";
        return `Widget<${first!}>`;
    }

    *entries(): Generator<[number, T]> {
        for (let i = 0; i < this.items.length; i++) yield [i, this.items[i]];
    }

    async loadAll(urls: string[], ...extra: string[]): Promise<T[]> {
        try {
            const merged = [...urls, ...extra];
            for (const u of merged) { await fetch(u); }
            return this.items;
        } catch (e) {
            throw e as Error;
        } finally {
            this.items.length;
        }
    }
}

function isShape(v: unknown): v is Shape {
    return typeof v === "object" && v !== null && "id" in v;
}

const tag = (parts: TemplateStringsArray, ...vals: unknown[]) =>
    parts.reduce<string>((acc, p, i) => acc + p + (vals[i] ?? ""), "");

async function compute<T extends number | string>(
    input: T,
    { scale = 1, label }: { scale?: number; label?: string } = {},
): Promise<Greeting> {
    const [head, ...rest] = String(input).split(""); // array destructure + rest
    const spread = { ...{ scale }, label, head, rest };
    const msg = tag`hello, ${label ?? "world"} (${spread.scale})` as Greeting;
    return msg satisfies Greeting;
}

/* A block comment describing control flow demos. */
export function controlFlow(n: number): Status {
    let status: Status = "idle";
    if (n < 0) status = "error";
    else if (n === 0) status = "idle";
    else if (n < 10) status = "loading";
    else status = "ready";

    switch (status) {
        case "ready": break;
        case "error": throw new Error("bad");
        default: /* fallthrough */ break;
    }

    const obj: Record<string, number> = { a: 1, b: 2 };
    for (const k in obj) { obj[k]! += 1; }
    for (const [, v] of Object.entries(obj)) { void v; }
    while (false) { /* never */ }
    return n > 0 ? status : "idle";
}

const arrow = <U,>(xs: U[]): U | undefined => xs[0];
const doubled: number[] = [1, 2, 3].map((x): number => x * 2);
const arrowBlock = (x: number): number => { return x * 2; };
const maybe: string | null = null;
const fallback = maybe ?? "default";
const nonNull = (maybe as string | null)!;
void arrow; void doubled; void fallback; void nonNull; void ns; void defaultExport; void aliasedB;

// Iter 18: TS 5+ / advanced type / declare / module shapes.

// Ambient `declare` declarations.
declare const __DEV__: boolean;
declare function externalThing(name: string): void;
declare namespace SideEffect {
    function init(): void;
}

// Constructor / typeof / this / rest / optional-tuple / template-type shapes.
type Builder = new (input: string) => Status;
type Identity<T> = typeof globalThis extends T ? T : never;
type Reflective = { copy(): this };
// Rest in tuple — anonymous form keeps the rest_type at the outer level
// so `//type[rest]` matches without a `<name>` field wrapper.
type RestArgs = [number, ...string[]];
// `T?` only appears inside tuple element annotations.
type OptionalSlot = [head: number, tail?: string];

// Old-style cast (`<T>expr`) — type assertion.
const asserted = <number>(<unknown>"42");

// Class with static initializer block.
class StaticBlock {
    static counter = 0;
    static {
        StaticBlock.counter = 7;
    }
}

// Asserts / type predicate annotations.
function isNumber(x: unknown): asserts x is number {
    if (typeof x !== "number") throw new Error("not a number");
}

// `Foo<T>` instantiation expression.
const arrayOf = Array<number>;
const fixed = arrayOf();

// Old-style `import x = require(y)` form.
import legacy = require("./legacy");

// `import.meta` / `new.target` meta-properties.
const url = import.meta.url;
function MetaCtor(this: unknown) { return new.target?.name ?? ""; }

// Nested type / value identifiers.
type NestedTy = StaticBlock["counter"];
const nestedVal = StaticBlock.counter;

// Regex literal + flags.
const pattern = /^foo(\d+)bar$/giu;

// Labeled / sequence / for-classic / debugger / do-while / with (deprecated).
function controlOdds(value: number): number {
    let total = 0;
    outer: for (let i = 0, j = 100; i < 5; i++, j--) {
        if (i === 2) continue outer;
        if (i === 4) break outer;
        total = (total += i, total * 2);
    }
    let n = 0;
    do {
        n++;
    } while (n < 3);
    if (false) debugger;
    return total + n;
}

// Pair pattern + assignment pattern in destructure with defaults.
function destructure({ a: aa = 1, b: bb }: { a?: number; b: number }, [c = 0, ...rest]: number[]): number {
    return aa + bb + c + rest.length;
}

void controlOdds; void destructure; void asserted; void fixed; void url; void nestedVal; void pattern;
void MetaCtor; void legacy;
