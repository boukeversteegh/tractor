// Canonical TypeScript fixture for tractor's render round-trip tests.
//
// The tractor/tests/render_roundtrip.rs test parses this file, renders the
// resulting semantic tree back to source, and fails if the output differs
// from this file byte-for-byte. Anything that stays in here is, by
// definition, supported by the TypeScript renderer — expand this file to
// grow the renderer's feature surface.
//
// Scope: data-structure constructs only (interfaces, type aliases, enums,
// classes with typed fields). Imperative code is out of scope and lives
// in a later batch.

interface User {
    name: string;
    age: number;
    nickname?: string;
    tags: string[];
}

type Id = string;

enum Status {
    Active,
    Inactive
}
