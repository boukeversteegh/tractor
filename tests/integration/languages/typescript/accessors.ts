// Property accessor methods (`get foo()` / `set foo(v)`) carry a
// <get/> or <set/> marker on the <method>, so //method[get] picks
// them out uniformly regardless of body shape. Mirrors C#'s
// accessor-flattening fixture.

class Counter {
    private _value = 0;

    get value(): number {
        return this._value;
    }

    set value(v: number) {
        this._value = v;
    }

    static get singleton(): Counter {
        return new Counter();
    }
}
