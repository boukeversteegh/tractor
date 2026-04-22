// Conditional shape: flat <if>/<else_if>/<else>. Ternary keeps
// <then>/<else> via surgical wrap.

class Conditionals
{
    public string Classify(int n)
    {
        if (n < 0)
        {
            return "neg";
        }
        else if (n == 0)
        {
            return "zero";
        }
        else if (n < 10)
        {
            return "small";
        }
        else
        {
            return "big";
        }
    }

    public string Label(int n) => n > 0 ? "positive" : "non-positive";
}
