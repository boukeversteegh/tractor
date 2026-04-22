// `ctor` → `<constructor>` (Principle #2: full names over abbreviations).

class Point {
    int x, y;

    Point() { this(0, 0); }

    Point(int x, int y) {
        this.x = x;
        this.y = y;
    }
}
