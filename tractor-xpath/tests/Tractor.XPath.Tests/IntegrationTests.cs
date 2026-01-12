using System.Diagnostics;

namespace Tractor.XPath.Tests;

public class IntegrationTests : IDisposable
{
    private readonly string _testDir;
    private readonly string _tractor;

    public IntegrationTests()
    {
        _testDir = Path.Combine(Path.GetTempPath(), $"codexpath-tests-{Guid.NewGuid():N}");
        Directory.CreateDirectory(_testDir);

        // Use the main tractor CLI (which delegates to parsers)
        _tractor = "tractor";
    }

    public void Dispose()
    {
        if (Directory.Exists(_testDir))
            Directory.Delete(_testDir, true);
    }

    private (int exitCode, string stdout, string stderr) Run(string args, string? stdin = null)
    {
        var psi = new ProcessStartInfo
        {
            FileName = _tractor,
            Arguments = args,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            RedirectStandardInput = stdin != null,
            UseShellExecute = false,
            WorkingDirectory = _testDir
        };

        using var process = Process.Start(psi)!;

        if (stdin != null)
        {
            process.StandardInput.Write(stdin);
            process.StandardInput.Close();
        }

        var stdout = process.StandardOutput.ReadToEnd();
        var stderr = process.StandardError.ReadToEnd();
        process.WaitForExit();

        return (process.ExitCode, stdout, stderr);
    }

    private string CreateTestFile(string name, string content)
    {
        var path = Path.Combine(_testDir, name);
        File.WriteAllText(path, content);
        return path;
    }

    // ========== XML Generation Tests ==========

    [Fact]
    public void GeneratesXmlForSimpleClass()
    {
        var file = CreateTestFile("test.cs", "public class Foo { }");
        var (exit, stdout, _) = Run($"\"{file}\" -f xml");

        Assert.Equal(0, exit);
        // /specs/tractor-parse/semantic-tree/transform-rules/rename-element.md: Rename Element
        Assert.Contains("<class", stdout);
        // /specs/tractor-parse/semantic-tree/transform-rules/rename-identifier.md: Rename Identifier to Name
        Assert.Contains("<name", stdout);
        Assert.Contains(">Foo<", stdout);
    }

    [Fact]
    public void GeneratesXmlForMethod()
    {
        var file = CreateTestFile("test.cs", "class C { void M() { } }");
        var (exit, stdout, _) = Run($"\"{file}\" -f xml");

        Assert.Equal(0, exit);
        // /specs/tractor-parse/semantic-tree/transform-rules/rename-element.md: Rename Element
        Assert.Contains("<method", stdout);
        // /specs/tractor-parse/semantic-tree/transform-rules/rename-identifier.md: Rename Identifier to Name
        Assert.Contains(">M<", stdout);
    }

    [Fact]
    public void XmlOutputIncludesLocations()
    {
        var file = CreateTestFile("test.cs", "class C { }");
        var (exit, stdout, _) = Run($"\"{file}\" -f xml");

        Assert.Equal(0, exit);
        // /specs/tractor-parse/semantic-tree/transform-rules/compact-location.md: Compact Location
        Assert.Contains("start=", stdout);
        Assert.Contains("end=", stdout);
    }

    [Fact]
    public void KeepLocationsIncludesLineAndColumnAttributes()
    {
        var file = CreateTestFile("test.cs", "class C { }");
        var (exit, stdout, _) = Run($"\"{file}\" --keep-locations -f xml");

        Assert.Equal(0, exit);
        // /specs/tractor-parse/semantic-tree/transform-rules/compact-location.md: Compact Location
        Assert.Contains("start=\"1:1\"", stdout);
        Assert.Contains("end=", stdout);
    }

    [Fact]
    public void IncludesModifiers()
    {
        var file = CreateTestFile("test.cs", "public static class Foo { }");
        var (exit, stdout, _) = Run($"\"{file}\" -f xml");

        Assert.Equal(0, exit);
        // /specs/tractor-parse/semantic-tree/transform-rules/lift-modifiers.md: Lift Modifiers
        Assert.Contains("<public", stdout);
        Assert.Contains("<static", stdout);
    }

    [Fact]
    public void MarksExtensionMethodParameter()
    {
        var file = CreateTestFile("test.cs", @"
            public static class Ext {
                public static void M(this string s) { }
            }");
        var (exit, stdout, _) = Run($"\"{file}\" -f xml");

        Assert.Equal(0, exit);
        // /specs/tractor-parse/semantic-tree/transform-rules/lift-modifiers.md: Lift Modifiers
        Assert.Contains("<this", stdout);
    }

    // ========== XPath Query Tests ==========

    [Fact]
    public void XPathQueryReturnsMethodNames()
    {
        var file = CreateTestFile("test.cs", "class C { void A() { } void B() { } }");
        // /specs/tractor-parse/semantic-tree/transform-rules/rename-identifier.md: Rename Identifier to Name
        var (exit, stdout, _) = Run($"\"{file}\" -x \"//method/name\"");

        Assert.Equal(0, exit);
        Assert.Contains("A", stdout);
        Assert.Contains("B", stdout);
    }

    [Fact]
    public void XPathQueryReturnsClassName()
    {
        var file = CreateTestFile("test.cs", "class MyClass { }");
        // /specs/tractor-parse/semantic-tree/transform-rules/rename-identifier.md: Rename Identifier to Name
        // Use -f value to get just the text content (default is lines which shows full source line)
        var (exit, stdout, _) = Run($"\"{file}\" -x \"//class/name\" -f value");

        Assert.Equal(0, exit);
        Assert.Equal("MyClass\n", stdout.Replace("\r\n", "\n"));
    }

    [Fact]
    public void XPathCountFunction()
    {
        var file = CreateTestFile("test.cs", "class C { void A() { } void B() { } void C() { } }");
        var (exit, stdout, _) = Run($"\"{file}\" -x \"count(//method)\"");

        Assert.Equal(0, exit);
        Assert.Equal("3\n", stdout.Replace("\r\n", "\n"));
    }

    [Fact]
    public void XPathWithPredicate()
    {
        var file = CreateTestFile("test.cs", @"
            class C {
                public void Pub() { }
                private void Priv() { }
            }");
        // /specs/tractor-parse/semantic-tree/transform-rules/lift-modifiers.md: Lift Modifiers
        var (exit, stdout, _) = Run($"\"{file}\" -x \"//method[public]/name\"");

        Assert.Equal(0, exit);
        Assert.Contains("Pub", stdout);
        Assert.DoesNotContain("Priv", stdout);
    }

    // ========== Output Format Tests ==========

    [Fact]
    public void FormatCount()
    {
        var file = CreateTestFile("test.cs", "class C { void A() { } void B() { } }");
        var (exit, stdout, _) = Run($"\"{file}\" -x \"//method\" -f count");

        Assert.Equal(0, exit);
        Assert.Equal("2\n", stdout.Replace("\r\n", "\n"));
    }

    [Fact]
    public void FormatJson()
    {
        var file = CreateTestFile("test.cs", "class C { void M() { } }");
        // /specs/tractor-parse/semantic-tree/transform-rules/rename-identifier.md: Rename Identifier to Name
        var (exit, stdout, _) = Run($"\"{file}\" -x \"//method/name\" -f json");

        Assert.Equal(0, exit);
        Assert.Contains("\"value\": \"M\"", stdout);
        Assert.Contains("\"line\":", stdout);
    }

    [Fact]
    public void FormatGccShowsFileAndLine()
    {
        var file = CreateTestFile("test.cs", "class C { void M() { } }");
        var (exit, stdout, _) = Run($"\"{file}\" -x \"//method\" -f gcc -m \"found method\"");

        Assert.Equal(0, exit);
        Assert.Contains("test.cs:", stdout);
        Assert.Contains("found method", stdout);
    }

    [Fact]
    public void FormatLinesShowsSourceSnippet()
    {
        var file = CreateTestFile("test.cs", @"
namespace Sample
{
    class C
    {
        void M()
        {
            if (true) Console.WriteLine();
        }
    }
}");
        var (exit, stdout, _) = Run($"\"{file}\" -x \"//if\"");

        Assert.Equal(0, exit);
        Assert.Contains("if (true) Console.WriteLine()", stdout.Replace("\r\n", "\n"));
    }

    [Fact]
    public void FormatLinesHandlesMultiLineNodes()
    {
        var file = CreateTestFile("test.cs", @"
namespace Sample
{
    class C
    {
        void M()
        {
            if (true) Console.WriteLine();
        }
    }
}");
        var (exit, stdout, _) = Run($"\"{file}\" -x \"//namespace\"");

        Assert.Equal(0, exit);
        var normalized = stdout.Replace("\r\n", "\n");
        Assert.Contains("namespace Sample", normalized);
        Assert.Contains("class C", normalized);
    }

    // ========== Expect Mode Tests ==========

    [Fact]
    public void ExpectNonePassesWhenNoMatches()
    {
        var file = CreateTestFile("test.cs", "class C { }");
        var (exit, _, _) = Run($"\"{file}\" -x \"//method\" -e none");

        Assert.Equal(0, exit);
    }

    [Fact]
    public void ExpectNoneFailsWhenMatches()
    {
        var file = CreateTestFile("test.cs", "class C { void M() { } }");
        var (exit, _, stderr) = Run($"\"{file}\" -x \"//method\" -e none");

        Assert.Equal(1, exit);
    }

    [Fact]
    public void ExpectSomePassesWhenMatches()
    {
        var file = CreateTestFile("test.cs", "class C { void M() { } }");
        var (exit, _, stderr) = Run($"\"{file}\" -x \"//method\" -e some");

        Assert.Equal(0, exit);
    }

    [Fact]
    public void ExpectSomeFailsWhenNoMatches()
    {
        var file = CreateTestFile("test.cs", "class C { }");
        var (exit, _, _) = Run($"\"{file}\" -x \"//method\" -e some");

        Assert.Equal(1, exit);
    }

    [Fact]
    public void ExpectCountPasses()
    {
        var file = CreateTestFile("test.cs", "class C { void A() { } void B() { } }");
        var (exit, _, _) = Run($"\"{file}\" -x \"//method\" -e 2");

        Assert.Equal(0, exit);
    }

    [Fact]
    public void ExpectCountFails()
    {
        var file = CreateTestFile("test.cs", "class C { void A() { } void B() { } }");
        var (exit, _, _) = Run($"\"{file}\" -x \"//method\" -e 3");

        Assert.Equal(1, exit);
    }

    // ========== Message Placeholder Tests ==========

    [Fact]
    public void MessagePlaceholderValue()
    {
        var file = CreateTestFile("test.cs", "class C { void MyMethod() { } }");
        // /specs/tractor-parse/semantic-tree/transform-rules/rename-identifier.md: Rename Identifier to Name
        var (exit, stdout, _) = Run($"\"{file}\" -x \"//method/name\" -f gcc -m \"name is {{value}}\"");

        Assert.Equal(0, exit);
        Assert.Contains("name is MyMethod", stdout);
    }

    [Fact]
    public void MessagePlaceholderXPath()
    {
        var file = CreateTestFile("test.cs", "class Outer { void Inner() { } }");
        // /specs/tractor-parse/semantic-tree/transform-rules/rename-identifier.md: Rename Identifier to Name
        var (exit, stdout, _) = Run($"\"{file}\" -x \"//method\" -f gcc -m \"method in {{ancestor::class/name}}\"");

        Assert.Equal(0, exit);
        Assert.Contains("method in Outer", stdout);
    }

    // ========== Stdin Tests ==========

    [Fact]
    public void ReadsFromStdin()
    {
        // /specs/tractor-parse/semantic-tree/transform-rules/rename-identifier.md: Rename Identifier to Name
        var (exit, stdout, _) = Run("--lang csharp -x \"//class/name\"", "class FromStdin { }");

        Assert.Equal(0, exit);
        Assert.Contains("FromStdin", stdout);
    }

    // ========== Help Tests ==========

    [Fact]
    public void HelpShowsUsage()
    {
        var (exit, stdout, _) = Run("--help");

        Assert.Equal(0, exit);
        Assert.Contains("Usage:", stdout);
        Assert.Contains("--xpath", stdout);
        Assert.Contains("--expect", stdout);
        Assert.Contains("--format", stdout);
    }
}
