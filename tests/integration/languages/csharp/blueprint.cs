// Kitchen-sink C# blueprint fixture. Exercises every major construct so
// design-principle changes to the C# transform surface as visible snapshot diffs.

using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading.Tasks;
using static System.Math;

namespace Tractor.Fixtures.Traditional
{
    public delegate TResult Transformer<in T, out TResult>(T value);

    public enum Priority { Low, Medium, High }

    [Flags]
    public enum Trait : uint { None = 0, Fast = 1, Safe = 2, Cheap = 4 }

    public interface IRepository<T> where T : class, new()
    {
        T? Find(int id);
        Task<IReadOnlyList<T>> AllAsync();
        event EventHandler<T>? Added;
    }

    public readonly struct Point(double x, double y)
    {
        public double X { get; } = x;
        public double Y { get; } = y;
        public static Point operator +(Point a, Point b) => new(a.X + b.X, a.Y + b.Y);
        public static bool operator ==(Point a, Point b) => a.X == b.X && a.Y == b.Y;
        public static bool operator !=(Point a, Point b) => !(a == b);
        public override bool Equals(object? o) => o is Point p && p == this;
        public override int GetHashCode() => HashCode.Combine(X, Y);
    }

    public record Money(decimal Amount, string Currency);

    public record TaxedMoney(decimal Amount, string Currency, decimal TaxRate)
        : Money(Amount, Currency);

    [Serializable, Obsolete("use V2", false)]
    public abstract partial class EntityBase
    {
        public const int MaxNameLength = 128;
        protected internal static readonly DateTime Epoch = new(1970, 1, 1);
        private int _version;

        public int Id { get; init; }
        public string Name { get; set; } = "";
        public virtual Priority Priority { get; protected set; } = Priority.Low;

        public int this[string key] => key.Length;

        public event Action<EntityBase>? Changed;

        protected EntityBase() : this(0) { }
        protected EntityBase(int id) { Id = id; }

        ~EntityBase() { /* finalizer */ }

        public abstract string Describe();
        protected virtual void OnChanged() => Changed?.Invoke(this);
    }

    public sealed partial class Widget : EntityBase, IRepository<Widget>
    {
        public event EventHandler<Widget>? Added;

        public Widget() : base(0) { }
        public Widget(int id, string name) : base(id) { Name = name; }

        public override Priority Priority { get => base.Priority; protected set => base.Priority = value; }
        public override string Describe() => $"Widget#{Id}:{Name!}";

        public Widget? Find(int id) => id == Id ? this : null;

        public async Task<IReadOnlyList<Widget>> AllAsync()
        {
            await Task.Delay(1);
            return [this];
        }

        public static TResult Pipe<T, TResult>(T input, Transformer<T, TResult> fn) where TResult : notnull
            => fn(input);
    }

    public static class WidgetExtensions
    {
        public static string Shout(this Widget w) => w.Name.ToUpperInvariant() + "!";
    }

    internal static class Demo
    {
        public static async Task<int> RunAsync(Widget? maybe, IEnumerable<int> numbers)
        {
            var widget = maybe ?? new Widget(1, "alpha");
            var shouted = widget?.Shout() ?? "n/a";
            var label = widget is { Name.Length: > 0 } ? "named" : "blank";

            // switch expression + pattern matching
            string kind = widget switch
            {
                { Priority: Priority.High } => "hot",
                Widget w when w.Id > 100 => "big",
                _ => "plain",
            };

            // switch statement + is-type-pattern
            object token = widget!;
            switch (token)
            {
                case Widget w: Console.WriteLine(w.Describe()); break;
                case null: throw new InvalidOperationException();
                default: break;
            }
            if (token is Widget probe) Console.WriteLine(probe.Id);

            // LINQ query syntax + method syntax
            var evens = from n in numbers where n % 2 == 0 orderby n select n * n;
            var odds = numbers.Where(n => n % 2 != 0).Select(n => n + 1).ToList();

            // local function + lambda
            int Square(int x) => x * x;
            Func<int, int> cube = x => x * x * x;
            Func<int, int> blockLambda = x => { return x + 1; };
            Action<string> log = msg => Console.WriteLine(msg);
            log($"sum={evens.Sum()} cube={cube(3)} sq={Square(4)}");

            // loops
            for (var i = 0; i < 2; i++) { /* for */ }
            int j = 0; while (j++ < 2) { }
            int k = 0; do { k++; } while (k < 2);
            foreach (var n in odds) log(n.ToString());

            // try/catch/finally with when + using declaration + using statement
            using var scope = new System.IO.MemoryStream();
            try
            {
                using (var inner = new System.IO.MemoryStream()) { inner.WriteByte(1); }
                throw new InvalidOperationException("boom");
            }
            catch (InvalidOperationException ex) when (ex.Message.Length > 0) { log(ex.Message); }
            catch (Exception) { throw; }
            finally { scope.Flush(); }

            // tuples + deconstruction + target-typed new + collection expression
            (int count, string tag) pair = (odds.Count, "odd");
            var (cnt, tg) = pair;
            List<int> list = [1, 2, 3, .. odds];
            Dictionary<string, int> map = new() { ["a"] = 1, ["b"] = 2 };

            // string literals
            var plain = "hello";
            var verbatim = @"C:\path\file.txt";
            var interp = $"cnt={cnt} tag={tg}";
            var raw = """
                {
                    "kind": "raw"
                }
                """;

            return list.Count + map.Count + plain.Length + verbatim.Length + interp.Length + raw.Length
                 + (label == kind ? 0 : 1) + shouted.Length;
        }
    }
}

namespace Tractor.Fixtures.FileScoped;

using System;

file sealed class Hidden
{
    public static int Answer => 42;
}

public static class Entry
{
    public static int Get() => Hidden.Answer;
}

// Iter 20: new shapes — statements, expressions, patterns, literals.

/// <summary>cast + char + checked + with + patterns + array/stackalloc new</summary>
public static class Extras
{
    public static void Statements(object o)
    {
        // cast expression
        int n = (int)3.14;
        // char literal
        char ch = 'A';
        // checked statement + yield return (via iterator method below)
        checked { n = n + 1; }
        // fixed + unsafe handled in unsafe context (pointer omitted for clarity)
        // goto + labeled statement
        goto skip;
        int unreachable = 0;
        skip:
        _ = n;
        // lock statement
        lock (typeof(Entry)) { n++; }
        // with expression (record update)
        var m = new Money(1.0m, "USD");
        var m2 = m with { Currency = "EUR" };
        // anonymous object creation
        var anon = new { Name = "foo", Value = 42 };
        // array creation
        int[] arr = new int[] { 1, 2, 3 };
        int[] arr2 = new[] { 4, 5, 6 };
        // typeof + sizeof
        Type t = typeof(int);
        int sz = sizeof(int);
        // default expression
        int def = default(int);
        // throw expression (in null-coalescing context)
        string s = null;
        string s2 = s ?? throw new ArgumentNullException(nameof(s));
    }

    public static IEnumerable<int> Yields()
    {
        yield return 1;
        yield return 2;
    }

    public static string Patterns(object o)
    {
        return o switch
        {
            int i and > 0        => "positive int",
            int i and <= 0       => "non-positive int",
            string s or null     => "string or null",
            not string           => "not string",
            var x                => x!.ToString(),
        };
    }

    // Attributes, generic constraints, indexer, destructor.
    [Obsolete("legacy", false)]
    public class Container<T> where T : class, new()
    {
        public T Value { get; set; } = new T();
        public T this[int i] => Value;
        ~Container() { /* finalizer */ }
        public Container() : base() { }
    }

    // Delegate + event.
    public delegate void OnChanged(int newValue);
    public event OnChanged Changed;

    // Alias-qualified name + extern alias forms.
    private global::System.Int32 aliased = 0;

    // Operator overloads + conversion operator.
    public static Container<T> operator +(Container<T> a, Container<T> b) => a;
    public static implicit operator int(Container<T> c) => 0;

    // Event with explicit add/remove accessors.
    private event OnChanged _changed;
    public event OnChanged Explicit
    {
        add { _changed += value; }
        remove { _changed -= value; }
    }

    // Tuple deconstruction with parenthesized variable designation.
    public static void Deconstruct(out int x, out int y) { x = 1; y = 2; }
    public static (int, int) Pair() => (1, 2);

    // Positional pattern in switch arm.
    public static string Origin(object o) => o switch
    {
        (0, 0) => "origin",
        (var x, 0) => "x-axis",
        _ => "other",
    };

    // Attribute target specifier + global attribute target.
    [return: System.Diagnostics.CodeAnalysis.NotNull]
    public string Trim(string s) => s.Trim();
}

// Global / assembly-level attribute (file scope).
[assembly: System.Reflection.AssemblyDescription("blueprint")]

// Preprocessor regions and conditionals.
#region helpers
#if DEBUG
internal static class Debugging { }
#endif
#endregion
