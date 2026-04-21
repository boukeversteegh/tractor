// Canonical C# fixture for tractor's render round-trip tests.
//
// The tractor/tests/render_roundtrip.rs test parses this file, renders the
// resulting semantic tree back to source, and fails if the output differs
// from this file byte-for-byte. Anything that stays in here is, by
// definition, supported by the C# renderer — expand this file to grow the
// renderer's feature surface.
//
// Formatting is whatever the renderer emits (Allman braces, four-space
// indent, blank line between class members, single blank line between
// imports and the first declaration).

using System;

namespace SampleApp
{
    public class User : IEntity
    {
        public Guid? Id { get; set; }

        public string Name { get; set; }

        public List<string> Tags { get; set; }

        public Dictionary<string, int> Counters { get; set; }

        public int[] Scores { get; set; }

        [Required]
        public string Email { get; set; }

        private readonly int _count;
    }

    public struct Point
    {
        public int X;

        public int Y;
    }

    public interface IEntity
    {
        Guid? Id { get; set; }

        void Save(int id);
    }

    public enum Status
    {
        Active,
        Inactive = 2,
        Suspended
    }
}
