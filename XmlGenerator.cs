using System.Xml;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp;
using Microsoft.CodeAnalysis.CSharp.Syntax;

namespace CodeXPath;

public static class XmlGenerator
{
    public static void WriteNode(XmlWriter writer, SyntaxNode node)
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
                writer.WriteCData(text);
            else
                writer.WriteString(text);
        }
        else
        {
            foreach (var child in children)
                WriteNode(writer, child);
        }

        writer.WriteEndElement();
    }

    private static void WriteNodeAttributes(XmlWriter writer, SyntaxNode node)
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
                    writer.WriteAttributeString("this", "true");
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
                var stringValue = literal.Token.ValueText;
                if (stringValue != null)
                    writer.WriteAttributeString("textValue", stringValue);
                break;
            case AttributeSyntax attr:
                writer.WriteAttributeString("name", attr.Name.ToString());
                break;
        }
    }

    private static void WriteModifiers(XmlWriter writer, SyntaxTokenList modifiers)
    {
        if (modifiers.Count == 0) return;
        var modifierStrings = modifiers.Select(m => m.Text).ToList();
        writer.WriteAttributeString("modifiers", string.Join(" ", modifierStrings));
    }

    private static string GetElementName(SyntaxNode node)
    {
        return node.Kind() switch
        {
            // Statements → keywords
            SyntaxKind.IfStatement => "if",
            SyntaxKind.ElseClause => "else",
            SyntaxKind.SwitchStatement => "switch",
            SyntaxKind.SwitchSection => "case",
            SyntaxKind.CaseSwitchLabel => "case",
            SyntaxKind.DefaultSwitchLabel => "default",
            SyntaxKind.ForStatement => "for",
            SyntaxKind.ForEachStatement => "foreach",
            SyntaxKind.WhileStatement => "while",
            SyntaxKind.DoStatement => "do",
            SyntaxKind.TryStatement => "try",
            SyntaxKind.CatchClause => "catch",
            SyntaxKind.FinallyClause => "finally",
            SyntaxKind.ReturnStatement => "return",
            SyntaxKind.BreakStatement => "break",
            SyntaxKind.ContinueStatement => "continue",
            SyntaxKind.ThrowStatement => "throw",
            SyntaxKind.ThrowExpression => "throw",
            SyntaxKind.UsingStatement => "using",
            SyntaxKind.UsingDirective => "using",
            SyntaxKind.LockStatement => "lock",
            SyntaxKind.FixedStatement => "fixed",
            SyntaxKind.CheckedStatement => "checked",
            SyntaxKind.UncheckedStatement => "unchecked",
            SyntaxKind.CheckedExpression => "checked",
            SyntaxKind.UncheckedExpression => "unchecked",
            SyntaxKind.GotoStatement => "goto",
            SyntaxKind.YieldReturnStatement => "yield",
            SyntaxKind.YieldBreakStatement => "yield",

            // Declarations → keywords
            SyntaxKind.ClassDeclaration => "class",
            SyntaxKind.StructDeclaration => "struct",
            SyntaxKind.InterfaceDeclaration => "interface",
            SyntaxKind.EnumDeclaration => "enum",
            SyntaxKind.RecordDeclaration => "record",
            SyntaxKind.RecordStructDeclaration => "record",
            SyntaxKind.NamespaceDeclaration => "namespace",
            SyntaxKind.FileScopedNamespaceDeclaration => "namespace",
            SyntaxKind.DelegateDeclaration => "delegate",
            SyntaxKind.EventDeclaration => "event",
            SyntaxKind.EventFieldDeclaration => "event",
            SyntaxKind.OperatorDeclaration => "operator",

            // Expressions with keywords
            SyntaxKind.ObjectCreationExpression => "new",
            SyntaxKind.ImplicitObjectCreationExpression => "new",
            SyntaxKind.ArrayCreationExpression => "new",
            SyntaxKind.AnonymousObjectCreationExpression => "new",
            SyntaxKind.AwaitExpression => "await",
            SyntaxKind.TypeOfExpression => "typeof",
            SyntaxKind.SizeOfExpression => "sizeof",
            SyntaxKind.DefaultExpression => "default",
            SyntaxKind.IsExpression => "is",
            SyntaxKind.IsPatternExpression => "is",
            SyntaxKind.AsExpression => "as",
            SyntaxKind.BaseExpression => "base",
            SyntaxKind.ThisExpression => "this",
            SyntaxKind.NullLiteralExpression => "null",
            SyntaxKind.TrueLiteralExpression => "true",
            SyntaxKind.FalseLiteralExpression => "false",
            SyntaxKind.StackAllocArrayCreationExpression => "stackalloc",
            SyntaxKind.RefExpression => "ref",

            // Other keywords
            SyntaxKind.Attribute => "Attribute",
            SyntaxKind.AttributeList => "Attributes",

            // Structural concepts (no keyword) → PascalCase
            SyntaxKind.MethodDeclaration => "Method",
            SyntaxKind.ConstructorDeclaration => "Constructor",
            SyntaxKind.DestructorDeclaration => "Destructor",
            SyntaxKind.PropertyDeclaration => "Property",
            SyntaxKind.IndexerDeclaration => "Indexer",
            SyntaxKind.FieldDeclaration => "Field",
            SyntaxKind.LocalFunctionStatement => "Function",
            SyntaxKind.Parameter => "Param",
            SyntaxKind.ParameterList => "Params",
            SyntaxKind.Argument => "Arg",
            SyntaxKind.ArgumentList => "Args",
            SyntaxKind.BracketedArgumentList => "Args",
            SyntaxKind.TypeParameter => "TypeParam",
            SyntaxKind.TypeParameterList => "TypeParams",
            SyntaxKind.TypeArgumentList => "TypeArgs",
            SyntaxKind.Block => "Block",
            SyntaxKind.LocalDeclarationStatement => "Var",
            SyntaxKind.VariableDeclaration => "VarDecl",
            SyntaxKind.VariableDeclarator => "Variable",
            SyntaxKind.InvocationExpression => "Call",
            SyntaxKind.SimpleMemberAccessExpression => "Access",
            SyntaxKind.ElementAccessExpression => "Index",
            SyntaxKind.ConditionalAccessExpression => "Access",
            SyntaxKind.MemberBindingExpression => "Bind",
            SyntaxKind.ConditionalExpression => "Ternary",
            SyntaxKind.SimpleLambdaExpression => "Lambda",
            SyntaxKind.ParenthesizedLambdaExpression => "Lambda",
            SyntaxKind.AnonymousMethodExpression => "Lambda",
            SyntaxKind.IdentifierName => "Id",
            SyntaxKind.GenericName => "Generic",
            SyntaxKind.QualifiedName => "Qualified",
            SyntaxKind.AliasQualifiedName => "Alias",
            SyntaxKind.PredefinedType => "Type",
            SyntaxKind.ArrayType => "ArrayType",
            SyntaxKind.NullableType => "NullableType",
            SyntaxKind.TupleType => "TupleType",
            SyntaxKind.TupleElement => "TupleElement",
            SyntaxKind.TupleExpression => "Tuple",
            SyntaxKind.CastExpression => "Cast",
            SyntaxKind.ParenthesizedExpression => "Paren",
            SyntaxKind.SimpleAssignmentExpression or
            SyntaxKind.AddAssignmentExpression or
            SyntaxKind.SubtractAssignmentExpression or
            SyntaxKind.MultiplyAssignmentExpression or
            SyntaxKind.DivideAssignmentExpression => "Assign",
            SyntaxKind.CoalesceAssignmentExpression => "Assign",
            SyntaxKind.AddExpression or
            SyntaxKind.SubtractExpression or
            SyntaxKind.MultiplyExpression or
            SyntaxKind.DivideExpression or
            SyntaxKind.ModuloExpression => "Binary",
            SyntaxKind.EqualsExpression or
            SyntaxKind.NotEqualsExpression or
            SyntaxKind.LessThanExpression or
            SyntaxKind.LessThanOrEqualExpression or
            SyntaxKind.GreaterThanExpression or
            SyntaxKind.GreaterThanOrEqualExpression => "Compare",
            SyntaxKind.LogicalAndExpression or
            SyntaxKind.LogicalOrExpression => "Logical",
            SyntaxKind.LogicalNotExpression => "Not",
            SyntaxKind.BitwiseAndExpression or
            SyntaxKind.BitwiseOrExpression or
            SyntaxKind.ExclusiveOrExpression or
            SyntaxKind.LeftShiftExpression or
            SyntaxKind.RightShiftExpression => "Bitwise",
            SyntaxKind.CoalesceExpression => "Coalesce",
            SyntaxKind.UnaryPlusExpression or
            SyntaxKind.UnaryMinusExpression or
            SyntaxKind.BitwiseNotExpression => "Unary",
            SyntaxKind.PreIncrementExpression or
            SyntaxKind.PreDecrementExpression or
            SyntaxKind.PostIncrementExpression or
            SyntaxKind.PostDecrementExpression => "Increment",
            SyntaxKind.StringLiteralExpression => "String",
            SyntaxKind.NumericLiteralExpression => "Number",
            SyntaxKind.CharacterLiteralExpression => "Char",
            SyntaxKind.InterpolatedStringExpression => "InterpolatedString",
            SyntaxKind.Interpolation => "Interpolation",
            SyntaxKind.ExpressionStatement => "Expr",
            SyntaxKind.EmptyStatement => "Empty",
            SyntaxKind.LabeledStatement => "Label",
            SyntaxKind.GetAccessorDeclaration => "get",
            SyntaxKind.SetAccessorDeclaration => "set",
            SyntaxKind.InitAccessorDeclaration => "init",
            SyntaxKind.AddAccessorDeclaration => "add",
            SyntaxKind.RemoveAccessorDeclaration => "remove",
            SyntaxKind.AccessorList => "Accessors",
            SyntaxKind.ArrowExpressionClause => "Arrow",
            SyntaxKind.EqualsValueClause => "Equals",
            SyntaxKind.BaseList => "BaseList",
            SyntaxKind.SimpleBaseType => "BaseType",
            SyntaxKind.EnumMemberDeclaration => "EnumMember",
            SyntaxKind.CompilationUnit => "File",
            SyntaxKind.GlobalStatement => "Global",

            _ => DeriveElementName(node.Kind().ToString())
        };
    }

    private static string DeriveElementName(string kind)
    {
        if (kind.EndsWith("Syntax"))
            kind = kind[..^6];
        if (kind.EndsWith("Declaration"))
            kind = kind[..^11];
        if (kind.EndsWith("Statement"))
            kind = kind[..^9];
        if (kind.EndsWith("Expression"))
            kind = kind[..^10];
        if (kind.EndsWith("Clause"))
            kind = kind[..^6];
        if (kind.EndsWith("List"))
            kind = kind[..^4] + "s";
        return kind;
    }

    private static bool NeedsCData(string text)
    {
        return text.Contains('<') || text.Contains('>') || text.Contains('&') || text.Contains("]]>");
    }
}
