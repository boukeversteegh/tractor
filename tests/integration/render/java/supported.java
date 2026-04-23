// Canonical Java fixture for tractor's render round-trip tests.
//
// The tractor/tests/render_roundtrip.rs test parses this file, renders the
// resulting semantic tree back to source, and fails if the output differs
// from this file byte-for-byte. Anything that stays in here is, by
// definition, supported by the Java renderer — expand this file to grow
// the renderer's feature surface.
//
// Scope: data-structure constructs only (package and import declarations,
// classes with typed fields, interfaces with method signatures, records,
// enums with constants). Imperative code (method bodies, statements) is
// out of scope and lives in a later batch.

package com.example;

import java.util.List;
import java.util.Map;

public class User {
    public static final String VERSION = "1.0";

    private String name;

    private int age = 0;

    public List<String> tags;

    public Map<String, Integer> counters;

    public int[] scores;
}

public interface Entity {
    String getId();
}

public record Point(int x, int y) {}

public enum Status {
    ACTIVE,
    INACTIVE,
    SUSPENDED
}
