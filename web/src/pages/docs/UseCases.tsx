import { Link } from 'react-router-dom';
import { DocLayout } from '../../components/DocLayout';
import { CodeBlock, Example } from '../../components/CodeBlock';

const CONTROLLER_CS = `[ApiController]
[Route("api/[controller]")]
public class OrdersController : ControllerBase
{
    [HttpGet]
    [Authorize]
    public IActionResult GetOrders() { ... }

    [HttpPost]
    public IActionResult CreateOrder() { ... }

    [HttpDelete("{id}")]
    [AllowAnonymous]
    public IActionResult Health() { ... }
}`;

const TENANT_CS = `[ApiController]
public class InvoicesController : ControllerBase
{
    [HttpGet]
    public IActionResult GetInvoices()
    {
        var tenantId = GetTenantId();
        return Ok(_service.GetInvoices(tenantId));
    }

    [HttpPost]
    public IActionResult CreateInvoice(CreateInvoiceRequest request)
    {
        _service.CreateInvoice(request);
        return Ok();
    }
}`;

const NAMING_CS = `public class OrderService
{
    public async Task<Order> getOrder(int id) { ... }
    public async Task<List<Order>> GetAllOrders() { ... }
    public async Task deleteOrder(int id) { ... }
}`;

export function UseCases() {
  return (
    <DocLayout>
      <h1>Use Cases</h1>
      <p className="doc-lead">
        Tractor is a general-purpose code querying tool. Here are the most common ways teams use it to keep their codebase consistent and catch problems early.
      </p>

      <div className="doc-cards">
        <a href="#code-conventions" className="doc-card">
          <h3>Code Conventions</h3>
          <p>Enforce naming, syntax, and style rules across your codebase.</p>
        </a>
        <a href="#preventing-serious-errors" className="doc-card">
          <h3>Preventing Serious Errors</h3>
          <p>Catch missing security attributes, forgotten checks, and dangerous omissions.</p>
        </a>
        <a href="#architecture-enforcement" className="doc-card">
          <h3>Architecture Enforcement</h3>
          <p>Ensure structural patterns and design rules are followed consistently.</p>
        </a>
        <a href="#ai-guard-railing" className="doc-card">
          <h3>AI Guard Railing</h3>
          <p>Give AI coding agents instant corrections instead of lengthy style guides.</p>
        </a>
        <a href="#centralized-configuration" className="doc-card">
          <h3>Centralized Configuration</h3>
          <p>Fan out a single value across multiple config files and formats.</p>
        </a>
      </div>

      {/* ── Code Conventions ── */}
      <h2 id="code-conventions">Code Conventions</h2>
      <p>
        Syntactical rules that define <em>how</em> you write code. These are the low-level, high-volume rules that keep a codebase looking consistent — naming, modifier usage, modern syntax, and more.
      </p>

      <h3>Enforce naming conventions</h3>
      <p>
        Catch methods that don't follow PascalCase in C#:
      </p>
      <Example
        file={{ name: 'OrderService.cs', language: 'csharp', content: NAMING_CS }}
        command={`tractor check OrderService.cs \\
    -x "//method[public]/name[matches(., '^[a-z]')]" \\
    --reason "Public methods should use PascalCase"`}
        output={`OrderService.cs:3:5: error: Public methods should use PascalCase
3 |     public async Task<Order> getOrder(int id) { ... }
                                 ^~~~~~~~
OrderService.cs:5:5: error: Public methods should use PascalCase
5 |     public async Task deleteOrder(int id) { ... }
                          ^~~~~~~~~~~


2 errors in 1 file`}
      />

      <p>
        The same approach works for other languages. In TypeScript, you might enforce camelCase:
      </p>
      <CodeBlock
        language="yaml"
        code={`- id: method-camelcase
  xpath: "//method[public]/name[matches(., '^[A-Z]')]"
  reason: "Public methods should use camelCase"
  severity: error`}
      />

      <h3>Require file-scoped namespaces (C# 10+)</h3>
      <CodeBlock
        language="yaml"
        code={`- id: namespaces-file-scoped
  xpath: "//namespace[block][not(contains(name, 'Migrations'))]/name"
  reason: "Use file-scoped namespaces"
  severity: error`}
      />

      <h3>Mapper methods should be extension methods</h3>
      <CodeBlock
        language="yaml"
        code={`- id: mapper-extension-method
  xpath: >-
    //class[static][name[contains(., 'Mapper')]]
    /method[public][static]
    [count(params/param)=1]
    [not(params/param[this])]/name
  reason: "Static mapping methods should be extension methods"
  severity: error`}
      />

      <h3>Don't apply [Required] to non-nullable value types</h3>
      <p>
        The <code>[Required]</code> attribute has no effect on value types like <code>int</code>, <code>bool</code>, or <code>Guid</code> — they can never be null. This rule catches the mistake:
      </p>
      <CodeBlock
        language="yaml"
        code={`- id: required-on-value-type
  xpath: >-
    //prop[public]
    [attrs[contains(., 'Required')]]
    [type[contains('|int|long|bool|Guid|', concat('|', ., '|'))]]
    [not(nullable)]/name
  reason: "[Required] has no effect on non-nullable value types"
  severity: error`}
      />

      {/* ── Preventing Serious Errors ── */}
      <h2 id="preventing-serious-errors">Preventing Serious Errors</h2>
      <p>
        Some mistakes don't just look wrong — they cause security vulnerabilities, data leaks, or broken functionality. These are the rules that catch dangerous omissions before they reach production.
      </p>

      <h3>Missing [Authorize] on controller methods</h3>
      <p>
        Every public controller action should either have <code>[Authorize]</code> or explicitly opt out with <code>[AllowAnonymous]</code>. Forgetting both means the endpoint is open to anyone:
      </p>
      <Example
        file={{ name: 'OrdersController.cs', language: 'csharp', content: CONTROLLER_CS }}
        command={`tractor check OrdersController.cs \\
    -x "//class[contains(name,'Controller')]
        /method[public]
        [not(attrs[contains(.,'Authorize')])]
        [not(attrs[contains(.,'AllowAnonymous')])]/name" \\
    --reason "Controller actions must have [Authorize] or [AllowAnonymous]"`}
        output={`OrdersController.cs:14:5: error: Controller actions must have [Authorize] or [AllowAnonymous]
14 |     public IActionResult CreateOrder() { ... }
                              ^~~~~~~~~~~


1 error in 1 file`}
      />

      <h3>Missing tenant ID in multi-tenant controllers</h3>
      <p>
        In multi-tenant applications, every controller action that accesses data should read the tenant ID. Forgetting this can leak data across tenants:
      </p>
      <Example
        file={{ name: 'InvoicesController.cs', language: 'csharp', content: TENANT_CS }}
        command={`tractor check InvoicesController.cs \\
    -x "//class[contains(name,'Controller')]
        /method[public]
        [not(contains(.,'GetTenantId'))]/name" \\
    --reason "Controller actions must call GetTenantId()"`}
        output={`InvoicesController.cs:12:5: error: Controller actions must call GetTenantId()
12 |     public IActionResult CreateInvoice(CreateInvoiceRequest request)
                              ^~~~~~~~~~~~~


1 error in 1 file`}
      />

      <h3>Query methods must use AsNoTracking</h3>
      <p>
        Read-only queries that map results should use <code>AsNoTracking()</code> for performance. Missing it means Entity Framework tracks entities unnecessarily:
      </p>
      <CodeBlock
        language="yaml"
        code={`- id: query-asnotracking
  xpath: >-
    //method[name[starts-with(., 'Get')]]
    [contains(., '_context')]
    [contains(., 'Map')]
    [not(contains(., 'AsNoTracking'))]
    [not(contains(., 'SaveChanges'))]/name
  reason: "Read-only queries should use AsNoTracking()"
  severity: error`}
      />

      <h3>Repository GetAll methods must sort results</h3>
      <CodeBlock
        language="yaml"
        code={`- id: repository-getall-orderby
  xpath: >-
    //class[contains(name,'Repository')]
    [not(name[contains(.,'Mock')])]
    /method[name[contains(.,'GetAll')]]
    [not(contains(.,'OrderBy'))]/name
  reason: "GetAll methods must include OrderBy for deterministic results"
  severity: error`}
      />

      {/* ── Architecture Enforcement ── */}
      <h2 id="architecture-enforcement">Architecture Enforcement</h2>
      <p>
        Beyond individual lines of code, tractor can enforce structural rules about how your code is organized — design patterns, layering constraints, and API contracts. This section will expand as more patterns emerge.
      </p>

      <h3>Functions should not have too many parameters</h3>
      <CodeBlock
        language="yaml"
        code={`- id: max-parameters
  xpath: "//function[count(parameters/params/type) > 5]/name"
  reason: "Functions should have at most 5 parameters — consider a parameter object"
  severity: warning`}
      />

      <h3>Test builders must make exactly one API call</h3>
      <p>
        In integration test frameworks, builder classes should follow the single-responsibility principle — each builder creates one entity via one API call:
      </p>
      <CodeBlock
        language="yaml"
        code={`- id: builder-single-api-call
  files: ["Builders/**/*.cs"]
  xpath: "//method[name='BuildMainEntity'][count(.//await) > 1]/name"
  reason: "Builders should make exactly one API call"
  severity: error`}
      />

      <h3>Model IDs must be nullable</h3>
      <p>
        In test models, ID properties should be <code>Guid?</code> because they're only populated after persistence:
      </p>
      <CodeBlock
        language="yaml"
        code={`- id: model-id-nullable
  files: ["Models/**/*.cs"]
  xpath: "//prop[name='Id'][type='Guid'][not(nullable)]/name"
  reason: "Model IDs must be Guid? — they are null until persisted"
  severity: error`}
      />

      <h3>Collections must be initialized</h3>
      <CodeBlock
        language="yaml"
        code={`- id: collection-initialized
  files: ["Models/**/*.cs"]
  xpath: "//prop[type[contains(.,'List')]][not(default)]/name"
  reason: "Collection properties must have a default initializer"
  severity: error`}
      />

      <p>
        <em>This section is a starting point — architecture enforcement is one of tractor's strongest areas. We'll expand this with more patterns for layering rules, dependency constraints, and API design enforcement.</em>
      </p>

      {/* ── AI Guard Railing ── */}
      <h2 id="ai-guard-railing">AI Guard Railing</h2>
      <p>
        AI coding assistants (ChatGPT, Claude, Copilot) generate code fast — but they don't always follow your team's conventions. The traditional approach is to write lengthy style guides in markdown files that the LLM reads before generating code. This works, but it's expensive: every token of instructions costs latency and context window space.
      </p>
      <p>
        Tractor offers a more efficient alternative: <strong>let the AI write code freely, then correct mistakes instantly.</strong>
      </p>

      <h3>How it works</h3>
      <ol>
        <li>The AI generates code as usual</li>
        <li>Tractor runs your rules against the generated code</li>
        <li>Violations are fed back to the AI as concrete error messages with line numbers</li>
        <li>The AI fixes only what's wrong — no re-reading of style guides needed</li>
      </ol>
      <p>
        This is the same feedback loop as a compiler or linter, but for your team's custom conventions. The AI already knows how to respond to error messages — it does it all day with TypeScript errors and ESLint warnings.
      </p>

      <h3>Example: CI hook for AI-generated PRs</h3>
      <p>
        Add tractor to your CI pipeline. When an AI agent opens a PR, it gets the same feedback as a human developer:
      </p>
      <CodeBlock
        language="yaml"
        title=".tractor.yml"
        code={`diff-lines: "main..HEAD"

check:
  files:
    - "src/**/*.cs"
  rules:
    - id: authorize-required
      xpath: >-
        //class[contains(name,'Controller')]
        /method[public]
        [not(attrs[contains(.,'Authorize')])]
        [not(attrs[contains(.,'AllowAnonymous')])]/name
      reason: "Controller actions must have [Authorize] or [AllowAnonymous]"
      severity: error

    - id: tenant-id-required
      xpath: >-
        //class[contains(name,'Controller')]
        /method[public]
        [not(contains(.,'GetTenantId'))]/name
      reason: "Controller actions must call GetTenantId()"
      severity: error

    - id: method-pascalcase
      xpath: "//method[public]/name[matches(., '^[a-z]')]"
      reason: "Public methods should use PascalCase"
      severity: error`}
      />
      <CodeBlock
        language="bash"
        title="CI step"
        code={`tractor run .tractor.yml --format github`}
      />
      <p>
        With <code>--format github</code>, violations appear as inline annotations on the PR — the AI agent can read them and push a fix automatically.
      </p>

      <h3>Why this beats style guides</h3>
      <table className="doc-table">
        <thead>
          <tr><th></th><th>Markdown style guide</th><th>Tractor rules</th></tr>
        </thead>
        <tbody>
          <tr><td><strong>Cost</strong></td><td>Uses context window on every request</td><td>Zero tokens — runs as a separate step</td></tr>
          <tr><td><strong>Precision</strong></td><td>LLM may misinterpret instructions</td><td>Exact match — no ambiguity</td></tr>
          <tr><td><strong>Feedback</strong></td><td>None until human review</td><td>Instant, with file, line, and reason</td></tr>
          <tr><td><strong>Consistency</strong></td><td>Varies by prompt and model</td><td>Same rules for humans and AI</td></tr>
          <tr><td><strong>Maintenance</strong></td><td>Duplicate rules in prose and code</td><td>One rule file, enforced everywhere</td></tr>
        </tbody>
      </table>

      <h3>Instant feedback with Claude Code hooks</h3>
      <p>
        For the tightest feedback loop, you can run tractor as an <strong>edit hook</strong> in Claude Code. Every time Claude edits a file, tractor checks just that file and reports violations immediately — before Claude moves on to the next change.
      </p>
      <p>
        Add this to your project's <code>.claude/settings.json</code>:
      </p>
      <CodeBlock
        language="json"
        title=".claude/settings.json"
        code={`{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "Edit|Write",
        "hooks": [
          {
            "type": "command",
            "command": "FILE=$(jq -r '.tool_input.file_path') && tractor run .tractor.yml --files \\"$FILE\\" -f claude-code"
          }
        ]
      }
    ]
  }
}`}
      />
      <p>
        With <code>-f claude-code</code>, tractor emits the JSON format that Claude Code hooks expect — no extra <code>jq</code> wrapping needed for the output. When violations are found, Claude sees the errors and fixes them immediately. When the file is clean, tractor outputs nothing and the hook passes silently.
      </p>
      <p>
        This way, Claude gets the same error-and-fix loop it's used to from compilers and linters — but for your team's custom rules. No style guide to read, no tokens wasted, just immediate corrections on every edit.
      </p>

      {/* ── Centralized Configuration ── */}
      <h2 id="centralized-configuration">Centralized Configuration</h2>
      <p>
        Use tractor's <Link to="/docs/commands/set"><code>set</code> command</Link> to maintain a single source of truth and fan out values to multiple config files — even across different formats.
      </p>

      <h3>Fan out a value to multiple files</h3>
      <p>
        Set the database host in every environment config at once:
      </p>
      <CodeBlock
        language="bash"
        code={`tractor set "config/**/*.yaml" -x "//database/host" --value "new-db.example.com"`}
      />

      <h3>Update across formats</h3>
      <p>
        The same query syntax works for JSON, YAML, TOML, and INI. Update a version number everywhere, regardless of format:
      </p>
      <CodeBlock
        language="bash"
        code={`tractor set config.json config.yaml config.toml \\
    -x "//app/version" --value "2.1.0"`}
      />

      <h3>Inject CI/CD variables</h3>
      <p>
        Replace placeholder values with environment-specific settings before deployment:
      </p>
      <CodeBlock
        language="bash"
        code={`tractor set "deploy/**/*.yaml" -x "//image/tag" --value "$CI_COMMIT_SHA"`}
      />

      <h3>Create missing structure</h3>
      <p>
        Use path expressions to ensure config files have the required keys — tractor creates whatever is missing:
      </p>
      <CodeBlock
        language="bash"
        code={`tractor set defaults.json "logging[level='info'][format='json']"`}
      />
      <p>
        See the <Link to="/docs/commands/set">set command</Link> documentation for the full reference.
      </p>

      <div className="doc-next">
        <p>Next: <Link to="/docs/guides/writing-queries">Writing Queries</Link> — learn to build your own rules step by step.</p>
      </div>
    </DocLayout>
  );
}
