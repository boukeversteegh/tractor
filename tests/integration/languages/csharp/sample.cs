// Simple C# example
using System;

namespace SampleApp
{
    public class Sample
    {
        private readonly int _value;

        public Sample(int value)
        {
            _value = value;
        }

        public static int Add(int a, int b)
        {
            return a + b;
        }

        internal void Log(string message)
        {
            Console.WriteLine(message);
        }

        protected virtual string Format()
        {
            return _value.ToString();
        }

        static void Main()
        {
            int result = Add(5, 3);
            Console.WriteLine($"Result: {result}");
        }
    }

    interface IService
    {
        void Execute();
    }

    enum Status { Active, Inactive }

    struct Point
    {
        public int X;
        public int Y;
    }
}
