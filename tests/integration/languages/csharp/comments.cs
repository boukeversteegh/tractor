// Application entry point
// Handles startup configuration
using System;

namespace Comments
{
    /// <summary>
    /// Sample class demonstrating comment patterns
    /// </summary>
    public class Demo
    {
        private int _count; // instance counter

        // Configuration settings
        // loaded from environment
        public string Config { get; set; }

        /* block comment */
        public void Run()
        {
            _count++;
        }
    }
}
