// Principle #12: parameters / arguments / generics / accessors render as
// flat siblings with field attributes. No <parameters>/<accessor_list>
// wrapper nesting.

class FlatLists
{
    public T First<T, U>(T a, U b, int c) where T : class
    {
        return a;
    }

    public int Count { get; set; }

    public void Caller()
    {
        First<string, int>("x", 1, 2);
    }
}
