import { Link } from 'react-router-dom';
import { DocLayout } from '../../components/DocLayout';
import { CodeBlock, Example } from '../../components/CodeBlock';

export function DocumentLanguages() {
  return (
    <DocLayout>
      <h1>Document Languages</h1>
      <p className="doc-lead">
        Markdown, CSS, and HTML are parsed into a structure tree — similar to code, but with elements specific to each format.
      </p>

      <h2>Markdown</h2>
      <p>
        Markdown is parsed into <code>section</code>, <code>heading</code>, <code>paragraph</code>, and <code>inline</code> nodes. Heading levels are represented as child markers like <code>h1</code>, <code>h2</code>, etc.
      </p>
      <Example
        file={{ name: 'readme.md', language: 'bash', content: `# Title
Some text
## Subtitle
More text` }}
        command={`tractor readme.md -v schema -d 6`}
        output={`Files
└─ File
   └─ document
      └─ section
         ├─ section
         │  ├─ heading
         │  │  ├─ inline  Subtitle
         │  │  └─ h2
         │  └─ paragraph
         │     └─ inline  More text
         ├─ heading
         │  ├─ inline  Title
         │  └─ h1
         └─ paragraph
            └─ inline  Some text`}
      />

      <h3>Finding all headings</h3>
      <Example
        command={`tractor readme.md -x "//heading/inline" -v value`}
        output={`Title\nSubtitle`}
      />

      <h3>Finding only h1 headings</h3>
      <CodeBlock language="bash" code={`tractor readme.md -x "//heading[h1]/inline" -v value`} />

      <h2>CSS</h2>
      <p>
        CSS is parsed into <code>rule_set</code>, <code>selectors</code>, <code>declaration</code>, <code>property_name</code>, and value nodes.
      </p>
      <Example
        file={{ name: 'styles.css', language: 'css', content: `body { color: red; }
.header { font-size: 16px; }` }}
        command={`tractor styles.css -v schema -d 6`}
        output={`Files
└─ File
   └─ stylesheet
      └─ rule_set (2)
         ├─ block (2)  {…}
         │  └─ declaration (2)  :, ;
         │     ├─ property_name (2)  color, font-size
         │     ├─ integer_value  16
         │     │  └─ … (1 children)
         │     └─ plain_value  red
         └─ selectors (2)
            ├─ tag_name  body
            └─ class_selector  .
               └─ class_name
                  └─ … (1 children)

(use -d to increase depth, or -x to query specific elements)`}
      />

      <h3>Finding all CSS properties</h3>
      <Example
        command={`tractor styles.css -x "//declaration/property_name" -v value`}
        output={`color\nfont-size`}
      />

      <h2>HTML</h2>
      <p>
        HTML is parsed into a syntax tree with <code>element</code>, <code>start_tag</code>, <code>tag_name</code>, <code>attribute</code>, and <code>text</code> nodes.
      </p>
      <p>
        <strong>Note:</strong> HTML support is basic. The tree exposes the parser's syntax structure, which means you query through nodes like <code>start_tag/tag_name</code> rather than directly by tag name. For example, you can't write <code>//h1</code> — you need <code>//element[.//tag_name='h1']</code>. If you're working with well-formed HTML, consider treating it as XML (<code>-l xml</code> or <code>-t raw</code>) where you can query the tag structure directly.
      </p>
      <Example
        file={{ name: 'page.html', language: 'xml', content: `<div class="app">
  <h1>Hello</h1>
  <p>World</p>
</div>` }}
        command={`tractor page.html -v schema -d 6`}
        output={`Files
└─ File
   └─ document
      └─ element
         ├─ element (2)
         │  ├─ text (2)  Hello, World
         │  ├─ start_tag (2)  <…>
         │  │  └─ tag_name (2)  h1, p
         │  └─ end_tag (2)  </, >
         │     └─ tag_name (2)  h1, p
         ├─ start_tag  <…>
         │  ├─ attribute  =
         │  │  ├─ attribute_name  class
         │  │  └─ quoted_attribute_value  "
         │  │     └─ … (1 children)
         │  └─ tag_name  div
         └─ end_tag  </, >
            └─ tag_name  div

(use -d to increase depth, or -x to query specific elements)`}
      />

      <h3>Finding all tag names</h3>
      <Example
        command={`tractor page.html -x "//start_tag/tag_name" -v value`}
        output={`div\nh1\np`}
      />

      <h3>Finding elements with a specific class</h3>
      <CodeBlock language="bash" code={`tractor page.html -x "//element[.//attribute_name='class'][contains(.//quoted_attribute_value,'app')]" -v source`} />

      <h2>Supported Formats</h2>
      <table className="doc-table">
        <thead>
          <tr><th>Language</th><th>Extension</th><th>-l value</th></tr>
        </thead>
        <tbody>
          <tr><td>Markdown</td><td><code>.md</code></td><td><code>markdown</code></td></tr>
          <tr><td>CSS</td><td><code>.css</code></td><td><code>css</code></td></tr>
          <tr><td>HTML</td><td><code>.html</code></td><td><code>html</code></td></tr>
        </tbody>
      </table>

      <div className="doc-next">
        <p>See also: <Link to="/docs/languages/code">Code Languages</Link> and <Link to="/docs/languages/data">Data Formats</Link>.</p>
      </div>
    </DocLayout>
  );
}
