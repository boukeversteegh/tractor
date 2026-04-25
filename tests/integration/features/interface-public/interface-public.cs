// Interface members without an explicit access modifier default to
// <public/>. Matches Java behavior — C# semantics agree.

interface IShape
{
    double Area();                    // implicitly public
    double Perimeter();               // implicitly public
    string Name => "shape";           // implicitly public (expression-bodied property)
    public void Stroke();             // explicit — still <public/>
}
