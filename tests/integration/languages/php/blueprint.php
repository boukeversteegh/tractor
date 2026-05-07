<?php
// Blueprint fixture — exercises every major PHP construct so any
// transform change shows up as a visible snapshot diff. Adjust
// freely when real language features need to be represented.

declare(strict_types=1);

namespace App\Blueprint;

use App\Base;
use App\Logger as Log;
use App\{First, Second};

/**
 * Demo class with various PHP features.
 */
#[Attribute(Attribute::TARGET_CLASS)]
abstract class Demo extends Base implements Speaker, Countable
{
    use Loggable;

    public const MAX = 100;
    private const DEFAULT_NAME = "anonymous";

    private string $name;
    public readonly int $count;
    protected ?string $label = null;
    public static int $registry = 0;

    public function __construct(string $name, int $count = 0)
    {
        $this->name = $name;
        $this->count = $count;
    }

    abstract public function render(): string;

    final public static function create(string $name): static
    {
        return new static($name);
    }

    public function greet(?string $who = null, string ...$extras): string
    {
        $target = $who ?? $this->name;
        if ($this->count > 10) {
            return "hello crowd, {$target}";
        } elseif ($this->count > 0) {
            return "hello again, {$target}";
        } else {
            return "hello, {$target}";
        }
    }

    public function describe(int $n): string
    {
        return $n > 0 ? "positive" : ($n < 0 ? "negative" : "zero");
    }

    public function categorize(mixed $value): string
    {
        return match (true) {
            is_int($value) => "int",
            is_string($value) && strlen($value) > 0 => "non-empty string",
            is_array($value) => "array",
            default => "other",
        };
    }

    public function process(iterable $items): array
    {
        $results = [];
        foreach ($items as $key => $item) {
            if ($item === null) {
                continue;
            }
            try {
                $results[$key] = $this->transform($item);
            } catch (\Throwable $e) {
                continue;
            }
        }
        return $results;
    }

    private function transform(mixed $item): mixed
    {
        return $item;
    }
}

interface Speaker
{
    public function greet(?string $who): string;
}

interface Countable
{
    public function count(): int;
}

trait Loggable
{
    protected function log(string $msg): void
    {
        echo "[log] {$msg}\n";
    }
}

enum Status: string
{
    case Active = "active";
    case Inactive = "inactive";
    case Pending = "pending";

    public function label(): string
    {
        return match ($this) {
            Status::Active => "✓",
            Status::Inactive => "×",
            Status::Pending => "…",
        };
    }
}

function factory(callable $fn, mixed ...$args): mixed
{
    return $fn(...$args);
}

$pipeline = fn(int $x): int => $x * 2;
$doubled = array_map($pipeline, [1, 2, 3]);

$closure = function (int $x) use ($pipeline): int {
    return $pipeline($x) + 1;
};

// Iter 17 shapes: nullsafe, anonymous class, intersection / DNF / bottom
// type, function-static / global vars, dynamic var name, property
// promotion, cast type, list-destructure, clone / unset / unique-label,
// shell command, error suppression, augmented assignment, sequence /
// reference assignment, first-class callable, heredoc / nowdoc.

class PromotedDemo
{
    public function __construct(
        public readonly string $name,
        protected int $count = 0,
    ) {}
}

interface Stringable { public function __toString(): string; }
interface Countable2 { public function size(): int; }

function shapes(?PromotedDemo $thing, mixed $value): mixed
{
    // Nullsafe member access + call.
    $name = $thing?->name;
    $size = $thing?->describe();

    // Anonymous class.
    $anon = new class implements Stringable {
        public function __toString(): string { return "anon"; }
    };

    // First-class callable + augmented assignment.
    $maker = strtoupper(...);
    $count = 0;
    $count += 5;
    $count *= 2;

    // Reference assignment + by-ref foreach.
    $items = [1, 2, 3];
    $alias =& $items[0];
    foreach ($items as &$x) { $x *= 10; }
    unset($x);

    // List destructure.
    [$first, $second] = $items;
    list($third, , $fifth) = [10, 20, 30, 40, 50];

    // Function-local static + global.
    static $cached = null;
    global $REGISTRY;

    // Dynamic variable name + cast type.
    $varname = "thing";
    $$varname = 42;
    $cast = (int) $value;

    // Clone, error suppression, shell.
    $copy = clone $anon;
    $maybe = @file_get_contents("/missing");
    $output = `echo hi`;

    // Heredoc / nowdoc.
    $here = <<<EOT
        hello $name
        line two
        EOT;
    $now = <<<'EOT'
        no interpolation: $not_a_var
        EOT;

    // Sequence in a for header.
    for ($i = 0, $j = 10; $i < $j; $i++, $j--) { /* ... */ }

    // Goto + named label.
    goto end;
    end:

    // Match-arm; intersection / DNF / bottom types appear in
    // declarative positions (parameters / returns), not in
    // `instanceof` expressions, so the type-shape exercises live in
    // sibling functions below.
    return match (true) {
        $value instanceof Stringable => 1,
        $value instanceof Countable2 => 2,
        default => 0,
    };
}

// Intersection type in parameter position.
function require_both(Stringable&Countable2 $thing): int { return $thing->size(); }

// DNF type in parameter position.
function dnf_demo((Stringable&Countable2)|null $thing): bool { return $thing !== null; }

// Bottom type (`never`) in return position.
function never_returns(): never { throw new \RuntimeException("boom"); }

echo "result: " . $closure(5) . "\n";
