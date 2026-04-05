import { Link } from 'react-router-dom';
import { DocLayout } from '../../components/DocLayout';
import { CodeBlock, Example } from '../../components/CodeBlock';

const GREETER_JS = `function greet(name) {
  return "Hello, " + name;
}

function add(a, b) {
  return a + b;
}`;

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

      <h2>Controlling Depth</h2>
      <p>
        The default depth is 4 levels. Use <code>-d</code> to go deeper:
      </p>
      <Example
        command={`tractor greeter.js -v schema -d 6`}
        output={`Files
└─ File
   └─ program
      └─ function (2)  function
         ├─ parameters (2)
         │  └─ params (2)  (, ), ,
         │     └─ type (3)  name, a, b
         ├─ name (2)  greet, add
         └─ body (2)
            └─ block (2)  {…}
               └─ return (2)  return, ;
                  └─ … (9 children)

(use -d to increase depth, or -x to query specific elements)`}
      />

      <h2>Schema on Query Results</h2>
      <p>
        Combine <code>-x</code> with <code>-v schema</code> to see the structure inside matched elements. This is the most powerful way to explore:
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

      <h3>Reading the Output</h3>
      <ul>
        <li><strong>Numbers in parentheses</strong> like <code>(2)</code> — how many times this element appears across all matches</li>
        <li><strong>Values after the name</strong> like <code>greet, add</code> — unique text values found in these elements</li>
        <li><strong>Ellipsis</strong> — deeper children exist (increase <code>-d</code> to reveal them)</li>
      </ul>

      <h2>Go Deeper with -d</h2>
      <Example
        command={`tractor greeter.js -x "//function" -v schema -d 8`}
        output={`function (2)  function
├─ name (2)  greet, add
├─ parameters (2)
│  └─ params (2)  (, ), ,
│     └─ type (3)  name, a, b
└─ body (2)
   └─ block (2)  {…}
      └─ return (2)  return, ;
         └─ binary (2)  +
            ├─ op (2)  +
            │  └─ plus (2)
            ├─ right (2)
            │  └─ type (2)  name, b
            └─ left (2)
               ├─ string  "
               │  └─ string_fragment  Hello,
               └─ type  a`}
      />

      <h2>Across Multiple Files</h2>
      <p>
        Schema really shines when exploring a whole codebase:
      </p>
      <CodeBlock language="bash" code={`tractor "src/**/*.js" -v schema`} />
      <p>
        This shows you every element type across all JavaScript files in <code>src/</code>. From there, narrow down:
      </p>
      <CodeBlock
        language="bash"
        code={`# What do classes look like?
tractor "src/**/*.js" -x "//class" -v schema

# What do functions have inside?
tractor "src/**/*.js" -x "//function" -v schema -d 6`}
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
