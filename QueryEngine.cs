using System.Xml;
using System.Xml.XPath;
using Microsoft.CodeAnalysis.CSharp;
using Wmhelp.XPath2;

namespace CodeXPath;

public static class QueryEngine
{
    public static List<Match> ProcessFile(string filePath, string code, string? xpath)
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
            matches.Add(new Match(filePath, 1, 1, 1, 1, reader.ReadToEnd(), sourceLines, null));
        }
        else
        {
            var doc = new XPathDocument(memStream);
            var nav = doc.CreateNavigator();
            var result = nav.XPath2Evaluate(xpath);

            if (result is IEnumerable<object> sequence)
            {
                foreach (var item in sequence)
                {
                    if (item is XPathNavigator current)
                    {
                        var (line, col, endLine, endCol) = GetLocation(current);
                        matches.Add(new Match(filePath, line, col, endLine, endCol, current.Value, sourceLines, current.Clone()));
                    }
                    else
                    {
                        matches.Add(new Match(filePath, 1, 1, 1, 1, item?.ToString() ?? "", sourceLines, null));
                    }
                }
            }
            else if (result is XPathNodeIterator iter)
            {
                while (iter.MoveNext())
                {
                    var current = iter.Current!;
                    var (line, col, endLine, endCol) = GetLocation(current);
                    matches.Add(new Match(filePath, line, col, endLine, endCol, current.Value, sourceLines, current.Clone()));
                }
            }
            else
            {
                matches.Add(new Match(filePath, 1, 1, 1, 1, result?.ToString() ?? "", sourceLines, null));
            }
        }

        return matches;
    }

    private static (int line, int col, int endLine, int endCol) GetLocation(XPathNavigator current)
    {
        int line = 1, col = 1, endLine = 1, endCol = 1;
        var lineAttr = current.GetAttribute("startLine", "");
        var colAttr = current.GetAttribute("startCol", "");
        var endLineAttr = current.GetAttribute("endLine", "");
        var endColAttr = current.GetAttribute("endCol", "");

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
