// Canonical C# fixture for tractor's render round-trip tests.
//
// The tractor/tests/render_roundtrip.rs test parses this file, renders the
// resulting semantic tree back to source, and fails if the output differs
// from this file byte-for-byte. Anything that stays in here is, by
// definition, supported by the C# renderer — expand this file to grow the
// renderer's feature surface.
//
// Scope: data-structure constructs only (types, fields, properties, enums,
// records) plus namespaces/imports/attributes. Imperative code (method
// bodies, statements) is out of scope and lives in a later batch.
//
// Canonical form: Allman braces, four-space indent, blank line between
// members, and always-explicit access modifiers (so queries like
// `//method[public]` never have to branch against an implicit default).

using System;
using System.Collections.Generic;

namespace SampleApp
{
    public class User : IEntity
    {
        public Guid? Id { get; set; }

        public string Name { get; set; }

        public string DisplayName { get; set; } = "default";

        public List<string> Tags { get; set; }

        public Dictionary<string, int> Counters { get; set; }

        public int[] Scores { get; set; }

        public Dictionary<string, List<int>> Buckets { get; set; }

        [Required]
        [MaxLength(100)]
        public string Email { get; set; }

        public const int MaxLength = 100;

        public int InitialCount = 0;

        private readonly int _cache;
    }

    public static class Helpers
    {
        public const string Version = "1.0";

        public static int Count { get; set; }
    }

    public struct Point
    {
        public int X;

        public int Y;
    }

    public record Address(string Street, string City);

    public record Person(string Name)
    {
        public int Age { get; set; }
    }

    public interface IEntity
    {
        public Guid? Id { get; set; }
    }

    public enum Status : byte
    {
        Active = 0,
        Inactive = 1,
        Suspended = 2
    }

    public class Outer
    {
        public class Inner
        {
            public int X { get; set; }
        }
    }
}
