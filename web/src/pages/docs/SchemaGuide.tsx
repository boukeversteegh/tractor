import { Link } from 'react-router-dom';
import { DocLayout } from '../../components/DocLayout';
import { CodeBlock, OutputBlock } from '../../components/CodeBlock';

export function SchemaGuide() {
  return (
    <DocLayout>
      <h1>Exploring with Schema</h1>
      <p className="doc-lead">
        The schema view shows you what element types exist in your code — a map of what's available to query.
      </p>

      <h2>Why Schema?</h2>
      <p>
        When you work with a new codebase or language, the first question is: "what can I query?". The schema view answers that by showing element types, their nesting, and how often they appear.
      </p>

      <h2>Basic Usage</h2>
      <p>
        Add <code>-v schema</code> to any tractor command:
      </p>
      <CodeBlock
        language="bash"
        code={`echo 'public class Greeter {\n    public string Greet(string name) {\n        return "Hello, " + name;\n    }\n    public int Add(int a, int b) {\n        return a + b;\n    }\n}' | tractor -l csharp -v schema`}
      />
      <OutputBlock output={`Files
└─ File
   └─ unit
      └─ class  class
         ├─ public
         ├─ name  Greeter
         └─ body  {…}
            └─ … (21 children)

(use -d to increase depth, or -x to query specific elements)`} />

      <h2>Controlling Depth</h2>
      <p>
        The default depth is 4 levels. Use <code>-d</code> to go deeper:
      </p>
      <CodeBlock
        language="bash"
        code={`echo 'public class Greeter {\n    public string Greet(string name) {\n        return "Hello, " + name;\n    }\n}' | tractor -l csharp -v schema -d 6`}
      />
      <OutputBlock output={`Files
└─ File
   └─ unit
      └─ class  class
         ├─ public
         ├─ body  {…}
         │  └─ method
         │     ├─ name  Greet
         │     ├─ parameters  (…)
         │     │  └─ … (3 children)
         │     ├─ body
         │     │  └─ … (10 children)
         │     ├─ returns
         │     │  └─ … (1 children)
         │     └─ public
         └─ name  Greeter

(use -d to increase depth, or -x to query specific elements)`} />

      <h2>Schema on Query Results</h2>
      <p>
        Combine <code>-x</code> with <code>-v schema</code> to see the structure inside matched elements. This is the most powerful way to explore:
      </p>
      <CodeBlock
        language="bash"
        code={`echo 'public class Greeter {\n    public string Greet(string name) {\n        return "Hello, " + name;\n    }\n    public int Add(int a, int b) {\n        return a + b;\n    }\n}' | tractor -l csharp -x "//method" -v schema`}
      />
      <OutputBlock output={`method (2)
├─ public (2)
├─ returns (2)
│  └─ type (2)  string, int
├─ body (2)
│  └─ block (2)  {…}
│     └─ return (2)  return, ;
│        └─ binary (2)  +
│           └─ … (8 children)
├─ parameters (2)  (, ), ,
│  └─ parameter (3)
│     ├─ type (3)  string, int
│     └─ name (3)  name, a, b
└─ name (2)  Greet, Add

(use -d to increase depth, or -x to query specific elements)`} />

      <h3>Reading the Output</h3>
      <ul>
        <li><strong>Numbers in parentheses</strong> like <code>(2)</code> — how many times this element appears across all matches</li>
        <li><strong>Values after the name</strong> like <code>string, int</code> — unique text values found in these elements</li>
        <li><strong>Ellipsis</strong> — deeper children exist (increase <code>-d</code> to reveal them)</li>
      </ul>

      <h2>Go Deeper with -d</h2>
      <CodeBlock
        language="bash"
        code={`echo 'public class Greeter {\n    public string Greet(string name) {\n        return "Hello, " + name;\n    }\n}' | tractor -l csharp -x "//method" -v schema -d 8`}
      />
      <OutputBlock output={`method
├─ name  Greet
├─ public
├─ returns
│  └─ type  string
├─ body
│  └─ block  {…}
│     └─ return  return, ;
│        └─ binary  +
│           ├─ left
│           │  └─ string  "
│           │     └─ string_literal_content  Hello,
│           ├─ right
│           │  └─ ref  name
│           └─ op  +
│              └─ plus
└─ parameters  (…)
   └─ parameter
      ├─ name  name
      └─ type  string`} />

      <h2>Across Multiple Files</h2>
      <p>
        Schema really shines when exploring a whole codebase:
      </p>
      <CodeBlock
        language="bash"
        code={`tractor "src/**/*.cs" -v schema`}
      />
      <p>
        This shows you every element type across all C# files in <code>src/</code>. From there, narrow down:
      </p>
      <CodeBlock
        language="bash"
        code={`# What do classes look like?
tractor "src/**/*.cs" -x "//class" -v schema

# What do methods have inside?
tractor "src/**/*.cs" -x "//method" -v schema -d 6`}
      />

      <h2>Workflow Summary</h2>
      <ol>
        <li><code>tractor "files" -v schema</code> — see what's there</li>
        <li><code>tractor "files" -x "//element" -v schema</code> — zoom into an element type</li>
        <li>Increase <code>-d</code> to go deeper</li>
        <li>Once you know the structure, write your query</li>
      </ol>

      <div className="doc-next">
        <p>Next: <Link to="/docs/guides/lint-rules">Writing Lint Rules</Link> — turn queries into enforceable rules.</p>
      </div>
    </DocLayout>
  );
}
