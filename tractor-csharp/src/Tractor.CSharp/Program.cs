using System.Xml;
using Microsoft.CodeAnalysis.CSharp;
using Tractor.CSharp;

var files = new List<string>();
bool listLanguages = false;
string colorMode = "auto";

for (int i = 0; i < args.Length; i++)
{
    switch (args[i])
    {
        case "--list-languages":
            listLanguages = true;
            break;
        case "--color":
            colorMode = args[++i];
            break;
        case "--no-color":
            colorMode = "never";
            break;
        case "--help" or "-h":
            PrintHelp();
            return 0;
        case "-":
            // Read source from stdin
            files.Add("-");
            break;
        default:
            if (!args[i].StartsWith("-"))
                files.Add(args[i]);
            break;
    }
}

if (listLanguages)
{
    Console.WriteLine("Supported languages (1):");
    Console.WriteLine("  csharp     (.cs)");
    return 0;
}

// If no files provided as args, read file paths from stdin
if (files.Count == 0 && !Console.IsInputRedirected)
{
    PrintHelp();
    return 1;
}

if (files.Count == 0)
{
    // Read file paths from stdin
    string? line;
    while ((line = Console.ReadLine()) != null)
    {
        var path = line.Trim();
        if (!string.IsNullOrEmpty(path))
            files.Add(path);
    }
}

if (files.Count == 0)
{
    Console.Error.WriteLine("error: no files provided");
    return 1;
}

// Determine if we should use color
bool useColor = colorMode switch
{
    "always" => true,
    "never" => false,
    _ => !Console.IsOutputRedirected && Environment.GetEnvironmentVariable("NO_COLOR") == null
};

Colors.UseColor = useColor;

// Parse all files first
var parsedFiles = new List<(string path, Microsoft.CodeAnalysis.SyntaxNode root)>();

foreach (var file in files)
{
    try
    {
        string code;
        string filePath;

        if (file == "-")
        {
            code = Console.In.ReadToEnd();
            filePath = "<stdin>";
        }
        else
        {
            code = File.ReadAllText(file);
            filePath = file;
        }

        var tree = CSharpSyntaxTree.ParseText(code);
        var root = tree.GetRoot();
        parsedFiles.Add((filePath, root));
    }
    catch (Exception ex)
    {
        Console.Error.WriteLine($"error: failed to parse {file}: {ex.Message}");
    }
}

// Output XML (colored or plain)
if (useColor)
{
    ColoredXmlGenerator.WriteDocument(Console.Out, parsedFiles);
}
else
{
    var settings = new XmlWriterSettings
    {
        Indent = true,
        IndentChars = "  ",
        OmitXmlDeclaration = false
    };

    using var writer = XmlWriter.Create(Console.Out, settings);
    writer.WriteStartDocument();
    writer.WriteStartElement("Files");

    foreach (var (filePath, root) in parsedFiles)
    {
        writer.WriteStartElement("File");
        writer.WriteAttributeString("path", filePath);
        XmlGenerator.WriteNode(writer, root);
        writer.WriteEndElement();
    }

    writer.WriteEndElement();
    writer.WriteEndDocument();
}

return 0;

void PrintHelp()
{
    Console.WriteLine("""
        tractor-csharp - Roslyn-based C# parser (outputs XML AST)

        Usage: tractor-csharp <files...>
               tractor-csharp -              (read source from stdin)
               echo "file.cs" | tractor-csharp

        Options:
          --list-languages    Show supported languages
          --color <mode>      Color output: auto (default), always, never
          --no-color          Disable colored output
          -h, --help          Show this help

        Note: This parser uses Roslyn for high-fidelity C# AST.
              For other languages, use tractor-parse (TreeSitter).

        Examples:
          # Parse C# files to XML
          tractor-csharp Program.cs

          # Parse and query with tractor-xpath
          tractor-csharp src/*.cs | tractor-xpath -x "//Method/@name"

          # Parse source from stdin
          echo "class Foo {}" | tractor-csharp -
        """);
}
