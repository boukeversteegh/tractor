using System.Text.Json;
using System.Text.RegularExpressions;
using System.Xml.XPath;
using Wmhelp.XPath2;

namespace Tractor.XPath;

public static class OutputFormatter
{
    public static void OutputGcc(List<Match> matches, string? customMessage)
    {
        foreach (var m in matches)
        {
            var msg = FormatMessage(customMessage ?? "match", m);
            Console.WriteLine($"{m.File}:{m.Line}:{m.Column}: error: {msg}");

            if (m.SourceLines.Length >= m.Line && m.Line > 0)
            {
                var startLine = m.Line;
                var endLine = Math.Min(m.EndLine, m.SourceLines.Length);
                var lineCount = endLine - startLine + 1;
                var lineNumWidth = endLine.ToString().Length;

                if (lineCount == 1)
                {
                    var sourceLine = m.SourceLines[startLine - 1].TrimEnd('\r');
                    Console.WriteLine($"{startLine.ToString().PadLeft(lineNumWidth)} | {sourceLine}");

                    var caretCol = m.Column - 1;
                    var underlineLen = Math.Max(1, m.EndColumn - m.Column);
                    var padding = new string(' ', lineNumWidth + 3 + caretCol);
                    var underline = "^" + new string('~', Math.Max(0, underlineLen - 1));
                    Console.WriteLine($"{padding}{underline}");
                }
                else if (lineCount <= 6)
                {
                    for (int i = startLine; i <= endLine; i++)
                    {
                        var sourceLine = m.SourceLines[i - 1].TrimEnd('\r');
                        var marker = (i == startLine || i == endLine) ? ">" : " ";
                        Console.WriteLine($"{i.ToString().PadLeft(lineNumWidth)} {marker}| {sourceLine}");
                    }
                }
                else
                {
                    for (int i = startLine; i < startLine + 2 && i <= endLine; i++)
                    {
                        var sourceLine = m.SourceLines[i - 1].TrimEnd('\r');
                        Console.WriteLine($"{i.ToString().PadLeft(lineNumWidth)} >| {sourceLine}");
                    }
                    Console.WriteLine($"{"...".PadLeft(lineNumWidth)}  | ... ({lineCount - 4} more lines)");
                    for (int i = endLine - 1; i <= endLine; i++)
                    {
                        var sourceLine = m.SourceLines[i - 1].TrimEnd('\r');
                        Console.WriteLine($"{i.ToString().PadLeft(lineNumWidth)} >| {sourceLine}");
                    }
                }
                Console.WriteLine();
            }
        }
    }

    public static void OutputJson(List<Match> matches, string? customMessage)
    {
        var output = matches.Select(m => new
        {
            file = m.File,
            line = m.Line,
            column = m.Column,
            value = m.Value,
            message = customMessage
        });
        Console.WriteLine(JsonSerializer.Serialize(output, new JsonSerializerOptions { WriteIndented = true }));
    }

    public static string FormatMessage(string template, Match match)
    {
        if (!template.Contains('{'))
            return template;

        return Regex.Replace(template, @"\{([^}]+)\}", m =>
        {
            var expr = m.Groups[1].Value;
            return expr switch
            {
                "value" => Truncate(match.Value, 50),
                "line" => match.Line.ToString(),
                "col" => match.Column.ToString(),
                "file" => match.File,
                _ => EvaluateRelativeXPath(match.Navigator, expr)
            };
        });
    }

    private static string EvaluateRelativeXPath(XPathNavigator? nav, string xpath)
    {
        if (nav == null) return $"{{{xpath}}}";

        try
        {
            var result = nav.XPath2Evaluate(xpath);
            if (result is IEnumerable<object> sequence)
            {
                var first = sequence.FirstOrDefault();
                if (first is XPathNavigator navResult)
                    return navResult.Value;
                return first?.ToString() ?? "";
            }
            if (result is XPathNodeIterator iter)
            {
                if (iter.MoveNext())
                    return iter.Current?.Value ?? "";
                return "";
            }
            return result?.ToString() ?? "";
        }
        catch
        {
            return $"{{{xpath}}}";
        }
    }

    private static string Truncate(string s, int maxLen)
    {
        if (string.IsNullOrEmpty(s)) return s;
        s = Regex.Replace(s, @"\s+", " ").Trim();
        return s.Length <= maxLen ? s : s[..(maxLen - 3)] + "...";
    }

    // ANSI color codes
    private const string Reset = "\x1b[0m";
    private const string Dim = "\x1b[2m";
    private const string Blue = "\x1b[34m";
    private const string Cyan = "\x1b[36m";
    private const string Yellow = "\x1b[33m";
    private const string Green = "\x1b[32m";
    private const string Gray = "\x1b[90m";

    public static string ColorizeXml(string xml)
    {
        if (string.IsNullOrEmpty(xml))
            return xml;

        // XML declaration: <?xml ... ?>
        xml = Regex.Replace(xml, @"(<\?)(xml)([^?]*)(\?>)",
            m => $"{Dim}{m.Groups[1].Value}{m.Groups[2].Value}{m.Groups[3].Value}{m.Groups[4].Value}{Reset}");

        // CDATA sections: <![CDATA[...]]>
        xml = Regex.Replace(xml, @"(<!\[CDATA\[)(.*?)(\]\]>)",
            m => $"{Dim}{m.Groups[1].Value}{Reset}{Green}{m.Groups[2].Value}{Reset}{Dim}{m.Groups[3].Value}{Reset}",
            RegexOptions.Singleline);

        // Comments: <!-- ... -->
        xml = Regex.Replace(xml, @"(<!--)(.*?)(-->)",
            m => $"{Dim}{m.Groups[1].Value}{m.Groups[2].Value}{m.Groups[3].Value}{Reset}",
            RegexOptions.Singleline);

        // Attributes: name="value" or name='value'
        xml = Regex.Replace(xml, @"(\s)([a-zA-Z_][\w\-]*)(\s*=\s*)([""'])([^""']*)\4",
            m => $"{m.Groups[1].Value}{Cyan}{m.Groups[2].Value}{Reset}{Dim}{m.Groups[3].Value}{m.Groups[4].Value}{Reset}{Yellow}{m.Groups[5].Value}{Reset}{Dim}{m.Groups[4].Value}{Reset}");

        // Opening tags: <elementName (but not attributes)
        xml = Regex.Replace(xml, @"(<)(/?)([\w][\w\-\.]*)",
            m => $"{Dim}{m.Groups[1].Value}{m.Groups[2].Value}{Reset}{Blue}{m.Groups[3].Value}{Reset}");

        // Closing brackets: > and />
        xml = Regex.Replace(xml, @"(\s*/?>)", $"{Dim}$1{Reset}");

        return xml;
    }
}
