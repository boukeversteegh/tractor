/**
 * Unit tests for queryBuilder
 * @vitest-environment jsdom
 */

import { describe, it, expect } from 'vitest';
import { buildQuery } from './queryBuilder';
import { parseXmlToTree, XmlNode } from './xmlTree';
import { SelectionState } from './queryState';

// Helper to build nodeInfoMap from tree
function buildNodeInfoMap(root: XmlNode): Map<string, { name: string }> {
  const map = new Map<string, { name: string }>();
  function traverse(n: XmlNode) {
    map.set(n.id, { name: n.name });
    n.children.forEach(traverse);
  }
  traverse(root);
  return map;
}

// Helper to find node by name (first match)
function findNode(root: XmlNode, name: string): XmlNode | null {
  if (root.name === name) return root;
  for (const child of root.children) {
    const found = findNode(child, name);
    if (found) return found;
  }
  return null;
}

// Helper to find all nodes by name
function findAllNodes(root: XmlNode, name: string): XmlNode[] {
  const results: XmlNode[] = [];
  function traverse(n: XmlNode) {
    if (n.name === name) results.push(n);
    n.children.forEach(traverse);
  }
  traverse(root);
  return results;
}

// Helper to create selection state
function select(
  ...selections: Array<{ id: string; isTarget?: boolean; condition?: string }>
): SelectionState {
  const state: SelectionState = new Map();
  for (const s of selections) {
    state.set(s.id, {
      selected: true,
      isTarget: s.isTarget ?? false,
      condition: s.condition,
    });
  }
  return state;
}

describe('buildQuery', () => {
  // Simple nested structure
  const simpleXml = `
    <class name="MyClass">
      <method name="doSomething">
        <parameter name="x"/>
      </method>
    </class>
  `;

  it('returns empty string for no selection', () => {
    const tree = parseXmlToTree(simpleXml)!;
    const map = buildNodeInfoMap(tree);
    const result = buildQuery(tree, new Map(), map);
    expect(result).toBe('');
  });

  it('returns empty string for null tree', () => {
    const result = buildQuery(null, select({ id: 'x' }), new Map());
    expect(result).toBe('');
  });

  it('single node selection uses //', () => {
    const tree = parseXmlToTree(simpleXml)!;
    const map = buildNodeInfoMap(tree);
    const method = findNode(tree, 'method')!;

    const result = buildQuery(tree, select({ id: method.id }), map);
    expect(result).toBe('//method');
  });

  it('direct parent-child uses /', () => {
    const tree = parseXmlToTree(simpleXml)!;
    const map = buildNodeInfoMap(tree);
    const cls = findNode(tree, 'class')!;
    const method = findNode(tree, 'method')!;

    const result = buildQuery(
      tree,
      select({ id: cls.id }, { id: method.id, isTarget: true }),
      map
    );
    expect(result).toBe('//class/method');
  });

  it('distant ancestor uses //', () => {
    const tree = parseXmlToTree(simpleXml)!;
    const map = buildNodeInfoMap(tree);
    const cls = findNode(tree, 'class')!;
    const param = findNode(tree, 'parameter')!;

    const result = buildQuery(
      tree,
      select({ id: cls.id }, { id: param.id, isTarget: true }),
      map
    );
    expect(result).toBe('//class//parameter');
  });

  it('descendant of target becomes predicate (direct child)', () => {
    const tree = parseXmlToTree(simpleXml)!;
    const map = buildNodeInfoMap(tree);
    const method = findNode(tree, 'method')!;
    const param = findNode(tree, 'parameter')!;

    const result = buildQuery(
      tree,
      select({ id: method.id, isTarget: true }, { id: param.id }),
      map
    );
    expect(result).toBe('//method[parameter]');
  });

  it('descendant of target becomes predicate (distant)', () => {
    const tree = parseXmlToTree(simpleXml)!;
    const map = buildNodeInfoMap(tree);
    const cls = findNode(tree, 'class')!;
    const param = findNode(tree, 'parameter')!;

    const result = buildQuery(
      tree,
      select({ id: cls.id, isTarget: true }, { id: param.id }),
      map
    );
    expect(result).toBe('//class[.//parameter]');
  });

  it('condition on target node', () => {
    const tree = parseXmlToTree(simpleXml)!;
    const map = buildNodeInfoMap(tree);
    const method = findNode(tree, 'method')!;

    const result = buildQuery(
      tree,
      select({ id: method.id, condition: "@name='doSomething'" }),
      map
    );
    expect(result).toBe("//method[@name='doSomething']");
  });

  it('condition on ancestor node', () => {
    const tree = parseXmlToTree(simpleXml)!;
    const map = buildNodeInfoMap(tree);
    const cls = findNode(tree, 'class')!;
    const method = findNode(tree, 'method')!;

    const result = buildQuery(
      tree,
      select(
        { id: cls.id, condition: "@name='MyClass'" },
        { id: method.id, isTarget: true }
      ),
      map
    );
    expect(result).toBe("//class[@name='MyClass']/method");
  });

  it('condition on descendant predicate', () => {
    const tree = parseXmlToTree(simpleXml)!;
    const map = buildNodeInfoMap(tree);
    const method = findNode(tree, 'method')!;
    const param = findNode(tree, 'parameter')!;

    const result = buildQuery(
      tree,
      select(
        { id: method.id, isTarget: true },
        { id: param.id, condition: "@name='x'" }
      ),
      map
    );
    expect(result).toBe("//method[parameter[@name='x']]");
  });

  // More complex tree
  const complexXml = `
    <class name="Calculator">
      <method name="add">
        <parameter name="a"/>
        <parameter name="b"/>
        <body>
          <return>
            <binary_expression operator="+"/>
          </return>
        </body>
      </method>
      <method name="subtract">
        <parameter name="a"/>
        <body>
          <return/>
        </body>
      </method>
    </class>
  `;

  it('multiple ancestors in path', () => {
    const tree = parseXmlToTree(complexXml)!;
    const map = buildNodeInfoMap(tree);
    const cls = findNode(tree, 'class')!;
    const method = findAllNodes(tree, 'method')[0]!;
    const ret = findNode(tree, 'return')!;

    const result = buildQuery(
      tree,
      select(
        { id: cls.id },
        { id: method.id },
        { id: ret.id, isTarget: true }
      ),
      map
    );
    expect(result).toBe('//class/method//return');
  });

  it('multiple descendant predicates (siblings)', () => {
    const tree = parseXmlToTree(complexXml)!;
    const map = buildNodeInfoMap(tree);
    const method = findAllNodes(tree, 'method')[0]!;
    const params = findAllNodes(tree, 'parameter');
    const body = findNode(tree, 'body')!;

    const result = buildQuery(
      tree,
      select(
        { id: method.id, isTarget: true },
        { id: params[0].id },
        { id: body.id }
      ),
      map
    );
    // Both are direct children of method, but siblings - separate predicates
    expect(result).toBe('//method[parameter][body]');
  });

  it('chained descendant predicates form a path', () => {
    const tree = parseXmlToTree(complexXml)!;
    const map = buildNodeInfoMap(tree);
    const method = findAllNodes(tree, 'method')[0]!;
    const body = findNode(tree, 'body')!;
    const ret = findNode(tree, 'return')!;

    const result = buildQuery(
      tree,
      select(
        { id: method.id, isTarget: true },
        { id: body.id },
        { id: ret.id }
      ),
      map
    );
    // body and return form a chain: body/return
    expect(result).toBe('//method[body/return]');
  });

  it('uses deepest node as target when none specified (linear path)', () => {
    const tree = parseXmlToTree(simpleXml)!;
    const map = buildNodeInfoMap(tree);
    const cls = findNode(tree, 'class')!;
    const param = findNode(tree, 'parameter')!;

    const result = buildQuery(
      tree,
      select({ id: cls.id }, { id: param.id }),
      map
    );
    // param is deepest, so class becomes ancestor in path
    expect(result).toBe('//class//parameter');
  });

  it('uses LCA as default target when selection branches', () => {
    const tree = parseXmlToTree(complexXml)!;
    const map = buildNodeInfoMap(tree);
    const method = findAllNodes(tree, 'method')[0]!;
    const param = findAllNodes(tree, 'parameter')[0]!;
    const ret = findNode(tree, 'return')!;

    // Select method, param (in one branch), and return (in body branch)
    // LCA of param and return is method, so method becomes target
    const result = buildQuery(
      tree,
      select(
        { id: method.id },
        { id: param.id },
        { id: ret.id }
      ),
      map
    );
    // method is LCA, param and return are in different branches
    expect(result).toBe('//method[parameter][.//return]');
  });

  it('uncle nodes become predicates on common ancestor', () => {
    // XML where name and body are siblings under method
    const xml = `
      <class>
        <method>
          <name>Hello</name>
          <body>
            <block/>
          </body>
        </method>
      </class>
    `;
    const tree = parseXmlToTree(xml)!;
    const map = buildNodeInfoMap(tree);
    const name = findNode(tree, 'name')!;
    const body = findNode(tree, 'body')!;

    // Target: body, Uncle: name with condition
    // Common ancestor of name and body is method
    // Expected: //method[name[.='Hello']]//body
    const result = buildQuery(
      tree,
      select(
        { id: name.id, condition: ".='Hello'" },
        { id: body.id, isTarget: true }
      ),
      map
    );
    expect(result).toBe("//method[name[.='Hello']]/body");
  });

  it('uncle nodes: selecting deeper target with sibling condition', () => {
    const xml = `
      <class>
        <method>
          <name>Hello</name>
          <body>
            <block>
              <return/>
            </block>
          </body>
        </method>
      </class>
    `;
    const tree = parseXmlToTree(xml)!;
    const map = buildNodeInfoMap(tree);
    const name = findNode(tree, 'name')!;
    const block = findNode(tree, 'block')!;

    // Target: block, Uncle: name with condition
    const result = buildQuery(
      tree,
      select(
        { id: name.id, condition: ".='Hello'" },
        { id: block.id, isTarget: true }
      ),
      map
    );
    expect(result).toBe("//method[name[.='Hello']]//block");
  });

  it('complex: ancestor with condition, target, descendant predicate with condition', () => {
    const tree = parseXmlToTree(complexXml)!;
    const map = buildNodeInfoMap(tree);
    const cls = findNode(tree, 'class')!;
    const method = findAllNodes(tree, 'method')[0]!;
    const binExpr = findNode(tree, 'binary_expression')!;

    const result = buildQuery(
      tree,
      select(
        { id: cls.id, condition: "@name='Calculator'" },
        { id: method.id, isTarget: true },
        { id: binExpr.id, condition: "@operator='+'" }
      ),
      map
    );
    expect(result).toBe("//class[@name='Calculator']/method[.//binary_expression[@operator='+']]");
  });

  it('multiple uncle subtrees at different levels', () => {
    // Tree structure:
    // class
    // ├── className (uncle at class level)
    // └── method
    //     ├── methodName (uncle at method level)
    //     ├── params
    //     │   └── param (uncle at method level, deeper)
    //     └── body (TARGET)
    const xml = `
      <class>
        <className>MyClass</className>
        <method>
          <methodName>doSomething</methodName>
          <params>
            <param type="int"/>
          </params>
          <body>
            <return/>
          </body>
        </method>
      </class>
    `;
    const tree = parseXmlToTree(xml)!;
    const map = buildNodeInfoMap(tree);
    const className = findNode(tree, 'className')!;
    const methodName = findNode(tree, 'methodName')!;
    const param = findNode(tree, 'param')!;
    const body = findNode(tree, 'body')!;

    // Select uncles at different levels, all with conditions
    const result = buildQuery(
      tree,
      select(
        { id: className.id, condition: ".='MyClass'" },
        { id: methodName.id, condition: ".='doSomething'" },
        { id: param.id, condition: "@type='int'" },
        { id: body.id, isTarget: true }
      ),
      map
    );
    // class gets className predicate, method gets methodName and param predicates
    // method is direct child of class, so /method not //method
    expect(result).toBe("//class[className[.='MyClass']]/method[methodName[.='doSomething']][.//param[@type='int']]/body");
  });

  it('multiple uncles with same common ancestor', () => {
    // Tree where two sibling uncles share the same common ancestor with target
    const xml = `
      <method>
        <name>foo</name>
        <returns>int</returns>
        <body/>
      </method>
    `;
    const tree = parseXmlToTree(xml)!;
    const map = buildNodeInfoMap(tree);
    const name = findNode(tree, 'name')!;
    const returns = findNode(tree, 'returns')!;
    const body = findNode(tree, 'body')!;

    const result = buildQuery(
      tree,
      select(
        { id: name.id, condition: ".='foo'" },
        { id: returns.id, condition: ".='int'" },
        { id: body.id, isTarget: true }
      ),
      map
    );
    // Both name and returns are predicates on method
    expect(result).toBe("//method[name[.='foo']][returns[.='int']]/body");
  });
});
