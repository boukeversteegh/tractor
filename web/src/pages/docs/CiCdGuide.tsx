import { DocLayout } from '../../components/DocLayout';
import { CodeBlock, OutputBlock } from '../../components/CodeBlock';

export function CiCdGuide() {
  return (
    <DocLayout>
      <h1>CI/CD Integration</h1>
      <p className="doc-lead">
        Set up tractor in your pipeline to enforce conventions automatically. Download the binary, add your config, and run.
      </p>

      <h2>Quick Setup</h2>
      <p>
        The simplest way to add tractor to CI:
      </p>
      <ol>
        <li>Download the binary from the <a href="https://github.com/boukeversteegh/tractor/releases/latest" target="_blank" rel="noopener noreferrer">latest release</a></li>
        <li>Create a <code>.tractor.yml</code> in your repo</li>
        <li>Run <code>tractor run .tractor.yml</code></li>
      </ol>

      <h2>GitHub Actions</h2>
      <CodeBlock
        language="yaml"
        title=".github/workflows/tractor.yml"
        code={`name: Tractor
on: [push, pull_request]

jobs:
  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Download tractor
        run: |
          curl -sL https://github.com/boukeversteegh/tractor/releases/latest/download/tractor-linux-x86_64 -o tractor
          chmod +x tractor

      - name: Run checks
        run: ./tractor run .tractor.yml`}
      />

      <h3>With GitHub Annotations</h3>
      <p>
        Use <code>-f github</code> to make violations show directly on the pull request:
      </p>
      <CodeBlock
        language="yaml"
        title=".github/workflows/tractor.yml"
        code={`name: Tractor
on: [push, pull_request]

jobs:
  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Download tractor
        run: |
          curl -sL https://github.com/boukeversteegh/tractor/releases/latest/download/tractor-linux-x86_64 -o tractor
          chmod +x tractor

      - name: Run checks
        run: ./tractor run .tractor.yml -f github`}
      />
      <p>
        With <code>-f github</code>, violations produce GitHub annotation output:
      </p>
      <OutputBlock output={`::error file=src/app.cs,line=1,endLine=1,col=1,endColumn=24::TODO comment found`} />
      <p>
        These annotations appear inline on the pull request diff.
      </p>

      <h3>Only Check Changed Code</h3>
      <p>
        Use <code>--diff-lines</code> to only flag violations in new or changed code:
      </p>
      <CodeBlock
        language="yaml"
        title=".github/workflows/tractor.yml"
        code={`name: Tractor
on: pull_request

jobs:
  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Download tractor
        run: |
          curl -sL https://github.com/boukeversteegh/tractor/releases/latest/download/tractor-linux-x86_64 -o tractor
          chmod +x tractor

      - name: Run checks on changed code
        run: ./tractor run .tractor.yml --diff-lines "origin/main..HEAD" -f github`}
      />
      <p>
        Note: <code>fetch-depth: 0</code> is needed so git has the full history for the diff.
      </p>

      <h2>GitLab CI</h2>
      <CodeBlock
        language="yaml"
        title=".gitlab-ci.yml"
        code={`tractor:
  stage: lint
  script:
    - curl -sL https://github.com/boukeversteegh/tractor/releases/latest/download/tractor-linux-x86_64 -o tractor
    - chmod +x tractor
    - ./tractor run .tractor.yml`}
      />

      <h2>Azure DevOps</h2>
      <CodeBlock
        language="yaml"
        title="azure-pipelines.yml"
        code={`steps:
  - script: |
      curl -sL https://github.com/boukeversteegh/tractor/releases/latest/download/tractor-linux-x86_64 -o tractor
      chmod +x tractor
    displayName: 'Install tractor'

  - script: ./tractor run .tractor.yml
    displayName: 'Run tractor checks'`}
      />

      <h2>Example Config</h2>
      <p>
        A typical <code>.tractor.yml</code> for a C# project:
      </p>
      <CodeBlock
        language="yaml"
        title=".tractor.yml"
        code={`check:
  files:
    - "src/**/*.cs"
  exclude:
    - "src/generated/**"
  rules:
    - id: no-todo
      xpath: "//comment[contains(.,'TODO')]"
      reason: "TODO comments should be resolved"
      severity: warning
      expect:
        - valid: "public class Clean { }"
        - invalid: "// TODO: fix this"

    - id: repository-needs-orderby
      xpath: >-
        //class[contains(name,'Repository')]
        //method[contains(name,'GetAll')]
        [not(contains(.,'OrderBy'))]/name
      reason: "GetAll methods in repositories should use OrderBy"
      severity: error`}
      />

      <h2>Exit Codes</h2>
      <table className="doc-table">
        <thead>
          <tr><th>Code</th><th>Meaning</th></tr>
        </thead>
        <tbody>
          <tr><td><code>0</code></td><td>All checks passed (or only warnings)</td></tr>
          <tr><td><code>1</code></td><td>Errors found</td></tr>
        </tbody>
      </table>

      <h2>Tips</h2>
      <ul>
        <li>Cache the tractor binary to speed up builds</li>
        <li>Use <code>--diff-lines</code> for gradual adoption on existing codebases</li>
        <li>Start with <code>severity: warning</code> and upgrade to <code>error</code> once clean</li>
        <li>Use <code>-f github</code> in GitHub Actions for inline annotations</li>
        <li>Pin to a specific release version for reproducible builds:
          <CodeBlock
            language="bash"
            code={`curl -sL https://github.com/boukeversteegh/tractor/releases/download/v0.1.0/tractor-linux-x86_64 -o tractor`}
          />
        </li>
      </ul>
    </DocLayout>
  );
}
