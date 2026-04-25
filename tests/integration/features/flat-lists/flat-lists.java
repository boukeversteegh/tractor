// Principle #12: flat parameters/arguments/generics. No
// <parameters> wrapper — each parameter is a direct sibling with
// field="parameters".

class FlatLists {
    <T, U extends Comparable<U>> T first(T a, U b, int c) {
        return a;
    }

    void caller() {
        first("x", "y", 1);
    }
}
