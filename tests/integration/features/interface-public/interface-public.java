// Interface members without an explicit access modifier default to
// <public/>. This makes implicit visibility explicit — a simple
// //method[public] query hits every visible interface method.

interface Shape {
    double area();              // implicitly public
    double perimeter();         // implicitly public
    default String name() {     // implicitly public
        return "shape";
    }
    public void stroke();       // explicit — should still be <public/>
}
