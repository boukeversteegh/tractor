using System.Diagnostics;
using System.Xml.XPath;
using Tractor.XPath;

// Parse arguments
var files = new List<string>();
string? xpathExpr = null;
string? expectation = null;
string format = "lines";
string? message = null;
bool stripLocationAttributes = true;
bool verbose = false;
bool debug = false;
string colorMode = "auto";
string? tractorParsePath = null;
string? tractorCsharpPath = null;
string? stdinLang = null;

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
        case "--debug":
            debug = true;
            break;
        case "--lang" or "-l":
            stdinLang = args[++i];
            break;
        case "--color":
            colorMode = args[++i];
            break;
        case "--no-color":
            colorMode = "never";
            break;
        case "--parser":
            tractorParsePath = args[++i];
            break;
        case "--csharp-parser":
            tractorCsharpPath = args[++i];
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

// Handle stdin source input when --lang is specified
string? stdinXml = null;
if (stdinLang != null && files.Count == 0 && Console.IsInputRedirected)
{
    var source = Console.In.ReadToEnd();
    if (string.IsNullOrWhiteSpace(source))
    {
        Console.Error.WriteLine("error: no source code received on stdin");
        return 1;
    }

    // Call appropriate parser based on language
    var parserPath = stdinLang == "csharp" || stdinLang == "cs"
        ? FindParser("tractor-csharp", tractorCsharpPath)
        : FindParser("tractor-parse", tractorParsePath);

    if (parserPath == null)
    {
        Console.Error.WriteLine($"error: parser not found for language '{stdinLang}'");
        return 1;
    }

    var psi = new ProcessStartInfo
    {
        FileName = parserPath,
        RedirectStandardInput = true,
        RedirectStandardOutput = true,
        RedirectStandardError = true,
        UseShellExecute = false,
        CreateNoWindow = true
    };

    // For tractor-parse, add --lang argument
    if (stdinLang != "csharp" && stdinLang != "cs")
    {
        psi.ArgumentList.Add("--lang");
        psi.ArgumentList.Add(stdinLang);
    }
    // For tractor-csharp, use "-" to indicate stdin
    else
    {
        psi.ArgumentList.Add("-");
    }

    using var process = Process.Start(psi);
    if (process == null)
    {
        Console.Error.WriteLine("error: failed to start parser");
        return 1;
    }

    process.StandardInput.Write(source);
    process.StandardInput.Close();

    stdinXml = process.StandardOutput.ReadToEnd();
    var stderr = process.StandardError.ReadToEnd();
    process.WaitForExit();

    if (process.ExitCode != 0)
    {
        Console.Error.WriteLine($"error: parser exited with code {process.ExitCode}");
        if (!string.IsNullOrEmpty(stderr))
            Console.Error.WriteLine(stderr);
        return 1;
    }

    if (verbose)
        Console.Error.WriteLine($"[verbose] parsed stdin as {stdinLang}");
}
else if (files.Count == 0 && stdinLang == null)
{
    PrintHelp();
    return 1;
}

// Expand globs and group by language
var allFiles = files.SelectMany(QueryEngine.ExpandGlob).ToList();
var csharpFiles = new List<string>();
var otherFiles = new List<string>();

foreach (var file in allFiles)
{
    var lang = DetectLanguage(file);
    if (lang == "csharp")
        csharpFiles.Add(file);
    else if (lang != "unknown")
        otherFiles.Add(file);
    else if (verbose)
        Console.Error.WriteLine($"[verbose] skipping unknown file type: {file}");
}

if (verbose)
{
    Console.Error.WriteLine($"[verbose] C# files: {csharpFiles.Count}");
    Console.Error.WriteLine($"[verbose] Other files: {otherFiles.Count}");
    Console.Error.WriteLine($"[verbose] xpath: {xpathExpr ?? "(none)"}");
}

// Determine if we should use color
bool useColor = colorMode switch
{
    "always" => true,
    "never" => false,
    _ => !Console.IsOutputRedirected && Environment.GetEnvironmentVariable("NO_COLOR") == null
};

// Process files in parallel using external parsers
var allMatches = new List<Match>();
var allXml = new List<string>(); // Store XML for debug mode
var lockObj = new object();

var tasks = new List<Task>();

// Process stdin XML if we have it
if (stdinXml != null)
{
    lock (lockObj)
    {
        allXml.Add(stdinXml);
    }
    var matches = QueryEngine.ProcessXml(stdinXml, xpathExpr, stripLocationAttributes, verbose);
    allMatches.AddRange(matches);
}

// Task 1: Process C# files via tractor-csharp
if (csharpFiles.Count > 0)
{
    tasks.Add(Task.Run(() =>
    {
        try
        {
            var parserPath = FindParser("tractor-csharp", tractorCsharpPath);
            if (parserPath == null)
            {
                Console.Error.WriteLine("error: tractor-csharp not found. Install it or use --csharp-parser to specify path.");
                return;
            }

            if (verbose)
                Console.Error.WriteLine($"[verbose] using C# parser: {parserPath}");

            var psi = new ProcessStartInfo
            {
                FileName = parserPath,
                RedirectStandardInput = true,
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                UseShellExecute = false,
                CreateNoWindow = true
            };

            using var process = Process.Start(psi);
            if (process == null)
            {
                Console.Error.WriteLine("error: failed to start tractor-csharp");
                return;
            }

            foreach (var file in csharpFiles)
                process.StandardInput.WriteLine(file);
            process.StandardInput.Close();

            var xml = process.StandardOutput.ReadToEnd();
            var stderr = process.StandardError.ReadToEnd();
            process.WaitForExit();

            if (!string.IsNullOrEmpty(stderr) && verbose)
                Console.Error.WriteLine($"[verbose] tractor-csharp stderr: {stderr}");

            if (process.ExitCode != 0)
            {
                Console.Error.WriteLine($"error: tractor-csharp exited with code {process.ExitCode}");
                if (!string.IsNullOrEmpty(stderr))
                    Console.Error.WriteLine(stderr);
                return;
            }

            lock (lockObj)
            {
                allXml.Add(xml);
            }
            var matches = QueryEngine.ProcessXml(xml, xpathExpr, stripLocationAttributes, verbose);
            lock (lockObj)
            {
                allMatches.AddRange(matches);
            }
        }
        catch (Exception ex)
        {
            Console.Error.WriteLine($"error: failed to process C# files: {ex.Message}");
        }
    }));
}

// Task 2: Process other files via tractor-parse
if (otherFiles.Count > 0)
{
    tasks.Add(Task.Run(() =>
    {
        try
        {
            var parserPath = FindParser("tractor-parse", tractorParsePath);
            if (parserPath == null)
            {
                Console.Error.WriteLine("error: tractor-parse not found. Install it or use --parser to specify path.");
                return;
            }

            if (verbose)
                Console.Error.WriteLine($"[verbose] using parser: {parserPath}");

            var psi = new ProcessStartInfo
            {
                FileName = parserPath,
                RedirectStandardInput = true,
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                UseShellExecute = false,
                CreateNoWindow = true
            };

            using var process = Process.Start(psi);
            if (process == null)
            {
                Console.Error.WriteLine("error: failed to start tractor-parse");
                return;
            }

            foreach (var file in otherFiles)
                process.StandardInput.WriteLine(file);
            process.StandardInput.Close();

            var xml = process.StandardOutput.ReadToEnd();
            var stderr = process.StandardError.ReadToEnd();
            process.WaitForExit();

            if (!string.IsNullOrEmpty(stderr) && verbose)
                Console.Error.WriteLine($"[verbose] tractor-parse stderr: {stderr}");

            if (process.ExitCode != 0)
            {
                Console.Error.WriteLine($"error: tractor-parse exited with code {process.ExitCode}");
                return;
            }

            lock (lockObj)
            {
                allXml.Add(xml);
            }
            var matches = QueryEngine.ProcessXml(xml, xpathExpr, stripLocationAttributes, verbose);
            lock (lockObj)
            {
                allMatches.AddRange(matches);
            }
        }
        catch (Exception ex)
        {
            Console.Error.WriteLine($"error: failed to process non-C# files: {ex.Message}");
        }
    }));
}

// Wait for all tasks
Task.WaitAll(tasks.ToArray());

// Debug mode: show XML with matches highlighted
if (debug)
{
    // Collect all matched node paths for highlighting
    var matchedPaths = new HashSet<string>();
    foreach (var match in allMatches)
    {
        if (match.Navigator != null)
        {
            // Build a simple identifier for the matched node based on position
            var nav = match.Navigator.Clone();
            var path = $"{nav.Name}@{match.File}:{match.Line}:{match.Column}";
            matchedPaths.Add(path);
        }
    }

    Console.Error.WriteLine($"[debug] {allMatches.Count} match(es) found");
    if (xpathExpr != null)
        Console.Error.WriteLine($"[debug] XPath: {xpathExpr}");
    Console.Error.WriteLine();

    // Output colorized XML with matches highlighted
    foreach (var xml in allXml)
    {
        var highlighted = OutputFormatter.ColorizeXmlWithHighlights(xml, allMatches, useColor);
        Console.WriteLine(highlighted);
    }

    if (allMatches.Count == 0 && xpathExpr != null)
    {
        Console.Error.WriteLine();
        Console.Error.WriteLine("[debug] No matches. Check the XML structure above to find the correct XPath.");
    }

    return allMatches.Count > 0 ? 0 : 1;
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

// Helper functions

string DetectLanguage(string path)
{
    var ext = Path.GetExtension(path).ToLowerInvariant();
    return ext switch
    {
        ".cs" => "csharp",
        ".rs" => "rust",
        ".js" or ".mjs" or ".cjs" or ".jsx" => "javascript",
        ".ts" or ".tsx" => "typescript",
        ".py" or ".pyw" or ".pyi" => "python",
        ".go" => "go",
        ".java" => "java",
        ".rb" or ".rake" or ".gemspec" => "ruby",
        ".cpp" or ".cc" or ".cxx" or ".hpp" or ".hxx" or ".hh" => "cpp",
        ".c" or ".h" => "c",
        ".json" => "json",
        ".html" or ".htm" => "html",
        ".css" => "css",
        ".sh" or ".bash" => "bash",
        ".yml" or ".yaml" => "yaml",
        ".php" => "php",
        ".scala" or ".sc" => "scala",
        ".lua" => "lua",
        ".hs" or ".lhs" => "haskell",
        ".ml" or ".mli" => "ocaml",
        ".r" => "r",
        ".jl" => "julia",
        _ => "unknown"
    };
}

string? FindParser(string name, string? explicitPath)
{
    // 1. Explicit path
    if (!string.IsNullOrEmpty(explicitPath) && File.Exists(explicitPath))
        return explicitPath;

    var exeName = OperatingSystem.IsWindows() ? $"{name}.exe" : name;

    // 2. Same directory as this executable
    var exeDir = AppContext.BaseDirectory;
    var candidate = Path.Combine(exeDir, exeName);
    if (File.Exists(candidate))
        return candidate;

    // 3. PATH
    var pathVar = Environment.GetEnvironmentVariable("PATH") ?? "";
    var paths = pathVar.Split(Path.PathSeparator);

    foreach (var dir in paths)
    {
        var exePath = Path.Combine(dir, exeName);
        if (File.Exists(exePath))
            return exePath;
    }

    return null;
}

void PrintHelp()
{
    Console.WriteLine("""
        tractor - Multi-language code query tool using XPath 2.0

        Usage: tractor <files...> [options]
               echo "source" | tractor --lang <language> [options]

        Options:
          -x, --xpath <expr>       XPath 2.0 query (without: outputs full XML AST)
          -l, --lang <language>    Parse source from stdin as this language
          --debug                  Show full XML with matches highlighted (for debugging XPath)
          -e, --expect <value>     Expected result: none, some, or a number
          -f, --format <fmt>       Output format: lines, gcc, json, count, xml
          -m, --message <msg>      Custom message (supports {value}, {line}, {xpath})
          --keep-locations         Include start/end line+col attributes in XML output
          --color <mode>           Color output: auto (default), always, never
          --no-color               Disable colored output
          --parser <path>          Path to tractor-parse (auto-detected)
          --csharp-parser <path>   Path to tractor-csharp (auto-detected)
          -v, --verbose            Show verbose output
          -h, --help               Show this help

        Supported languages (22):
          C# (via Roslyn):    cs, csharp
          Via TreeSitter:     rust, javascript, typescript, python, go, java, ruby,
                              cpp, c, json, html, css, bash, yaml, php, scala, lua,
                              haskell, ocaml, r, julia

        Examples:
          tractor "src/**/*.cs" -x "//Method/@name"
          tractor "src/**/*.rs" -x "//function_item/identifier"

          # Pipe source directly
          echo "fn main() {}" | tractor --lang rust -x "//function_item"

          # Debug mode: see XML structure and what matched
          echo "fn main() {}" | tractor --lang rust -x "//function_item" --debug

          # CI/linting: fail if TODO comments found
          tractor "src/**/*.cs" -x "//TODO" --expect none -f gcc

        Low-level tools:
          tractor-csharp    Roslyn C# parser (files → XML AST)
          tractor-parse     TreeSitter parser (files → XML AST)
          tractor-xpath     XPath 2.0 engine (XML stdin → query results)
        """);
}
