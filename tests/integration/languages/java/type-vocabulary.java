// Principle #14: type references wrap their name in a <name> child.
// Covers: plain types, generic types, array types, extends/implements,
// type parameter declarations with <extends> bounds.

import java.util.List;

class Animal {}
interface Barker { void bark(); }
interface Runner { void run(); }

class Dog<T extends Animal> extends Animal implements Barker, Runner {
    T owner;
    List<String> tags;

    public void bark() {}
    public void run() {}
}
