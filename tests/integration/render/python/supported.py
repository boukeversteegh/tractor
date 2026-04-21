# Canonical Python fixture for tractor's render round-trip tests.
#
# The tractor/tests/render_roundtrip.rs test parses this file, renders the
# resulting semantic tree back to source, and fails if the output differs
# from this file byte-for-byte. Anything that stays in here is, by
# definition, supported by the Python renderer — expand this file to grow
# the renderer's feature surface.
#
# Scope: data-structure constructs only (classes with typed attributes,
# Enum / IntEnum, TypedDict-style containers, type aliases). Imperative
# code is out of scope and lives in a later batch.

import typing
import os.path
from dataclasses import dataclass
from typing import Optional, List


@dataclass
class User:
    name: str
    age: int = 0
    nickname: str = ""
    score: float = 0.0
    active: bool = True
    email: Optional[str] = None
    tags: list[str]
    counters: dict[str, int]


class Address:
    street: str
    city: str
    zip: str
