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

echo "result: " . $closure(5) . "\n";
