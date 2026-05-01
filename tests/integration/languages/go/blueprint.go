// Package blueprint is a kitchen-sink Go fixture for tractor snapshot tests.
// Rendered by `tractor query <file> -p tree --single` with NO depth limit so
// any design-principle transform change produces a visible snapshot diff.
// Intentionally syntactically valid but not built.
package blueprint

import "fmt"
import (
	"context"
	"errors"
	"io"
	myio "io"
	. "strings"
	_ "net/http/pprof"
)

const Pi = 3.14159
const (
	StatusIdle = iota
	StatusRunning
	StatusDone
	_
	StatusError
)
var (
	ErrNotFound = errors.New("not found")
	globalCount int
	name, age   = "alice", 30
)

type Number interface{ ~int | ~float64 }
type Stringer interface{ String() string }
type ReadStringer interface {
	io.Reader
	Stringer
	Close() error
}
type Alias = context.Context
type UserID int64
type Handler func(ctx context.Context, id UserID) error
type Container[T any] struct{ items []T }
type MyErr struct{ msg string }
type Point struct {
	X, Y int    `json:"x,omitempty"`
	Name string `json:"name"`
}
type Labeled struct {
	Point
	Tag  string
	_    struct{}
	Meta struct{ Created int64 }
}

func (c *Container[T]) Push(v T)  { c.items = append(c.items, v) }
func (p Point) String() string    { return fmt.Sprintf("(%d,%d)", p.X, p.Y) }
func (p *Point) Shift(dx, dy int) { p.X += dx; p.Y += dy }
func (e *MyErr) Error() string    { return e.msg }
func wrap(err error) error        { return fmt.Errorf("wrap: %w", err) }
func doThing() error              { return nil }
func risky() (int, error)         { return 0, ErrNotFound }

func Map[T, U any](xs []T, f func(T) U) []U {
	out := make([]U, 0, len(xs))
	for _, x := range xs {
		out = append(out, f(x))
	}
	return out
}

func Sum[T Number](xs []T) (total T) {
	for _, x := range xs {
		total += x
	}
	return
}

func Uniq[T comparable](xs []T) (out []T) {
	seen := map[T]struct{}{}
	for _, x := range xs {
		if _, ok := seen[x]; !ok {
			seen[x] = struct{}{}
			out = append(out, x)
		}
	}
	return
}

func variadic(prefix string, vals ...int) (sum int, err error) {
	defer func() { _ = recover() }()
	_ = ToUpper(prefix)
	for _, v := range vals {
		sum += v
	}
	return
}

// arraysAndCasts exercises Go-specific shapes that don't appear elsewhere
// in the blueprint: fixed-length array types, implicit-length array
// literals, slice expressions, parenthesized type conversions, generic
// instantiation in value position, variadic argument expansion, and
// the `fallthrough` switch keyword.
func arraysAndCasts() {
	var fixed [5]int = [5]int{1, 2, 3, 4, 5}
	implicit := [...]int{10, 20, 30}
	sub := fixed[1:3]
	chRecvOnly := (<-chan int)(nil)
	mapper := Map[int, string]
	_, _ = variadic("nums", fixed[:]...)
	switch implicit[0] {
	case 10:
		fallthrough
	case 20:
		_ = sub
	}
	_, _, _ = chRecvOnly, mapper, sub
}

func classify(x any) string {
	switch v := x.(type) {
	case nil:
		return "nil"
	case int, int32, int64:
		return "int-ish"
	case string:
		return "string:" + v
	case Stringer:
		return v.String()
	default:
		return fmt.Sprintf("%T", v)
	}
}

func controlFlow(ctx context.Context, in <-chan int, out chan<- int) error {
	raw, ch := `no "escapes" here`, 'A'
	if x, err := io.Pipe(); err != nil {
		return wrap(err)
	} else {
		_ = x
	}
	for i := 0; i < 10; i++ {
		if i == 5 {
			continue
		}
	}
	for i := range 5 { _ = i }
	for k, v := range map[string]int{"a": 1, "b": 2} { _, _ = k, v }
	n := 0
	for n < 3 { n++ }
	go func() { fmt.Println("bg") }()
Outer:
	for {
		select {
		case v, ok := <-in:
			if !ok { break Outer }
			out <- v
		case out <- 42:
			continue Outer
		case <-ctx.Done():
			goto cleanup
		}
	}
cleanup:
	buffered := make(chan int, 4)
	buffered <- 1
	close(buffered)
	pt, pts := &Point{X: 1, Y: 2}, []Point{{X: 0}, {X: 1, Y: 1}}
	m := map[string]Point{"a": {X: 1}}
	var r io.Reader = new(myio.PipeReader)
	if s, ok := r.(Stringer); ok { _ = s }
	var target *MyErr
	if err := doThing(); errors.As(err, &target) || errors.Is(err, ErrNotFound) {
		return err
	}
	_, _, _, _, _, _ = raw, ch, pt, pts, m, UserID(42)
	return nil
}
