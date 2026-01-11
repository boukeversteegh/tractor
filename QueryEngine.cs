using System.Text;
using System.Xml;
using System.Xml.XPath;
using Microsoft.CodeAnalysis.CSharp;
using Wmhelp.XPath2;

namespace CodeXTractor;

public static class QueryEngine
{
    public static List<Match> ProcessFile(string filePath, string code, string? xpath, bool stripFullXmlLocations, bool verbose = false)
    {
        var matches = new List<Match>();
        var sourceLines = code.Split('\n');

        var tree = CSharpSyntaxTree.ParseText(code);
        var root = tree.GetRoot();

        var memStream = new MemoryStream();
        var settings = new XmlWriterSettings
        {
            Indent = true,
            IndentChars = "  ",
            OmitXmlDeclaration = false
        };

        using (var writer = XmlWriter.Create(memStream, settings))
        {
            XmlGenerator.WriteNode(writer, root);
        }

        memStream.Position = 0;

        if (xpath == null)
        {
            using var reader = new StreamReader(memStream);
            var xml = reader.ReadToEnd();
            var formatted = stripFullXmlLocations ? StripLocationMetadata(xml) : xml;
            matches.Add(new Match(filePath, 1, 1, 1, 1, formatted, sourceLines, null));
        }
        else
        {
            var doc = new XPathDocument(memStream);
            var nav = doc.CreateNavigator();
            var result = nav.XPath2Evaluate(xpath);

            // Debug: show result type
            if (verbose)
                Console.Error.WriteLine($"[verbose] result type: {result?.GetType().FullName ?? "null"}");

            // Normalize all result types to IEnumerable<object> for uniform processing
            IEnumerable<object> items = result switch
            {
                XPathNavigator singleNav => new object[] { singleNav },
                IEnumerable<object> seq => seq,
                XPathNodeIterator iter => IteratorToObjects(iter),
                _ => new object[] { result! }
            };

            foreach (var item in items)
            {
                if (item is XPathNavigator current)
                {
                    var (line, col, endLine, endCol) = GetLocation(current);
                    if (verbose)
                        Console.Error.WriteLine($"[verbose] location: {line}:{col} - {endLine}:{endCol}");
                    var value = GetMatchValue(current, sourceLines, line, col, endLine, endCol);
                    matches.Add(new Match(filePath, line, col, endLine, endCol, value, sourceLines, current.Clone()));
                }
                else
                {
                    matches.Add(new Match(filePath, 1, 1, 1, 1, item?.ToString() ?? "", sourceLines, null));
                }
            }
        }

        return matches;
    }

    private static IEnumerable<object> IteratorToObjects(XPathNodeIterator iter)
    {
        while (iter.MoveNext())
            yield return iter.Current!.Clone();
    }

    private static (int line, int col, int endLine, int endCol) GetLocation(XPathNavigator current)
    {
        int line = 1, col = 1, endLine = 1, endCol = 1;

        // If at document root, move to first child element to get location
        var nav = current;
        if (current.NodeType == XPathNodeType.Root)
        {
            nav = current.Clone();
            nav.MoveToFirstChild();
        }

        var lineAttr = nav.GetAttribute("startLine", "");
        var colAttr = nav.GetAttribute("startCol", "");
        var endLineAttr = nav.GetAttribute("endLine", "");
        var endColAttr = nav.GetAttribute("endCol", "");

        if (string.IsNullOrEmpty(lineAttr) && current.NodeType == XPathNodeType.Attribute)
        {
            var parent = current.Clone();
            parent.MoveToParent();
            lineAttr = parent.GetAttribute("startLine", "");
            colAttr = parent.GetAttribute("startCol", "");
            endLineAttr = parent.GetAttribute("endLine", "");
            endColAttr = parent.GetAttribute("endCol", "");
        }

        if (!string.IsNullOrEmpty(lineAttr)) int.TryParse(lineAttr, out line);
        if (!string.IsNullOrEmpty(colAttr)) int.TryParse(colAttr, out col);
        if (!string.IsNullOrEmpty(endLineAttr)) int.TryParse(endLineAttr, out endLine);
        if (!string.IsNullOrEmpty(endColAttr)) int.TryParse(endColAttr, out endCol);

        return (line, col, endLine, endCol);
    }

    private static string GetMatchValue(XPathNavigator current, string[] sourceLines, int line, int col, int endLine, int endCol)
    {
        if (current.NodeType is XPathNodeType.Attribute or XPathNodeType.Text or XPathNodeType.Comment or XPathNodeType.Namespace)
            return current.Value;

        // For Root node, use all source lines
        if (current.NodeType == XPathNodeType.Root)
            return string.Join("\n", sourceLines.Select(l => l.TrimEnd('\r')));

        var snippet = ExtractSourceSnippet(sourceLines, line, col, endLine, endCol);
        return string.IsNullOrEmpty(snippet) ? current.Value : snippet;
    }

    private static string ExtractSourceSnippet(string[] sourceLines, int startLine, int startColumn, int endLine, int endColumn)
    {
        if (sourceLines.Length == 0 || startLine <= 0 || startColumn <= 0 || endLine <= 0 || endColumn <= 0)
            return string.Empty;

        startLine = Math.Max(1, startLine);
        endLine = Math.Max(startLine, Math.Min(endLine, sourceLines.Length));

        var builder = new StringBuilder();

        for (int line = startLine; line <= endLine; line++)
        {
            var text = sourceLines[line - 1].TrimEnd('\r');
            var lineStartIndex = line == startLine
                ? Math.Max(0, Math.Min(text.Length, startColumn - 1))
                : 0;

            var desiredEnd = line == endLine ? endColumn - 1 : text.Length;
            var lineEndExclusive = Math.Max(lineStartIndex, Math.Min(text.Length, desiredEnd));

            if (lineEndExclusive > lineStartIndex)
                builder.Append(text[lineStartIndex..lineEndExclusive]);

            if (line < endLine)
                builder.Append('\n');
        }

        return builder.ToString();
    }

    internal static string StripLocationMetadata(string xml)
    {
        var doc = new XmlDocument { PreserveWhitespace = true };
        doc.LoadXml(xml);
        RemoveLocationAttributes(doc.DocumentElement);

        using var stringWriter = new StringWriter();
        using (var xmlWriter = XmlWriter.Create(stringWriter, new XmlWriterSettings
        {
            Indent = true,
            IndentChars = "  ",
            OmitXmlDeclaration = false
        }))
        {
            doc.Save(xmlWriter);
        }

        return stringWriter.ToString();
    }

    private static void RemoveLocationAttributes(XmlNode? node)
    {
        if (node == null)
            return;

        if (node.NodeType == XmlNodeType.Element && node.Attributes != null)
        {
            foreach (var attrName in LocationAttributeNames)
            {
                var attr = node.Attributes[attrName];
                if (attr != null)
                    node.Attributes.Remove(attr);
            }
        }

        foreach (XmlNode child in node.ChildNodes)
        {
            RemoveLocationAttributes(child);
        }
    }

    private static readonly string[] LocationAttributeNames =
    {
        "startLine",
        "startCol",
        "endLine",
        "endCol"
    };

    public static IEnumerable<string> ExpandGlob(string pattern)
    {
        if (pattern.Contains('*') || pattern.Contains('?'))
        {
            var dir = Path.GetDirectoryName(pattern);
            if (string.IsNullOrEmpty(dir)) dir = ".";
            var filePattern = Path.GetFileName(pattern);

            var searchOption = pattern.Contains("**")
                ? SearchOption.AllDirectories
                : SearchOption.TopDirectoryOnly;

            if (pattern.Contains("**"))
            {
                dir = pattern.Split("**")[0].TrimEnd('/', '\\');
                if (string.IsNullOrEmpty(dir)) dir = ".";
                filePattern = pattern.Split("**").Last().TrimStart('/', '\\');
            }

            return Directory.GetFiles(dir, filePattern, searchOption);
        }
        return new[] { pattern };
    }
}
