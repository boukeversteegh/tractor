// Modifiers lifted as empty markers on the declaration. Every access
// modifier is exhaustive — private/package-private show up explicitly.

class Modifiers {
    public static final int PUB = 1;
    private int priv = 2;
    protected int prot = 3;
    int pkg = 4;                   // implicit package-private -> <package/>

    public synchronized void sync() {}
    public abstract static class AbsStatic {}
}
