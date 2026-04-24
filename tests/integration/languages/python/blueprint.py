"""Kitchen-sink Python blueprint for tractor snapshot tests.

This fixture is rendered by `tractor query <file> -p tree --single` with
NO depth limit, so any design-principle transform change produces a visible
snapshot diff. It is intentionally syntactically valid but not runnable.
"""

import os
import sys as system
from collections import OrderedDict
from typing import Optional, Union
from dataclasses import dataclass, field
from . import sibling
from .utils import helper as h

GLOBAL_FLAG: bool = True
PI: float = 3.14159
RAW_PATH, BYTES_BLOB = r"C:\Users\foo\bar", b"\x00\x01\x02"
TRIPLE = """line one\nline two"""


def simple(a, b=1, *args, kw_only=2, **kwargs):
    """Cover default, varargs, keyword-only, and **kwargs."""
    return a + b + kw_only


def pos_only(a, b, /, c, d, *, e, f=0):
    """Cover positional-only and keyword-only boundaries."""
    return (a, b, c, d, e, f)


def annotated(x: int, y: Optional[str] = None, z: list[int] | None = None) -> Union[int, None]:
    assert x >= 0, "x must be non-negative"
    result: int = x
    result += 1
    result //= 2
    matrix = [[1, 0], [0, 1]]
    matrix @= matrix
    return result if result else None


async def async_work(url: str) -> bytes:
    async with open_conn(url) as conn:
        async for chunk in conn:
            yield chunk
        data = await conn.read()
        return data


def gen_demo(n):
    for i in range(n): yield i
    yield from range(n, n * 2)


def repeat(times):
    def decorator(fn):
        def wrapper(*a, **kw):
            for _ in range(times):
                fn(*a, **kw)
        return wrapper
    return decorator


@staticmethod
@repeat(3)
def stacked():
    pass


@dataclass
class Point:
    """A dataclass with default factory."""
    x: int = 0
    y: int = 0
    tags: list[str] = field(default_factory=list)


class Base:
    pass


class Meta(type):
    pass


class Derived(Base, metaclass=Meta):
    """Class with inheritance and metaclass kwarg."""

    class Nested:
        CONST = 42

    def __init__(self, name: str):
        self.name = name

    def __eq__(self, other):
        return isinstance(other, Derived) and self.name == other.name


def control_flow(items):
    total = 0
    for i, v in enumerate(items):
        if v < 0:
            continue
        elif v == 0:
            break
        else:
            total += v
    else:
        total += 1

    n = 0
    while n < 10:
        n += 1
    else:
        pass

    try:
        risky()
    except (ValueError, TypeError) as e:
        raise RuntimeError("bad") from e
    except Exception:
        pass
    else:
        total += 1
    finally:
        cleanup()

    with open("a") as fa, open("b") as fb:
        fa.read()
        fb.read()

    return total


def match_demo(msg):
    match msg:
        case {"type": "ping", "id": pid} if pid > 0:
            return pid
        case [1, 2, *rest]:
            return rest
        case Point(x=0, y=y):
            return y
        case "yes" | "y" | "Y":
            return True
        case _:
            return None


def comprehensions(data):
    squares = [x * x for x in data if x > 0]
    pairs = {k: v for k, v in data.items() if v}
    uniq = {x for x in data}
    gen = (x for x in data for _ in range(2) if x)
    matrix = [(i, j) for i in range(3) for j in range(3) if i != j]
    return squares, pairs, uniq, gen, matrix


def expression_demo(items):
    first = items[0] if items else None
    if (n := len(items)) > 3:
        print(f"big: {n}")
    name = "world"
    greeting = f"hello {name!r}, value={n:>05d} nested={f'{name}'}"
    doubled = list(map(lambda v: v * 2, items))
    head, *mid, tail = items
    merged = {**{"a": 1}, **{"b": 2}}
    combined = [*items, *items]
    global GLOBAL_FLAG
    GLOBAL_FLAG = False
    return first, greeting, doubled, head, mid, tail, merged, combined


def closure_demo():
    counter = 0
    def inc():
        nonlocal counter
        counter += 1
        return counter
    del counter
    return inc
