import { Link } from 'react-router-dom';
import { DocLayout } from '../../components/DocLayout';
import { CodeBlock, Example } from '../../components/CodeBlock';

const GREETER_JS = `function greet(name) {
  return "Hello, " + name;
}

function add(a, b) {
  return a + b;
}`;

const USER_SERVICE_JS = `class UserService {
  static findById(id) {
    return null;
  }

  save(user) {
    return user;
  }

  _log(msg) {
    console.log(msg);
  }
}`;

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
        <li><strong>View the tree</strong> — inspect the full tree of specific code</li>
        <li><strong>Select elements</strong> — use <code>-x</code> to pick elements</li>
        <li><strong>Add predicates</strong> — filter by conditions</li>
        <li><strong>Choose a view</strong> — pick the output that fits your use case</li>
      </ol>

      <h2>Step 1: Explore the Structure</h2>
      <p>
        Start with the <Link to="/docs/guides/schema">schema view</Link> to see what element types exist:
      </p>
      <Example
        file={{ name: 'greeter.js', language: 'js', content: GREETER_JS }}
        command={`tractor greeter.js -v schema`}
        output={`Files
└─ File
   └─ program
      └─ function (2)  function
         ├─ name (2)  greet, add
         ├─ body (2)
         │  └─ … (11 children)
         └─ parameters (2)
            └─ … (2 children)

(use -d to increase depth, or -x to query specific elements)`}
      />
      <p>
        You can see there's a <code>function</code> element with <code>name</code>, <code>body</code>, and <code>parameters</code>. Let's look deeper.
      </p>

      <h2>Step 2: Zoom Into an Element</h2>
      <p>
        Combine <code>-x</code> with <code>-v schema</code> to see the structure inside matched elements:
      </p>
      <Example
        command={`tractor greeter.js -x "//function" -v schema`}
        output={`function (2)  function
├─ body (2)
│  └─ block (2)  {…}
│     └─ return (2)  return, ;
│        └─ binary (2)  +
│           └─ … (8 children)
├─ parameters (2)
│  └─ params (2)  (, ), ,
│     └─ type (3)  name, a, b
└─ name (2)  greet, add

(use -d to increase depth, or -x to query specific elements)`}
      />
      <p>
        Now you can see each <code>function</code> has a <code>name</code>, <code>parameters</code>, and <code>body</code>.
      </p>

      <h2>Step 3: Select Elements</h2>
      <p>
        Use <code>-x</code> with a <a href="https://www.w3.org/TR/xpath-31/" target="_blank" rel="noopener noreferrer">path expression</a> to select elements. The syntax is similar to file paths:
      </p>

      <h3>Select all method names</h3>
      <Example
        file={{ name: 'user-service.js', language: 'js', content: USER_SERVICE_JS }}
        command={`tractor user-service.js -x "//method/name" -v value`}
        output={`findById\nsave\n_log`}
      />

      <h3>Common path expressions</h3>
      <table className="doc-table">
        <thead>
          <tr><th>Expression</th><th>Meaning</th></tr>
        </thead>
        <tbody>
          <tr><td><code>//function</code></td><td>All functions anywhere in the tree</td></tr>
          <tr><td><code>//class/name</code></td><td>Name of every class</td></tr>
          <tr><td><code>//class//method</code></td><td>All methods inside any class</td></tr>
          <tr><td><code>//function/parameters/params/type</code></td><td>All parameters of all functions</td></tr>
        </tbody>
      </table>

      <h2>Step 4: Add Predicates</h2>
      <p>
        Predicates go in square brackets and filter matches by conditions:
      </p>

      <h3>Filter by child element</h3>
      <Example
        command={`tractor user-service.js -x "//method[not(static)]/name" -v value`}
        output={`findById\nsave\n_log`}
      />
      <p>
        <code>[not(static)]</code> means "does not have a <code>static</code> child element".
      </p>

      <h3>Filter by text content</h3>
      <Example
        command={`tractor user-service.js -x "//method[contains(name,'find')]/name" -v value`}
        output="findById"
      />

      <h3>Common predicates</h3>
      <table className="doc-table">
        <thead>
          <tr><th>Predicate</th><th>Meaning</th></tr>
        </thead>
        <tbody>
          <tr><td><code>[public]</code></td><td>Has a <code>public</code> child element</td></tr>
          <tr><td><code>[not(static)]</code></td><td>Does not have a <code>static</code> child</td></tr>
          <tr><td><code>[name='Foo']</code></td><td>Has name equal to "Foo"</td></tr>
          <tr><td><code>[contains(name,'get')]</code></td><td>Name contains "get"</td></tr>
          <tr><td><code>[contains(.,'orderBy')]</code></td><td>Full text of element contains "orderBy"</td></tr>
          <tr><td><code>[count(parameters/params/type) &gt; 3]</code></td><td>Has more than 3 parameters</td></tr>
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
          <tr><td><code>-v tree</code></td><td>Tree view (default) — see the structure</td></tr>
          <tr><td><code>-v value</code></td><td>Text content — get names, values</td></tr>
          <tr><td><code>-v source</code></td><td>Exact source code — copy-paste ready</td></tr>
          <tr><td><code>-v lines</code></td><td>Source lines with context</td></tr>
          <tr><td><code>-v count</code></td><td>Just the number of matches</td></tr>
          <tr><td><code>-v schema</code></td><td>Structural overview of what's inside</td></tr>
        </tbody>
      </table>

      <h2>Real-World Examples</h2>

      <h3>Find all class names in a project</h3>
      <CodeBlock language="bash" code={`tractor "src/**/*.js" -x "//class/name" -v value`} />

      <h3>Find functions with too many parameters</h3>
      <CodeBlock language="bash" code={`tractor "src/**/*.js" -x "//function[count(parameters/params/type) > 5]/name" -v value`} />

      <h3>Query JSON configuration</h3>
      <Example
        file={{ name: 'config.json', language: 'json', content: `{"database": {"host": "localhost", "port": 5432}}` }}
        command={`tractor config.json -x "//database/host" -v value`}
        output="localhost"
      />

      <h2>Tips</h2>
      <ul>
        <li>Always start by <strong>looking at the tree</strong> — run <code>tractor file.js</code> first.</li>
        <li>Use <code>-v schema</code> when querying multiple files to see element types at a glance.</li>
        <li>Use <code>-W</code> to ignore whitespace when matching formatted code.</li>
        <li>The <Link to="/playground">Playground</Link> lets you build queries visually.</li>
        <li>AI tools like ChatGPT and Claude can write tractor queries — the syntax is standard and they already know it.</li>
      </ul>

      <div className="doc-next">
        <p>Next: <Link to="/docs/guides/schema">Exploring with Schema</Link> — discover structure across your codebase.</p>
      </div>
    </DocLayout>
  );
}
