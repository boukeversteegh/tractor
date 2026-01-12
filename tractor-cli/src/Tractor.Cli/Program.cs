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
bool useRoslyn = false;
int? concurrency = null;

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
        case "--roslyn":
            useRoslyn = true;
            break;
        case "--concurrency" or "-c":
            concurrency = int.Parse(args[++i]);
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
string[]? stdinSourceLines = null;
if (stdinLang != null && files.Count == 0 && Console.IsInputRedirected)
{
    var source = Console.In.ReadToEnd();
    stdinSourceLines = source.Split('\n');
    if (string.IsNullOrWhiteSpace(source))
    {
        Console.Error.WriteLine("error: no source code received on stdin");
        return 1;
    }

    // Use Roslyn for C# when --roslyn flag is set, otherwise TreeSitter for all
    var isCsharp = stdinLang == "csharp" || stdinLang == "cs";
    var parserPath = (useRoslyn && isCsharp)
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

    // Add arguments based on parser type
    if (useRoslyn && isCsharp)
    {
        psi.ArgumentList.Add("-");
    }
    else
    {
        psi.ArgumentList.Add("--lang");
        psi.ArgumentList.Add(stdinLang);
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

// Expand globs and group files
var allFiles = files.SelectMany(QueryEngine.ExpandGlob).ToList();
var csharpFiles = new List<string>();
var otherFiles = new List<string>();

foreach (var file in allFiles)
{
    var lang = DetectLanguage(file);
    if (lang == "csharp" && useRoslyn)
        csharpFiles.Add(file);
    else if (lang != "unknown")
        otherFiles.Add(file);
    else if (verbose)
        Console.Error.WriteLine($"[verbose] skipping unknown file type: {file}");
}

if (verbose)
{
    if (useRoslyn)
        Console.Error.WriteLine($"[verbose] C# files (Roslyn): {csharpFiles.Count}");
    Console.Error.WriteLine($"[verbose] files (TreeSitter): {otherFiles.Count}");
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
    // Attach source lines to matches for --format lines support
    if (stdinSourceLines != null)
    {
        matches = matches.Select(m => m with { SourceLines = stdinSourceLines }).ToList();
    }
    allMatches.AddRange(matches);
}

// Task 1: Process C# files via tractor-csharp (Roslyn) when --roslyn is set
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
                Console.Error.WriteLine($"[verbose] using C# parser (Roslyn): {parserPath}");

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

// Task 2: Process all other files via tractor-parse (TreeSitter) in parallel batches
if (otherFiles.Count > 0)
{
    var parserPath = FindParser("tractor-parse", tractorParsePath);
    if (parserPath == null)
    {
        Console.Error.WriteLine("error: tractor-parse not found. Install it or use --parser to specify path.");
    }
    else
    {
        if (verbose)
            Console.Error.WriteLine($"[verbose] using parser (TreeSitter): {parserPath}");

        // Split files into batches for parallel processing
        int parallelism = concurrency ?? Math.Max(1, Environment.ProcessorCount / 2);
        int batchSize = Math.Max(1, (otherFiles.Count + parallelism - 1) / parallelism);
        var batches = otherFiles
            .Select((file, index) => (file, index))
            .GroupBy(x => x.index / batchSize)
            .Select(g => g.Select(x => x.file).ToList())
            .ToList();

        if (verbose)
            Console.Error.WriteLine($"[verbose] processing {otherFiles.Count} files in {batches.Count} parallel batches (batch size ~{batchSize})");

        foreach (var batch in batches)
        {
            var batchFiles = batch; // Capture for closure
            tasks.Add(Task.Run(() =>
            {
                try
                {
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

                    foreach (var file in batchFiles)
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
                    Console.Error.WriteLine($"error: failed to process batch: {ex.Message}");
                }
            }));
        }
    }
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
    case "value":
        foreach (var match in allMatches)
        {
            var output = (xpathExpr == null && useColor)
                ? OutputFormatter.ColorizeXml(match.Value)
                : match.Value;
            Console.WriteLine(output);
        }
        break;
    case "source":
        // Output exact source snippet using line:col positions
        var srcCache = new Dictionary<string, string[]>();
        foreach (var match in allMatches)
        {
            string[] srcLines;

            if (match.SourceLines.Length > 0)
            {
                srcLines = match.SourceLines;
            }
            else if (match.File != "<stdin>" && File.Exists(match.File))
            {
                if (!srcCache.TryGetValue(match.File, out srcLines!))
                {
                    srcLines = File.ReadAllLines(match.File);
                    srcCache[match.File] = srcLines;
                }
            }
            else
            {
                Console.WriteLine(match.Value);
                continue;
            }

            // Extract exact snippet using column positions
            if (srcLines.Length >= match.EndLine && match.Line > 0)
            {
                if (match.Line == match.EndLine)
                {
                    // Single line - extract from startCol to endCol
                    var line = srcLines[match.Line - 1];
                    var startIdx = Math.Min(match.Column - 1, line.Length);
                    var endIdx = Math.Min(match.EndColumn - 1, line.Length);
                    Console.WriteLine(line[startIdx..endIdx]);
                }
                else
                {
                    // Multi-line - first line from startCol, middle lines full, last line to endCol
                    var firstLine = srcLines[match.Line - 1];
                    var startIdx = Math.Min(match.Column - 1, firstLine.Length);
                    Console.WriteLine(firstLine[startIdx..].TrimEnd('\r'));

                    for (int i = match.Line + 1; i < match.EndLine; i++)
                    {
                        Console.WriteLine(srcLines[i - 1].TrimEnd('\r'));
                    }

                    var lastLine = srcLines[match.EndLine - 1];
                    var endIdx = Math.Min(match.EndColumn - 1, lastLine.Length);
                    Console.WriteLine(lastLine[..endIdx]);
                }
            }
            else
            {
                Console.WriteLine(match.Value);
            }
        }
        break;
    case "lines":
    default:
        // Output full source code lines from the original file
        var fileCache = new Dictionary<string, string[]>();
        foreach (var match in allMatches)
        {
            string[] sourceLines;

            // Get source lines - from match, cache, or read file
            if (match.SourceLines.Length > 0)
            {
                sourceLines = match.SourceLines;
            }
            else if (match.File != "<stdin>" && File.Exists(match.File))
            {
                if (!fileCache.TryGetValue(match.File, out sourceLines!))
                {
                    sourceLines = File.ReadAllLines(match.File);
                    fileCache[match.File] = sourceLines;
                }
            }
            else
            {
                // No source available - fall back to value (colorize if XML output)
                var output = (xpathExpr == null && useColor)
                    ? OutputFormatter.ColorizeXml(match.Value)
                    : match.Value;
                Console.WriteLine(output);
                continue;
            }

            // Output source lines from Line to EndLine
            if (sourceLines.Length >= match.EndLine && match.Line > 0)
            {
                for (int i = match.Line; i <= match.EndLine; i++)
                {
                    Console.WriteLine(sourceLines[i - 1].TrimEnd('\r'));
                }
            }
            else
            {
                var output = (xpathExpr == null && useColor)
                    ? OutputFormatter.ColorizeXml(match.Value)
                    : match.Value;
                Console.WriteLine(output);
            }
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
          -f, --format <fmt>       Output format: lines (default), source, value, gcc, json, count, xml
                                     lines: full source lines containing the match
                                     source: exact matched source (column-precise)
                                     value: XML text content of matched node
          -m, --message <msg>      Custom message (supports {value}, {line}, {xpath})
          --keep-locations         Include start/end line+col attributes in XML output
          --color <mode>           Color output: auto (default), always, never
          --no-color               Disable colored output
          -c, --concurrency <n>    Number of parallel batches (default: CPU count / 2)
          --roslyn                 Use Roslyn parser for C# instead of TreeSitter
          --parser <path>          Path to tractor-parse (auto-detected)
          --csharp-parser <path>   Path to tractor-csharp (auto-detected)
          -v, --verbose            Show verbose output
          -h, --help               Show this help

        Supported languages (22):
          Via TreeSitter:     csharp, rust, javascript, typescript, python, go, java,
                              ruby, cpp, c, json, html, css, bash, yaml, php, scala,
                              lua, haskell, ocaml, r, julia
          Legacy (--roslyn):  csharp (via Roslyn, different AST structure)

        Examples:
          tractor "src/**/*.cs" -x "//method/name"
          tractor "src/**/*.cs" -x "//class[public][static]/method[not(params/param[this])]/name"

          # Pipe source directly
          echo "public class Foo { }" | tractor --lang csharp -x "//class/name"

          # Debug mode: see XML structure and what matched
          echo "public class Foo { }" | tractor --lang csharp -x "//class" --debug

          # CI/linting: fail if methods without OrderBy in Repository
          tractor "src/**/*.cs" -x "//class[name[contains(.,'Repository')]]/method[not(contains(.,'OrderBy'))]" --expect none -f gcc

        Low-level tools:
          tractor-parse     TreeSitter parser (files → XML AST) - all languages
          tractor-csharp    Roslyn C# parser (files → XML AST) - legacy, use --roslyn
          tractor-xpath     XPath 2.0 engine (XML stdin → query results)
        """);
}
