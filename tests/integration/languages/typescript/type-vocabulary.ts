// Principle #14: type references wrap their name in a <name> child.
// Every <type> element should contain <name>X</name>, not bare text.
// Covers: plain types, generic types, base-class/implements, function types.

type Id = number;
type Handler = (x: number) => void;
type Box<T> = Array<T>;

class Animal {}
interface Barker { bark(): void; }
class Dog extends Animal implements Barker {
    bark(): void {}
}
