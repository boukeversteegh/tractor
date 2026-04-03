import { Link } from 'react-router-dom';
import { DocLayout } from '../../components/DocLayout';
import { CodeBlock, OutputBlock } from '../../components/CodeBlock';

export function WritingQueries() {
  return (
    <DocLayout>
      <h1>Writing Queries</h1>
      <p className="doc-lead">
        Learn to write tractor queries step by step. Start by looking at the tree, then narrow down with expressions.
      </p>

      <h2>The Workflow</h2>
      <p>
        Tractor queries follow a simple iterative process:
      </p>
      <ol>
        <li><strong>See the structure</strong> — explore with schema view</li>
        <li><strong>View the tree</strong> — inspect the full XML of specific code</li>
        <li><strong>Select elements</strong> — use <code>-x</code> to pick elements</li>
        <li><strong>Add predicates</strong> — filter by conditions</li>
        <li><strong>Choose a view</strong> — pick the output that fits your use case</li>
      </ol>

      <h2>Step 1: Explore the Structure</h2>
      <p>
        Start with the <Link to="/docs/guides/schema">schema view</Link> to see what element types exist:
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
      <p>
        You can see there's a <code>class</code> element with <code>name</code>, <code>public</code>, and a <code>body</code>. Let's look deeper.
      </p>

      <h2>Step 2: View the Full Tree</h2>
      <p>
        Inspect specific elements to see the exact structure:
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
      <p>
        Now you can see each <code>method</code> has a <code>name</code>, <code>returns</code>, <code>parameters</code>, and <code>body</code>.
      </p>

      <h2>Step 3: Select Elements</h2>
      <p>
        Use <code>-x</code> with an XPath expression to select elements. The syntax uses path expressions, similar to file paths:
      </p>

      <h3>Select all methods</h3>
      <CodeBlock
        language="bash"
        code={`echo 'public class UserService {\n    public static User FindById(int id) { return null; }\n    public User Save(User user) { return user; }\n    private void Log(string msg) { }\n}' | tractor -l csharp -x "//method/name" -v value`}
      />
      <OutputBlock output={`FindById\nSave\nLog`} />

      <h3>Common path expressions</h3>
      <table className="doc-table">
        <thead>
          <tr><th>Expression</th><th>Meaning</th></tr>
        </thead>
        <tbody>
          <tr><td><code>//method</code></td><td>All methods anywhere in the tree</td></tr>
          <tr><td><code>//class/name</code></td><td>Name of every class</td></tr>
          <tr><td><code>//class//method</code></td><td>All methods inside any class</td></tr>
          <tr><td><code>//method/parameters/parameter</code></td><td>All parameters of all methods</td></tr>
        </tbody>
      </table>

      <h2>Step 4: Add Predicates</h2>
      <p>
        Predicates go in square brackets and filter matches by conditions:
      </p>

      <h3>Filter by child element</h3>
      <CodeBlock
        language="bash"
        code={`echo 'public class UserService {\n    public static User FindById(int id) { return null; }\n    public User Save(User user) { return user; }\n    private void Log(string msg) { }\n}' | tractor -l csharp -x "//method[public][not(static)]/name" -v value`}
      />
      <OutputBlock output="Save" />
      <p>
        <code>[public]</code> means "has a <code>&lt;public/&gt;</code> child". <code>[not(static)]</code> means "does not have a <code>&lt;static/&gt;</code> child".
      </p>

      <h3>Filter by text content</h3>
      <CodeBlock
        language="bash"
        code={`echo 'public class UserService {\n    public static User FindById(int id) { return null; }\n    public User Save(User user) { return user; }\n    private void Log(string msg) { }\n}' | tractor -l csharp -x "//method[contains(name,'Find')]/name" -v value`}
      />
      <OutputBlock output="FindById" />

      <h3>Common predicates</h3>
      <table className="doc-table">
        <thead>
          <tr><th>Predicate</th><th>Meaning</th></tr>
        </thead>
        <tbody>
          <tr><td><code>[public]</code></td><td>Has a <code>&lt;public/&gt;</code> child element</td></tr>
          <tr><td><code>[not(static)]</code></td><td>Does not have a <code>&lt;static/&gt;</code> child</td></tr>
          <tr><td><code>[name='Foo']</code></td><td>Has name equal to "Foo"</td></tr>
          <tr><td><code>[contains(name,'Get')]</code></td><td>Name contains "Get"</td></tr>
          <tr><td><code>[contains(.,'OrderBy')]</code></td><td>Full text of element contains "OrderBy"</td></tr>
          <tr><td><code>[count(parameters/parameter) &gt; 3]</code></td><td>Has more than 3 parameters</td></tr>
          <tr><td><code>[starts-with(name,'test')]</code></td><td>Name starts with "test"</td></tr>
        </tbody>
      </table>

      <h2>Step 5: Choose a View</h2>
      <p>
        Pick the output that matches what you need:
      </p>
      <table className="doc-table">
        <thead>
          <tr><th>View</th><th>Use case</th></tr>
        </thead>
        <tbody>
          <tr><td><code>-v tree</code></td><td>Full XML (default) — see the structure</td></tr>
          <tr><td><code>-v value</code></td><td>Text content — get names, values</td></tr>
          <tr><td><code>-v source</code></td><td>Exact source code — copy-paste ready</td></tr>
          <tr><td><code>-v lines</code></td><td>Source lines with context</td></tr>
          <tr><td><code>-v count</code></td><td>Just the number of matches</td></tr>
          <tr><td><code>-v schema</code></td><td>Structural overview of what's inside</td></tr>
        </tbody>
      </table>

      <h2>Real-World Examples</h2>

      <h3>Find public methods that return void</h3>
      <CodeBlock
        language="bash"
        code={`tractor "src/**/*.cs" -x "//method[public][returns/type='void']/name" -v value`}
      />

      <h3>Find classes missing a constructor</h3>
      <CodeBlock
        language="bash"
        code={`tractor "src/**/*.cs" -x "//class[not(constructor)]/name" -v value`}
      />

      <h3>Find functions with too many parameters</h3>
      <CodeBlock
        language="bash"
        code={`tractor "src/**/*.py" -x "//function[count(parameters/parameter) > 5]/name" -v value`}
      />

      <h3>Query JSON configuration</h3>
      <CodeBlock
        language="bash"
        code={`echo '{"database": {"host": "localhost", "port": 5432}}' | tractor -l json -x "//database/host" -v value`}
      />
      <OutputBlock output="localhost" />

      <h2>Tips</h2>
      <ul>
        <li>Always start by <strong>looking at the tree</strong> — run <code>tractor file.cs</code> first.</li>
        <li>Use <code>-v schema</code> when querying multiple files to see element types at a glance.</li>
        <li>Use <code>-W</code> to ignore whitespace when matching formatted code.</li>
        <li>The <Link to="/playground">Playground</Link> lets you build queries visually.</li>
        <li>AI tools like ChatGPT and Claude can write tractor queries — the syntax is standard XPath that they already know.</li>
      </ul>

      <div className="doc-next">
        <p>Next: <Link to="/docs/guides/schema">Exploring with Schema</Link> — discover structure across your codebase.</p>
      </div>
    </DocLayout>
  );
}
