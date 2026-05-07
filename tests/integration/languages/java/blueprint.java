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

// Iter 19: new shapes — control flow, expressions, literals, patterns.

/** @interface (annotation type) with a constant. */
@interface Config {
    String value() default "default";
    int timeout() default 30;
}

/** Static initializer + try-with-resources + labeled loops + misc. */
class Extras {
    static final int SEED;
    static {
        SEED = 42;
    }

    /** Array initializer, char literal, class literal, method reference. */
    void literals() {
        int[] arr = {1, 2, 3};
        char ch = 'A';
        Class<?> cls = String.class;
        java.util.function.Function<Object, String> ref = String::valueOf;
    }

    /** Cast, instanceof, assert, break/continue with label. */
    void controlFlow(Object o) {
        int n = (int) 0L;
        assert n >= 0 : "non-negative";
        boolean isStr = o instanceof String;
        outer:
        for (int i = 0; i < 5; i++) {
            if (i == 1) continue outer;
            if (i == 3) break outer;
        }
        int k = 0;
        do { k++; } while (k < 3);
    }

    /** Try-with-resources + multi-catch + synchronized block. */
    void resources() throws Exception {
        try (java.io.InputStream s = null) {
            if (s == null) throw new java.io.IOException();
        } catch (java.io.IOException | RuntimeException e) {
            throw e;
        }
        synchronized (this) { SEED; }
    }

    /** Yield in block-switch + switch with underscore/record pattern. */
    int yieldDemo(Object o) {
        int v = switch (o) {
            case Integer i -> i;
            default -> { yield 0; }
        };
        return v;
    }
}
