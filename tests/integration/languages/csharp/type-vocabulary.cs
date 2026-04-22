// Principle #14: type references wrap their name in a <name> child.
// Covers: plain types, generic types, base list (extends + implements),
// type parameter declarations.

using System.Collections.Generic;

interface IBarker { void Bark(); }
class Animal {}

class Dog<T> : Animal, IBarker where T : Animal
{
    public T Owner;
    public List<string> Tags;
    public void Bark() {}
}
