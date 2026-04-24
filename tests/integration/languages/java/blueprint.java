// Blueprint fixture — exercises every major Java construct so any
// transform change shows up as a visible snapshot diff. Adjust
// freely when real language features need to be represented; every
// cycle of improvements should regenerate the snapshot.

package com.example.blueprint;

import java.util.List;
import java.util.Map;
import java.util.function.Function;
import static java.util.Collections.emptyList;

/** Javadoc comment on the class. */
@SuppressWarnings("unchecked")
public final class Demo<T extends Comparable<T>> extends Base implements Handler<T> {

    // Line comment before a field
    /* block comment */
    public static final int MAX = 100;
    private String name;
    protected List<Map<String, T>> items;
    int pkg; // trailing line comment
    transient volatile boolean ready;

    public Demo() {
        this(null);
    }

    public Demo(String name) {
        super();
        this.name = name;
    }

    public static <R> R factory(Function<String, R> mapper) {
        return mapper.apply("hello");
    }

    @Override
    public synchronized T process(T input) throws IllegalArgumentException {
        if (input == null) {
            throw new IllegalArgumentException("null");
        } else if (!ready) {
            return null;
        } else {
            return input;
        }
    }

    public String describe(int n) {
        return n > 0 ? "positive" : n < 0 ? "negative" : "zero";
    }

    public int sum(int... xs) {
        int total = 0;
        for (int x : xs) {
            total += x;
        }
        int i = 0;
        while (i < 3) {
            i = i + 1;
        }
        return total;
    }

    public String match(Object o) {
        return switch (o) {
            case Integer i -> "int " + i;
            case String s when !s.isEmpty() -> "non-empty: " + s;
            default -> "other";
        };
    }

    private interface Handler<U> {
        U process(U input) throws IllegalArgumentException;
    }

    public enum Color {
        RED, GREEN, BLUE;

        public int rgb() {
            return switch (this) {
                case RED -> 0xFF0000;
                case GREEN -> 0x00FF00;
                case BLUE -> 0x0000FF;
            };
        }
    }

    public record Point(int x, int y) {
        public Point {
            if (x < 0 || y < 0) throw new IllegalArgumentException();
        }
    }
}

class Base {
    String greet() {
        return String.format("hello from %s", getClass().getSimpleName());
    }
}
