using System.Xml.XPath;
using Tractor.XPath;

var files = new List<string>();
string? xpathExpr = null;
string? expectation = null;
string format = "lines";
string? message = null;
bool fromStdin = false;
bool stripLocationAttributes = true;
bool verbose = false;
string colorMode = "auto"; // auto, always, never

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

// Verbose output
if (verbose)
{
    Console.Error.WriteLine($"[verbose] files: {string.Join(", ", files)}");
    Console.Error.WriteLine($"[verbose] xpath: {xpathExpr ?? "(none)"}");
    Console.Error.WriteLine($"[verbose] format: {format}");
}

// Determine if we should use color
bool useColor = colorMode switch
{
    "always" => true,
    "never" => false,
    _ => !Console.IsOutputRedirected && Environment.GetEnvironmentVariable("NO_COLOR") == null
};

// Collect all matches across files
var allMatches = new List<Match>();
var filesToProcess = fromStdin
    ? new[] { ("stdin", Console.In.ReadToEnd()) }
    : files.SelectMany(QueryEngine.ExpandGlob).Select(f => (f, File.ReadAllText(f)));

foreach (var (filePath, code) in filesToProcess)
{
    var matches = QueryEngine.ProcessFile(filePath, code, xpathExpr, stripLocationAttributes, verbose);
    allMatches.AddRange(matches);
}

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
                var xml = match.Navigator.OuterXml;
                if (stripLocationAttributes && match.Navigator.NodeType == XPathNodeType.Element)
                    xml = QueryEngine.StripLocationMetadata(xml);
                Console.WriteLine(useColor ? OutputFormatter.ColorizeXml(xml) : xml);
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
            // When no xpath is specified, output is XML - colorize it
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
        CodeXTractor - Extract patterns from source code using XPath queries

        Usage: tractor <files...> [options]
               tractor --stdin [options]

        Options:
          -x, --xpath <expr>     XPath 2.0 query (without: outputs full XML AST)
          -e, --expect <value>   Expected result: none, some, or a number
          -f, --format <fmt>     Output format: lines, gcc, json, count, xml
          -m, --message <msg>    Custom message (supports {value}, {line}, {xpath})
          --keep-locations       Include start/end line+col attributes in XML output
          --color <mode>         Color output: auto (default), always, never
          --no-color             Disable colored output (same as --color never)
          -v, --verbose          Show parsed arguments (helps debug shell escaping)
          -h, --help             Show this help

        Element names (C#):
          Keywords (lowercase):  if, else, for, foreach, while, class, struct, etc.
          Structural (Pascal):   Method, Property, Field, Call, Block, Param, etc.

        Examples:
          # Output full XML AST
          tractor Program.cs

          # Find all method names
          tractor Program.cs -x "//Method/@name"

          # Find all classes
          tractor Program.cs -x "//class/@name"

          # Find if statements inside methods
          tractor Program.cs -x "//Method//if"

          # CI check - fail if any SQL missing semicolons
          tractor "Migrations/*.cs" -x "//String[not(ends-with(@textValue, ';'))]" \
              --expect none --format gcc --message "SQL must end with semicolon"
        """);
}
