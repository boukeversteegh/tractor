using System.Xml.XPath;
using Tractor.XPath;

string? xpathExpr = null;
string? expectation = null;
string format = "lines";
string? message = null;
bool stripLocationAttributes = true;
bool verbose = false;
string colorMode = "auto";

for (int i = 0; i < args.Length; i++)
{
    switch (args[i])
    {
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
        case "--keep-locations":
            stripLocationAttributes = false;
            break;
        case "--verbose" or "-v":
            verbose = true;
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
        default:
            break;
    }
}

// Read XML from stdin
var xml = Console.In.ReadToEnd();

if (string.IsNullOrWhiteSpace(xml))
{
    Console.Error.WriteLine("error: no XML input received on stdin");
    Console.Error.WriteLine("Usage: tractor-parse file.rs | tractor-xpath -x \"//query\"");
    return 1;
}

if (verbose)
{
    Console.Error.WriteLine($"[verbose] xpath: {xpathExpr ?? "(none)"}");
    Console.Error.WriteLine($"[verbose] format: {format}");
    Console.Error.WriteLine($"[verbose] xml length: {xml.Length}");
}

// Determine if we should use color
bool useColor = colorMode switch
{
    "always" => true,
    "never" => false,
    _ => !Console.IsOutputRedirected && Environment.GetEnvironmentVariable("NO_COLOR") == null
};

// Process XML with XPath
var allMatches = QueryEngine.ProcessXml(xml, xpathExpr, stripLocationAttributes, verbose);

// Output results
int matchCount = allMatches.Count;

switch (format)
{
    case "count":
        Console.WriteLine(matchCount);
        break;
    case "json":
        OutputFormatter.OutputJson(allMatches, message);
        break;
    case "gcc":
        OutputFormatter.OutputGcc(allMatches, message);
        break;
    case "xml":
        foreach (var match in allMatches)
        {
            if (match.Navigator != null)
            {
                var outputXml = match.Navigator.OuterXml;
                if (stripLocationAttributes && match.Navigator.NodeType == XPathNodeType.Element)
                    outputXml = QueryEngine.StripLocationMetadata(outputXml);
                Console.WriteLine(useColor ? OutputFormatter.ColorizeXml(outputXml) : outputXml);
            }
            else
            {
                var output = useColor ? OutputFormatter.ColorizeXml(match.Value) : match.Value;
                Console.WriteLine(output);
            }
        }
        break;
    case "lines":
    default:
        foreach (var match in allMatches)
        {
            var output = (xpathExpr == null && useColor)
                ? OutputFormatter.ColorizeXml(match.Value)
                : match.Value;
            Console.WriteLine(output);
        }
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

void PrintHelp()
{
    Console.WriteLine("""
        tractor-xpath - XPath 2.0 query engine for XML AST (reads from stdin)

        Usage: tractor-parse <files> | tractor-xpath -x <query> [options]
               cat file.xml | tractor-xpath -x <query> [options]

        Options:
          -x, --xpath <expr>     XPath 2.0 query (without: outputs full XML)
          -e, --expect <value>   Expected result: none, some, or a number
          -f, --format <fmt>     Output format: lines, gcc, json, count, xml
          -m, --message <msg>    Custom message (supports {value}, {line}, {xpath})
          --keep-locations       Include startLine/startCol/endLine/endCol attributes
          --color <mode>         Color output: auto (default), always, never
          --no-color             Disable colored output
          -v, --verbose          Show verbose output
          -h, --help             Show this help

        Examples:
          # Query Rust functions
          tractor-parse src/*.rs | tractor-xpath -x "//function_item/identifier"

          # Query Python from stdin
          echo "def foo(): pass" | tractor-parse - --lang python | tractor-xpath -x "//function_definition"

          # CI check - expect no matches
          tractor-parse *.cs | tractor-xpath -x "//TODO" --expect none -f gcc

          # Output as XML with colors
          tractor-parse main.go | tractor-xpath -x "//function_declaration" -f xml
        """);
}
