using System.Diagnostics;

namespace CodeXTractor.Tests;

public class IntegrationTests : IDisposable
{
    private readonly string _testDir;
    private readonly string _tractor;

    public IntegrationTests()
    {
        _testDir = Path.Combine(Path.GetTempPath(), $"codexpath-tests-{Guid.NewGuid():N}");
        Directory.CreateDirectory(_testDir);

        // Use the global tool
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
        var (exit, stdout, _) = Run($"\"{file}\"");

        Assert.Equal(0, exit);
        Assert.Contains("<class", stdout);
        Assert.Contains("name=\"Foo\"", stdout);
    }

    [Fact]
    public void GeneratesXmlForMethod()
    {
        var file = CreateTestFile("test.cs", "class C { void M() { } }");
        var (exit, stdout, _) = Run($"\"{file}\"");

        Assert.Equal(0, exit);
        Assert.Contains("<Method", stdout);
        Assert.Contains("name=\"M\"", stdout);
    }

    [Fact]
    public void StripsLineAndColumnAttributesByDefault()
    {
        var file = CreateTestFile("test.cs", "class C { }");
        var (exit, stdout, _) = Run($"\"{file}\"");

        Assert.Equal(0, exit);
        Assert.DoesNotContain("startLine=", stdout);
        Assert.DoesNotContain("startCol=", stdout);
        Assert.DoesNotContain("endLine=", stdout);
        Assert.DoesNotContain("endCol=", stdout);
    }

    [Fact]
    public void KeepLocationsIncludesLineAndColumnAttributes()
    {
        var file = CreateTestFile("test.cs", "class C { }");
        var (exit, stdout, _) = Run($"\"{file}\" --keep-locations");

        Assert.Equal(0, exit);
        Assert.Contains("startLine=\"1\"", stdout);
        Assert.Contains("startCol=\"1\"", stdout);
        Assert.Contains("endLine=", stdout);
        Assert.Contains("endCol=", stdout);
    }

    [Fact]
    public void IncludesModifiersAttribute()
    {
        var file = CreateTestFile("test.cs", "public static class Foo { }");
        var (exit, stdout, _) = Run($"\"{file}\"");

        Assert.Equal(0, exit);
        Assert.Contains("modifiers=\"public static\"", stdout);
    }

    [Fact]
    public void MarksExtensionMethodParameter()
    {
        var file = CreateTestFile("test.cs", @"
            public static class Ext {
                public static void M(this string s) { }
            }");
        var (exit, stdout, _) = Run($"\"{file}\"");

        Assert.Equal(0, exit);
        Assert.Contains("this=\"true\"", stdout);
    }

    // ========== XPath Query Tests ==========

    [Fact]
    public void XPathQueryReturnsMethodNames()
    {
        var file = CreateTestFile("test.cs", "class C { void A() { } void B() { } }");
        var (exit, stdout, _) = Run($"\"{file}\" -x \"//Method/@name\"");

        Assert.Equal(0, exit);
        Assert.Contains("A", stdout);
        Assert.Contains("B", stdout);
    }

    [Fact]
    public void XPathQueryReturnsClassName()
    {
        var file = CreateTestFile("test.cs", "class MyClass { }");
        var (exit, stdout, _) = Run($"\"{file}\" -x \"//class/@name\"");

        Assert.Equal(0, exit);
        Assert.Equal("MyClass\n", stdout.Replace("\r\n", "\n"));
    }

    [Fact]
    public void XPathCountFunction()
    {
        var file = CreateTestFile("test.cs", "class C { void A() { } void B() { } void C() { } }");
        var (exit, stdout, _) = Run($"\"{file}\" -x \"count(//Method)\"");

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
        var (exit, stdout, _) = Run($"\"{file}\" -x \"//Method[@modifiers='public']/@name\"");

        Assert.Equal(0, exit);
        Assert.Contains("Pub", stdout);
        Assert.DoesNotContain("Priv", stdout);
    }

    // ========== Output Format Tests ==========

    [Fact]
    public void FormatCount()
    {
        var file = CreateTestFile("test.cs", "class C { void A() { } void B() { } }");
        var (exit, stdout, _) = Run($"\"{file}\" -x \"//Method\" -f count");

        Assert.Equal(0, exit);
        Assert.Equal("2\n", stdout.Replace("\r\n", "\n"));
    }

    [Fact]
    public void FormatJson()
    {
        var file = CreateTestFile("test.cs", "class C { void M() { } }");
        var (exit, stdout, _) = Run($"\"{file}\" -x \"//Method/@name\" -f json");

        Assert.Equal(0, exit);
        Assert.Contains("\"value\": \"M\"", stdout);
        Assert.Contains("\"line\":", stdout);
    }

    [Fact]
    public void FormatGccShowsFileAndLine()
    {
        var file = CreateTestFile("test.cs", "class C { void M() { } }");
        var (exit, stdout, _) = Run($"\"{file}\" -x \"//Method\" -f gcc -m \"found method\"");

        Assert.Equal(0, exit);
        Assert.Contains("test.cs:", stdout);
        Assert.Contains(": error: found method", stdout);
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
        var (exit, _, _) = Run($"\"{file}\" -x \"//Method\" -e none");

        Assert.Equal(0, exit);
    }

    [Fact]
    public void ExpectNoneFailsWhenMatches()
    {
        var file = CreateTestFile("test.cs", "class C { void M() { } }");
        var (exit, _, stderr) = Run($"\"{file}\" -x \"//Method\" -e none");

        Assert.Equal(1, exit);
    }

    [Fact]
    public void ExpectSomePassesWhenMatches()
    {
        var file = CreateTestFile("test.cs", "class C { void M() { } }");
        var (exit, _, _) = Run($"\"{file}\" -x \"//Method\" -e some");

        Assert.Equal(0, exit);
    }

    [Fact]
    public void ExpectSomeFailsWhenNoMatches()
    {
        var file = CreateTestFile("test.cs", "class C { }");
        var (exit, _, _) = Run($"\"{file}\" -x \"//Method\" -e some");

        Assert.Equal(1, exit);
    }

    [Fact]
    public void ExpectCountPasses()
    {
        var file = CreateTestFile("test.cs", "class C { void A() { } void B() { } }");
        var (exit, _, _) = Run($"\"{file}\" -x \"//Method\" -e 2");

        Assert.Equal(0, exit);
    }

    [Fact]
    public void ExpectCountFails()
    {
        var file = CreateTestFile("test.cs", "class C { void A() { } void B() { } }");
        var (exit, _, _) = Run($"\"{file}\" -x \"//Method\" -e 3");

        Assert.Equal(1, exit);
    }

    // ========== Message Placeholder Tests ==========

    [Fact]
    public void MessagePlaceholderValue()
    {
        var file = CreateTestFile("test.cs", "class C { void MyMethod() { } }");
        var (exit, stdout, _) = Run($"\"{file}\" -x \"//Method/@name\" -f gcc -m \"name is {{value}}\"");

        Assert.Equal(0, exit);
        Assert.Contains("name is MyMethod", stdout);
    }

    [Fact]
    public void MessagePlaceholderXPath()
    {
        var file = CreateTestFile("test.cs", "class Outer { void Inner() { } }");
        var (exit, stdout, _) = Run($"\"{file}\" -x \"//Method\" -f gcc -m \"method in {{ancestor::class/@name}}\"");

        Assert.Equal(0, exit);
        Assert.Contains("method in Outer", stdout);
    }

    // ========== Stdin Tests ==========

    [Fact]
    public void ReadsFromStdin()
    {
        var (exit, stdout, _) = Run("--stdin -x \"//class/@name\"", "class FromStdin { }");

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
