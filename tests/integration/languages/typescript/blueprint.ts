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
const maybe: string | null = null;
const fallback = maybe ?? "default";
const nonNull = (maybe as string | null)!;
void arrow; void doubled; void fallback; void nonNull; void ns; void defaultExport; void aliasedB;
