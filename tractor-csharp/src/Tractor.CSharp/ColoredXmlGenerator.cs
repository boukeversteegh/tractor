using System.Text;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp;
using Microsoft.CodeAnalysis.CSharp.Syntax;

namespace Tractor.CSharp;

/// <summary>
/// Generates colored XML output using Modern Agri-Tech palette
/// </summary>
public static class ColoredXmlGenerator
{
    public static void WriteDocument(TextWriter writer, IEnumerable<(string path, SyntaxNode root)> files)
    {
        writer.WriteLine("<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
        WriteOpenTag(writer, "Files", 0);

        foreach (var (path, root) in files)
        {
            WriteFileStart(writer, path);
            WriteNode(writer, root, 2);
            WriteCloseTag(writer, "File", 1);
        }

        WriteCloseTag(writer, "Files", 0);
    }

    private static void WriteFileStart(TextWriter writer, string path)
    {
        var indent = "  ";
        if (Colors.UseColor)
        {
            // Pattern: DIM< RESET GREEN(File) RESET CYAN(path) DIM= RESET YELLOW("value") DIM> RESET
            writer.WriteLine($"{indent}{Colors.Dim}<{Colors.Reset}{Colors.Green}File{Colors.Reset} {Colors.Cyan}path{Colors.Dim}={Colors.Reset}{Colors.Yellow}\"{EscapeXml(path)}\"{Colors.Dim}>{Colors.Reset}");
        }
        else
        {
            writer.WriteLine($"{indent}<File path=\"{EscapeXml(path)}\">");
        }
    }

    private static void WriteOpenTag(TextWriter writer, string name, int indentLevel)
    {
        var indent = new string(' ', indentLevel * 2);
        if (Colors.UseColor)
        {
            // Pattern: DIM< RESET GREEN(name) DIM> RESET
            writer.WriteLine($"{indent}{Colors.Dim}<{Colors.Reset}{Colors.Green}{name}{Colors.Dim}>{Colors.Reset}");
        }
        else
        {
            writer.WriteLine($"{indent}<{name}>");
        }
    }

    private static void WriteCloseTag(TextWriter writer, string name, int indentLevel)
    {
        var indent = new string(' ', indentLevel * 2);
        if (Colors.UseColor)
        {
            // Pattern: DIM</ RESET GREEN(name) DIM> RESET
            writer.WriteLine($"{indent}{Colors.Dim}<{Colors.Reset}/{Colors.Green}{name}{Colors.Dim}>{Colors.Reset}");
        }
        else
        {
            writer.WriteLine($"{indent}</{name}>");
        }
    }

    public static void WriteNode(TextWriter writer, SyntaxNode node, int indentLevel = 0)
    {
        var indent = new string(' ', indentLevel * 2);
        var elementName = GetElementName(node);

        // Get location
        var span = node.GetLocation().GetLineSpan();
        var attrs = new List<(string name, string value)>
        {
            ("startLine", (span.StartLinePosition.Line + 1).ToString()),
            ("startCol", (span.StartLinePosition.Character + 1).ToString()),
            ("endLine", (span.EndLinePosition.Line + 1).ToString()),
            ("endCol", (span.EndLinePosition.Character + 1).ToString()),
            ("kind", node.Kind().ToString())
        };

        // Add node-specific attributes
        AddNodeAttributes(attrs, node);

        var children = node.ChildNodes().ToList();

        if (children.Count == 0)
        {
            // Leaf node - include text content
            var text = node.ToString();
            WriteElementWithContent(writer, indent, elementName, attrs, text);
        }
        else
        {
            // Node with children
            WriteElementOpen(writer, indent, elementName, attrs);
            foreach (var child in children)
                WriteNode(writer, child, indentLevel + 1);
            WriteCloseTag(writer, elementName, indentLevel);
        }
    }

    private static void WriteElementOpen(TextWriter writer, string indent, string name, List<(string name, string value)> attrs)
    {
        if (Colors.UseColor)
        {
            var sb = new StringBuilder();
            // Pattern: DIM< RESET GREEN(name) then for each attr: RESET CYAN(attr) DIM= RESET YELLOW("value") then DIM> RESET
            sb.Append($"{indent}{Colors.Dim}<{Colors.Reset}{Colors.Green}{name}");
            foreach (var (attrName, attrValue) in attrs)
            {
                sb.Append($"{Colors.Reset} {Colors.Cyan}{attrName}{Colors.Dim}={Colors.Reset}{Colors.Yellow}\"{EscapeXml(attrValue)}\"");
            }
            sb.Append($"{Colors.Dim}>{Colors.Reset}");
            writer.WriteLine(sb.ToString());
        }
        else
        {
            var sb = new StringBuilder();
            sb.Append($"{indent}<{name}");
            foreach (var (attrName, attrValue) in attrs)
            {
                sb.Append($" {attrName}=\"{EscapeXml(attrValue)}\"");
            }
            sb.Append(">");
            writer.WriteLine(sb.ToString());
        }
    }

    private static void WriteElementWithContent(TextWriter writer, string indent, string name, List<(string name, string value)> attrs, string content)
    {
        var escapedContent = NeedsCData(content) ? $"<![CDATA[{content}]]>" : EscapeXml(content);

        if (Colors.UseColor)
        {
            var sb = new StringBuilder();
            // Pattern: DIM< RESET GREEN(name) attrs DIM> RESET WHITE(content) RESET DIM</ RESET GREEN(name) DIM> RESET
            sb.Append($"{indent}{Colors.Dim}<{Colors.Reset}{Colors.Green}{name}");
            foreach (var (attrName, attrValue) in attrs)
            {
                sb.Append($"{Colors.Reset} {Colors.Cyan}{attrName}{Colors.Dim}={Colors.Reset}{Colors.Yellow}\"{EscapeXml(attrValue)}\"");
            }
            sb.Append($"{Colors.Dim}>{Colors.Reset}");
            sb.Append($"{Colors.White}{escapedContent}{Colors.Reset}");
            sb.Append($"{Colors.Dim}<{Colors.Reset}/{Colors.Green}{name}{Colors.Dim}>{Colors.Reset}");
            writer.WriteLine(sb.ToString());
        }
        else
        {
            var sb = new StringBuilder();
            sb.Append($"{indent}<{name}");
            foreach (var (attrName, attrValue) in attrs)
            {
                sb.Append($" {attrName}=\"{EscapeXml(attrValue)}\"");
            }
            sb.Append($">{escapedContent}</{name}>");
            writer.WriteLine(sb.ToString());
        }
    }

    private static void AddNodeAttributes(List<(string name, string value)> attrs, SyntaxNode node)
    {
        switch (node)
        {
            case ClassDeclarationSyntax cls:
                attrs.Add(("name", cls.Identifier.Text));
                AddModifiers(attrs, cls.Modifiers);
                break;
            case StructDeclarationSyntax str:
                attrs.Add(("name", str.Identifier.Text));
                AddModifiers(attrs, str.Modifiers);
                break;
            case InterfaceDeclarationSyntax iface:
                attrs.Add(("name", iface.Identifier.Text));
                AddModifiers(attrs, iface.Modifiers);
                break;
            case RecordDeclarationSyntax rec:
                attrs.Add(("name", rec.Identifier.Text));
                AddModifiers(attrs, rec.Modifiers);
                break;
            case EnumDeclarationSyntax enm:
                attrs.Add(("name", enm.Identifier.Text));
                AddModifiers(attrs, enm.Modifiers);
                break;
            case MethodDeclarationSyntax method:
                attrs.Add(("name", method.Identifier.Text));
                AddModifiers(attrs, method.Modifiers);
                break;
            case ConstructorDeclarationSyntax ctor:
                attrs.Add(("name", ctor.Identifier.Text));
                AddModifiers(attrs, ctor.Modifiers);
                break;
            case PropertyDeclarationSyntax prop:
                attrs.Add(("name", prop.Identifier.Text));
                AddModifiers(attrs, prop.Modifiers);
                break;
            case FieldDeclarationSyntax field:
                AddModifiers(attrs, field.Modifiers);
                break;
            case EventDeclarationSyntax evt:
                attrs.Add(("name", evt.Identifier.Text));
                AddModifiers(attrs, evt.Modifiers);
                break;
            case ParameterSyntax param:
                attrs.Add(("name", param.Identifier.Text));
                AddModifiers(attrs, param.Modifiers);
                break;
            case VariableDeclaratorSyntax varDecl:
                attrs.Add(("name", varDecl.Identifier.Text));
                break;
            case IdentifierNameSyntax id:
                attrs.Add(("name", id.Identifier.Text));
                break;
            case GenericNameSyntax generic:
                attrs.Add(("name", generic.Identifier.Text));
                break;
            case TypeParameterSyntax typeParam:
                attrs.Add(("name", typeParam.Identifier.Text));
                break;
            case NamespaceDeclarationSyntax ns:
                attrs.Add(("name", ns.Name.ToString()));
                break;
            case FileScopedNamespaceDeclarationSyntax fsns:
                attrs.Add(("name", fsns.Name.ToString()));
                break;
            case LiteralExpressionSyntax literal when literal.IsKind(SyntaxKind.StringLiteralExpression):
                var stringValue = literal.Token.ValueText;
                if (stringValue != null)
                    attrs.Add(("textValue", stringValue));
                break;
            case AttributeSyntax attr:
                attrs.Add(("name", attr.Name.ToString()));
                break;
        }
    }

    private static void AddModifiers(List<(string name, string value)> attrs, SyntaxTokenList modifiers)
    {
        if (modifiers.Count > 0)
        {
            var modifierStrings = modifiers.Select(m => m.Text).ToList();
            attrs.Add(("modifiers", string.Join(" ", modifierStrings)));
        }
    }

    private static string GetElementName(SyntaxNode node)
    {
        // Reuse logic from XmlGenerator
        return node.Kind() switch
        {
            SyntaxKind.IfStatement => "if",
            SyntaxKind.ElseClause => "else",
            SyntaxKind.SwitchStatement => "switch",
            SyntaxKind.ForStatement => "for",
            SyntaxKind.ForEachStatement => "foreach",
            SyntaxKind.WhileStatement => "while",
            SyntaxKind.DoStatement => "do",
            SyntaxKind.TryStatement => "try",
            SyntaxKind.CatchClause => "catch",
            SyntaxKind.FinallyClause => "finally",
            SyntaxKind.ReturnStatement => "return",
            SyntaxKind.ThrowStatement => "throw",
            SyntaxKind.UsingStatement => "using",
            SyntaxKind.UsingDirective => "using",
            SyntaxKind.ClassDeclaration => "class",
            SyntaxKind.StructDeclaration => "struct",
            SyntaxKind.InterfaceDeclaration => "interface",
            SyntaxKind.EnumDeclaration => "enum",
            SyntaxKind.RecordDeclaration => "record",
            SyntaxKind.NamespaceDeclaration => "namespace",
            SyntaxKind.FileScopedNamespaceDeclaration => "namespace",
            SyntaxKind.MethodDeclaration => "Method",
            SyntaxKind.ConstructorDeclaration => "Constructor",
            SyntaxKind.PropertyDeclaration => "Property",
            SyntaxKind.FieldDeclaration => "Field",
            SyntaxKind.Parameter => "Param",
            SyntaxKind.ParameterList => "Params",
            SyntaxKind.Block => "Block",
            SyntaxKind.InvocationExpression => "Call",
            SyntaxKind.SimpleMemberAccessExpression => "Access",
            SyntaxKind.IdentifierName => "Id",
            SyntaxKind.StringLiteralExpression => "String",
            SyntaxKind.NumericLiteralExpression => "Number",
            SyntaxKind.CompilationUnit => "CompilationUnit",
            _ => DeriveElementName(node.Kind().ToString())
        };
    }

    private static string DeriveElementName(string kind)
    {
        if (kind.EndsWith("Syntax")) kind = kind[..^6];
        if (kind.EndsWith("Declaration")) kind = kind[..^11];
        if (kind.EndsWith("Statement")) kind = kind[..^9];
        if (kind.EndsWith("Expression")) kind = kind[..^10];
        if (kind.EndsWith("Clause")) kind = kind[..^6];
        if (kind.EndsWith("List")) kind = kind[..^4] + "s";
        return kind;
    }

    private static string EscapeXml(string s)
    {
        return s.Replace("&", "&amp;")
                .Replace("<", "&lt;")
                .Replace(">", "&gt;")
                .Replace("\"", "&quot;");
    }

    private static bool NeedsCData(string text)
    {
        return text.Contains('<') || text.Contains('>') || text.Contains('&') || text.Contains("]]>");
    }
}
