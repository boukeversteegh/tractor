// Simple Go example
package main

import "fmt"

func add(a int, b int) int {
	return a + b
}

func main() {
	result := add(5, 3)
	fmt.Printf("Result: %d\n", result)
}

// Exported function
func PublicHelper() string {
	return "hello"
}

type Point struct {
	X int
	Y int
}

type Shape interface {
	Area() float64
}
