using CodeXPath;

var files = new List<string>();
string? xpathExpr = null;
string? expectation = null;
string format = "lines";
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
    : files.SelectMany(QueryEngine.ExpandGlob).Select(f => (f, File.ReadAllText(f)));

foreach (var (filePath, code) in filesToProcess)
{
    var matches = QueryEngine.ProcessFile(filePath, code, xpathExpr);
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
            Console.WriteLine(match.Navigator.OuterXml);
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

void PrintHelp()
{
    Console.WriteLine("""
        Usage: codexpath <files...> [options]
               codexpath --stdin [options]

        Options:
          -x, --xpath <expr>     XPath 2.0 query (without: outputs full XML)
          -e, --expect <value>   Expected result: none, some, or a number
          -f, --format <fmt>     Output format: lines, gcc, json, count
          -m, --message <msg>    Custom message (supports {value}, {line}, {xpath})
          -h, --help             Show this help

        Element names:
          Keywords (lowercase):  if, else, for, foreach, while, class, struct, etc.
          Structural (Pascal):   Method, Property, Field, Call, Block, Param, etc.

        Examples:
          # Output full XML AST
          codexpath Program.cs

          # Find all method names
          codexpath Program.cs -x "//Method/@name"

          # Find all classes
          codexpath Program.cs -x "//class/@name"

          # Find if statements inside methods
          codexpath Program.cs -x "//Method//if"

          # CI check - fail if any SQL missing semicolons
          codexpath "Migrations/*.cs" -x "//String[not(ends-with(@textValue, ';'))]" \
              --expect none --format gcc --message "SQL must end with semicolon"
        """);
}
