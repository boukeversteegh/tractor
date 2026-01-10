using System.Text.Json;
using System.Xml;
using System.Xml.XPath;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp;
using Microsoft.CodeAnalysis.CSharp.Syntax;
using Wmhelp.XPath2;

// Parse arguments
var files = new List<string>();
string? xpathExpr = null;
string? expectation = null; // "none", "some", or a number
string format = "lines";    // "lines", "gcc", "json", "count"
string? message = null;
bool fromStdin = false;

for (int i = 0; i < args.Length; i++)
{
    switch (args[i])
    {
        case "--stdin":
            fromStdin = true;
            break;
        case "--xpath" or "-x":
            xpathExpr = args[++i];
            break;
        case "--expect" or "-e":
            expectation = args[++i];
            break;
        case "--format" or "-f":
            format = args[++i];
            break;
        case "--message" or "-m":
            message = args[++i];
            break;
        case "--help" or "-h":
            PrintHelp();
            return 0;
        default:
            if (!args[i].StartsWith("-"))
                files.Add(args[i]);
            break;
    }
}

if (files.Count == 0 && !fromStdin)
{
    PrintHelp();
    return 1;
}

// Collect all matches across files
var allMatches = new List<Match>();
var filesToProcess = fromStdin
    ? new[] { ("stdin", Console.In.ReadToEnd()) }
    : files.SelectMany(ExpandGlob).Select(f => (f, File.ReadAllText(f)));

foreach (var (filePath, code) in filesToProcess)
{
    var matches = ProcessFile(filePath, code, xpathExpr);
    allMatches.AddRange(matches);
}

// Output results based on format
int matchCount = allMatches.Count;

switch (format)
{
    case "count":
        Console.WriteLine(matchCount);
        break;
    case "json":
        OutputJson(allMatches, message);
        break;
    case "gcc":
        OutputGcc(allMatches, message);
        break;
    case "lines":
    default:
        foreach (var match in allMatches)
            Console.WriteLine(match.Value);
        break;
}

// Determine exit code based on expectation
if (expectation != null)
{
    bool success = expectation switch
    {
        "none" => matchCount == 0,
        "some" => matchCount > 0,
        _ when int.TryParse(expectation, out int expected) => matchCount == expected,
        _ => true
    };

    if (!success)
    {
        if (format != "gcc" && format != "json")
        {
            string errorMsg = expectation switch
            {
                "none" => $"Expected no matches but found {matchCount}",
                "some" => "Expected matches but found none",
                _ => $"Expected {expectation} matches but found {matchCount}"
            };
            Console.Error.WriteLine($"error: {message ?? errorMsg}");
        }
        return 1;
    }
}

return 0;

// --- Helper functions ---

void PrintHelp()
{
    Console.WriteLine("""
        Usage: CodeXPath <files...> [options]
               CodeXPath --stdin [options]

        Options:
          -x, --xpath <expr>     XPath query (without: outputs full XML)
          -e, --expect <value>   Expected result: none, some, or a number
          -f, --format <fmt>     Output format: lines, gcc, json, count
          -m, --message <msg>    Custom error message for --expect failures
          -h, --help             Show this help

        Examples:
          # Output full XML AST
          CodeXPath Program.cs

          # Find all method names
          CodeXPath Program.cs -x "//method-decl/@name"

          # CI check - fail if any SQL missing semicolons
          CodeXPath "Migrations/*.cs" -x "//invocation-expr[...]/@startLine" \
              --expect none --format gcc --message "SQL must end with semicolon"

          # Ensure at least one test exists
          CodeXPath "**/*Tests.cs" -x "//method-decl[@name]" --expect some
        """);
}

IEnumerable<string> ExpandGlob(string pattern)
{
    if (pattern.Contains('*') || pattern.Contains('?'))
    {
        var dir = Path.GetDirectoryName(pattern);
        if (string.IsNullOrEmpty(dir)) dir = ".";
        var filePattern = Path.GetFileName(pattern);

        // Handle ** for recursive
        var searchOption = pattern.Contains("**")
            ? SearchOption.AllDirectories
            : SearchOption.TopDirectoryOnly;

        // Normalize pattern for Directory.GetFiles
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

List<Match> ProcessFile(string filePath, string code, string? xpath)
{
    var matches = new List<Match>();
    var sourceLines = code.Split('\n');

    var tree = CSharpSyntaxTree.ParseText(code);
    var root = tree.GetRoot();

    // Generate XML to memory
    var memStream = new MemoryStream();
    var settings = new XmlWriterSettings
    {
        Indent = true,
        IndentChars = "  ",
        OmitXmlDeclaration = false
    };

    using (var writer = XmlWriter.Create(memStream, settings))
    {
        WriteNode(writer, root);
    }

    memStream.Position = 0;

    if (xpath == null)
    {
        // No XPath - output full XML as single match
        using var reader = new StreamReader(memStream);
        matches.Add(new Match(filePath, 1, 1, 1, 1, reader.ReadToEnd(), sourceLines, null));
    }
    else
    {
        // Execute XPath 2.0 query
        var doc = new XPathDocument(memStream);
        var nav = doc.CreateNavigator();
        var result = nav.XPath2Evaluate(xpath);

        // XPath2 returns IEnumerable for node sequences, or scalar values
        if (result is IEnumerable<object> sequence)
        {
            foreach (var item in sequence)
            {
                if (item is XPathNavigator current)
                {
                    // Try to get location from attributes
                    int line = 1, col = 1, endLine = 1, endCol = 1;
                    var lineAttr = current.GetAttribute("startLine", "");
                    var colAttr = current.GetAttribute("startCol", "");
                    var endLineAttr = current.GetAttribute("endLine", "");
                    var endColAttr = current.GetAttribute("endCol", "");

                    // If this is an attribute, try parent's location
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

                    // Clone navigator so we can evaluate relative XPath later
                    matches.Add(new Match(filePath, line, col, endLine, endCol, current.Value, sourceLines, current.Clone()));
                }
                else
                {
                    // Scalar value in sequence
                    matches.Add(new Match(filePath, 1, 1, 1, 1, item?.ToString() ?? "", sourceLines, null));
                }
            }
        }
        else if (result is XPathNodeIterator iter)
        {
            // Fallback for XPath 1.0 style results
            while (iter.MoveNext())
            {
                var current = iter.Current!;
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

                matches.Add(new Match(filePath, line, col, endLine, endCol, current.Value, sourceLines, current.Clone()));
            }
        }
        else
        {
            // Scalar result (number, string, boolean)
            matches.Add(new Match(filePath, 1, 1, 1, 1, result?.ToString() ?? "", sourceLines, null));
        }
    }

    return matches;
}

void OutputGcc(List<Match> matches, string? customMessage)
{
    foreach (var m in matches)
    {
        var msg = FormatMessage(customMessage ?? "match", m);
        Console.WriteLine($"{m.File}:{m.Line}:{m.Column}: error: {msg}");

        // Show source snippet if available
        if (m.SourceLines.Length >= m.Line && m.Line > 0)
        {
            var startLine = m.Line;
            var endLine = Math.Min(m.EndLine, m.SourceLines.Length);
            var lineCount = endLine - startLine + 1;
            var lineNumWidth = endLine.ToString().Length;

            if (lineCount == 1)
            {
                // Single line - show with underline
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
                // Few lines - show all with line markers
                for (int i = startLine; i <= endLine; i++)
                {
                    var sourceLine = m.SourceLines[i - 1].TrimEnd('\r');
                    var marker = (i == startLine || i == endLine) ? ">" : " ";
                    Console.WriteLine($"{i.ToString().PadLeft(lineNumWidth)} {marker}| {sourceLine}");
                }
            }
            else
            {
                // Many lines - show first 2 and last 2
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
            Console.WriteLine(); // Blank line between matches
        }
    }
}

void OutputJson(List<Match> matches, string? customMessage)
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

void WriteNode(XmlWriter writer, SyntaxNode node)
{
    var elementName = GetElementName(node);
    writer.WriteStartElement(elementName);

    // Add location attributes
    var span = node.GetLocation().GetLineSpan();
    writer.WriteAttributeString("startLine", (span.StartLinePosition.Line + 1).ToString());
    writer.WriteAttributeString("startCol", (span.StartLinePosition.Character + 1).ToString());
    writer.WriteAttributeString("endLine", (span.EndLinePosition.Line + 1).ToString());
    writer.WriteAttributeString("endCol", (span.EndLinePosition.Character + 1).ToString());

    // Add kind for debugging/clarity
    writer.WriteAttributeString("kind", node.Kind().ToString());

    // Add node-specific attributes
    WriteNodeAttributes(writer, node);

    var children = node.ChildNodes().ToList();

    if (children.Count == 0)
    {
        // Leaf node - write the source as CDATA
        var text = node.ToString();
        if (NeedsCData(text))
        {
            writer.WriteCData(text);
        }
        else
        {
            writer.WriteString(text);
        }
    }
    else
    {
        // Has children - recurse
        foreach (var child in children)
        {
            WriteNode(writer, child);
        }
    }

    writer.WriteEndElement();
}

void WriteNodeAttributes(XmlWriter writer, SyntaxNode node)
{
    switch (node)
    {
        case ClassDeclarationSyntax cls:
            writer.WriteAttributeString("name", cls.Identifier.Text);
            WriteModifiers(writer, cls.Modifiers);
            break;
        case StructDeclarationSyntax str:
            writer.WriteAttributeString("name", str.Identifier.Text);
            WriteModifiers(writer, str.Modifiers);
            break;
        case InterfaceDeclarationSyntax iface:
            writer.WriteAttributeString("name", iface.Identifier.Text);
            WriteModifiers(writer, iface.Modifiers);
            break;
        case RecordDeclarationSyntax rec:
            writer.WriteAttributeString("name", rec.Identifier.Text);
            WriteModifiers(writer, rec.Modifiers);
            break;
        case EnumDeclarationSyntax enm:
            writer.WriteAttributeString("name", enm.Identifier.Text);
            WriteModifiers(writer, enm.Modifiers);
            break;
        case MethodDeclarationSyntax method:
            writer.WriteAttributeString("name", method.Identifier.Text);
            WriteModifiers(writer, method.Modifiers);
            break;
        case ConstructorDeclarationSyntax ctor:
            writer.WriteAttributeString("name", ctor.Identifier.Text);
            WriteModifiers(writer, ctor.Modifiers);
            break;
        case PropertyDeclarationSyntax prop:
            writer.WriteAttributeString("name", prop.Identifier.Text);
            WriteModifiers(writer, prop.Modifiers);
            break;
        case FieldDeclarationSyntax field:
            WriteModifiers(writer, field.Modifiers);
            break;
        case EventDeclarationSyntax evt:
            writer.WriteAttributeString("name", evt.Identifier.Text);
            WriteModifiers(writer, evt.Modifiers);
            break;
        case ParameterSyntax param:
            writer.WriteAttributeString("name", param.Identifier.Text);
            WriteModifiers(writer, param.Modifiers);
            if (param.Modifiers.Any(SyntaxKind.ThisKeyword))
            {
                writer.WriteAttributeString("this", "true");
            }
            break;
        case VariableDeclaratorSyntax varDecl:
            writer.WriteAttributeString("name", varDecl.Identifier.Text);
            break;
        case IdentifierNameSyntax id:
            writer.WriteAttributeString("name", id.Identifier.Text);
            break;
        case GenericNameSyntax generic:
            writer.WriteAttributeString("name", generic.Identifier.Text);
            break;
        case TypeParameterSyntax typeParam:
            writer.WriteAttributeString("name", typeParam.Identifier.Text);
            break;
        case NamespaceDeclarationSyntax ns:
            writer.WriteAttributeString("name", ns.Name.ToString());
            break;
        case FileScopedNamespaceDeclarationSyntax fsns:
            writer.WriteAttributeString("name", fsns.Name.ToString());
            break;
        case LiteralExpressionSyntax literal when literal.IsKind(SyntaxKind.StringLiteralExpression):
            // Store the parsed string value for XPath 2.0 string functions (ends-with, matches, etc.)
            var stringValue = literal.Token.ValueText;
            if (stringValue != null)
                writer.WriteAttributeString("textValue", stringValue);
            break;
        case AttributeSyntax attr:
            writer.WriteAttributeString("name", attr.Name.ToString());
            break;
    }
}

void WriteModifiers(XmlWriter writer, SyntaxTokenList modifiers)
{
    if (modifiers.Count == 0) return;
    var modifierStrings = modifiers.Select(m => m.Text).ToList();
    writer.WriteAttributeString("modifiers", string.Join(" ", modifierStrings));
}

string GetElementName(SyntaxNode node)
{
    var kind = node.Kind().ToString();

    // Remove common suffixes and use abbreviated forms
    if (kind.EndsWith("Syntax"))
        kind = kind[..^6];
    if (kind.EndsWith("Declaration"))
        kind = kind[..^11] + "Decl";
    if (kind.EndsWith("Statement"))
        kind = kind[..^9] + "Stmt";
    if (kind.EndsWith("Expression"))
        kind = kind[..^10] + "Expr";

    return kind; // Keep PascalCase
}

bool NeedsCData(string text)
{
    return text.Contains('<') || text.Contains('>') || text.Contains('&') || text.Contains("]]>");
}

string FormatMessage(string template, Match match)
{
    if (!template.Contains('{'))
        return template;

    // Replace {xpath} placeholders with evaluated results
    return System.Text.RegularExpressions.Regex.Replace(template, @"\{([^}]+)\}", m =>
    {
        var expr = m.Groups[1].Value;

        // Built-in placeholders
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

string EvaluateRelativeXPath(XPathNavigator? nav, string xpath)
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
        return $"{{{xpath}}}"; // Return original on error
    }
}

string Truncate(string s, int maxLen)
{
    if (string.IsNullOrEmpty(s)) return s;
    // Collapse whitespace for display
    s = System.Text.RegularExpressions.Regex.Replace(s, @"\s+", " ").Trim();
    return s.Length <= maxLen ? s : s[..(maxLen - 3)] + "...";
}

record Match(string File, int Line, int Column, int EndLine, int EndColumn, string Value, string[] SourceLines, XPathNavigator? Navigator);
