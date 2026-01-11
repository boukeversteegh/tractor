namespace Tractor.CSharp;

/// <summary>
/// Modern Agri-Tech color palette for tractor toolchain
/// </summary>
public static class Colors
{
    public const string Reset = "\x1b[0m";
    public const string Dim = "\x1b[2;37m";     // Punctuation: < > = / (dim white)
    public const string Green = "\x1b[32m";     // Element names (fresh/growth)
    public const string Cyan = "\x1b[36m";      // Attribute names (tech accent)
    public const string Yellow = "\x1b[33m";    // Attribute values (harvest gold)
    public const string White = "\x1b[97m";     // Text content (clean)

    public static bool UseColor { get; set; } = false;

    public static string Colorize(string text, string color)
        => UseColor ? $"{color}{text}{Reset}" : text;

    public static string Element(string name) => Colorize(name, Green);
    public static string Attr(string name) => Colorize(name, Cyan);
    public static string Value(string value) => Colorize(value, Yellow);
    public static string Content(string text) => Colorize(text, White);
    public static string Punct(string p) => Colorize(p, Dim);
}
