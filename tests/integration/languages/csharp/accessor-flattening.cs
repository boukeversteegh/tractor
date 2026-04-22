// Property accessor lists flatten: get/set are direct siblings of the
// <property>, not nested inside an <accessor_list> wrapper.

class Accessors
{
    public int AutoProp { get; set; }

    private int _backing;
    public int Manual
    {
        get { return _backing; }
        set { _backing = value; }
    }

    public int ReadOnly { get; }
    public int WriteOnly { set { _backing = value; } }
}
