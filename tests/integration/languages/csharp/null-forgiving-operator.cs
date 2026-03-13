// Test: C# null-forgiving operator (!) should parse as postfix_unary_expression
// This was historically misidentified as broken due to shell escaping issues (! -> \!)

public class NullForgivingTest
{
    public void TestMethod()
    {
        string? nullable = GetNullable();

        // Simple null-forgiving
        var length = nullable!.Length;

        // Chained member access
        var upper = nullable!.ToUpper().Length;

        // In method call
        Process(nullable!);

        // Multiple on same line
        var combined = first!.Length + second!.Length;
    }

    private string? GetNullable() => null;
    private void Process(string s) { }

    private string? first;
    private string? second;
}
