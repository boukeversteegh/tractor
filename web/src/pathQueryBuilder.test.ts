/**
 * Unit tests for path-based queryBuilder
 * @vitest-environment jsdom
 */

import { describe, it, expect } from 'vitest';
import { buildQueryFromPaths, PathSelection } from './pathQueryBuilder';

// Helper to create selections from path strings
function select(
  ...selections: Array<{ path: string; isTarget?: boolean; condition?: string }>
): PathSelection[] {
  return selections.map(s => ({
    path: s.path.split('/'),
    isTarget: s.isTarget,
    condition: s.condition,
  }));
}

describe('buildQueryFromPaths', () => {
  it('returns empty string for no selection', () => {
    const result = buildQueryFromPaths([]);
    expect(result).toBe('');
  });

  it('single node selection uses //', () => {
    const result = buildQueryFromPaths(select({ path: 'class/method' }));
    expect(result).toBe('//method');
  });

  it('direct parent-child uses /', () => {
    const result = buildQueryFromPaths(
      select(
        { path: 'class' },
        { path: 'class/method', isTarget: true }
      )
    );
    expect(result).toBe('//class/method');
  });

  it('distant ancestor uses //', () => {
    const result = buildQueryFromPaths(
      select(
        { path: 'class' },
        { path: 'class/method/parameter', isTarget: true }
      )
    );
    expect(result).toBe('//class//parameter');
  });

  it('descendant of target becomes predicate (direct child)', () => {
    const result = buildQueryFromPaths(
      select(
        { path: 'class/method', isTarget: true },
        { path: 'class/method/parameter' }
      )
    );
    expect(result).toBe('//method[parameter]');
  });

  it('descendant of target becomes predicate (distant)', () => {
    const result = buildQueryFromPaths(
      select(
        { path: 'class', isTarget: true },
        { path: 'class/method/parameter' }
      )
    );
    expect(result).toBe('//class[.//parameter]');
  });

  it('condition on target node', () => {
    const result = buildQueryFromPaths(
      select({ path: 'class/method', condition: "@name='doSomething'" })
    );
    expect(result).toBe("//method[@name='doSomething']");
  });

  it('condition on ancestor node', () => {
    const result = buildQueryFromPaths(
      select(
        { path: 'class', condition: "@name='MyClass'" },
        { path: 'class/method', isTarget: true }
      )
    );
    expect(result).toBe("//class[@name='MyClass']/method");
  });

  it('condition on descendant predicate', () => {
    const result = buildQueryFromPaths(
      select(
        { path: 'class/method', isTarget: true },
        { path: 'class/method/parameter', condition: "@name='x'" }
      )
    );
    expect(result).toBe("//method[parameter[@name='x']]");
  });

  it('multiple ancestors in path', () => {
    const result = buildQueryFromPaths(
      select(
        { path: 'class' },
        { path: 'class/method' },
        { path: 'class/method/body/return', isTarget: true }
      )
    );
    expect(result).toBe('//class/method//return');
  });

  it('multiple descendant predicates (siblings)', () => {
    const result = buildQueryFromPaths(
      select(
        { path: 'class/method', isTarget: true },
        { path: 'class/method/parameter' },
        { path: 'class/method/body' }
      )
    );
    expect(result).toBe('//method[parameter][body]');
  });

  it('chained descendant predicates form a path', () => {
    const result = buildQueryFromPaths(
      select(
        { path: 'class/method', isTarget: true },
        { path: 'class/method/body' },
        { path: 'class/method/body/return' }
      )
    );
    expect(result).toBe('//method[body/return]');
  });

  it('uses deepest node as target when none specified (linear path)', () => {
    const result = buildQueryFromPaths(
      select(
        { path: 'class' },
        { path: 'class/method/parameter' }
      )
    );
    // parameter is deepest, class is ancestor
    expect(result).toBe('//class//parameter');
  });

  it('uses LCA as default target when selection branches', () => {
    // Select nodes in different branches under method
    const result = buildQueryFromPaths(
      select(
        { path: 'class/method/parameter' },
        { path: 'class/method/body/return' }
      )
    );
    // LCA is class/method (even though not explicitly selected!)
    // parameter and return are in different branches
    expect(result).toBe('//method[parameter][.//return]');
  });

  it('uncle nodes become predicates on common ancestor', () => {
    // name and body are siblings under method
    // body is target, name is uncle
    const result = buildQueryFromPaths(
      select(
        { path: 'class/method/name', condition: ".='Hello'" },
        { path: 'class/method/body', isTarget: true }
      )
    );
    expect(result).toBe("//method[name[.='Hello']]/body");
  });

  it('uncle nodes: selecting deeper target with sibling condition', () => {
    const result = buildQueryFromPaths(
      select(
        { path: 'class/method/name', condition: ".='Hello'" },
        { path: 'class/method/body/block', isTarget: true }
      )
    );
    expect(result).toBe("//method[name[.='Hello']]//block");
  });

  it('complex: ancestor with condition, target, descendant predicate with condition', () => {
    const result = buildQueryFromPaths(
      select(
        { path: 'class', condition: "@name='Calculator'" },
        { path: 'class/method', isTarget: true },
        { path: 'class/method/body/return/binary_expression', condition: "@operator='+'" }
      )
    );
    expect(result).toBe("//class[@name='Calculator']/method[.//binary_expression[@operator='+']]");
  });

  it('multiple uncle subtrees at different levels', () => {
    const result = buildQueryFromPaths(
      select(
        { path: 'class/className', condition: ".='MyClass'" },
        { path: 'class/method/methodName', condition: ".='doSomething'" },
        { path: 'class/method/params/param', condition: "@type='int'" },
        { path: 'class/method/body', isTarget: true }
      )
    );
    expect(result).toBe("//class[className[.='MyClass']]/method[methodName[.='doSomething']][.//param[@type='int']]/body");
  });

  it('multiple uncles with same common ancestor', () => {
    const result = buildQueryFromPaths(
      select(
        { path: 'method/name', condition: ".='foo'" },
        { path: 'method/returns', condition: ".='int'" },
        { path: 'method/body', isTarget: true }
      )
    );
    expect(result).toBe("//method[name[.='foo']][returns[.='int']]/body");
  });

  it('siblings without selected ancestor use LCA', () => {
    // Select only siblings - their parent (method) should be inferred as context
    const result = buildQueryFromPaths(
      select(
        { path: 'class/method/name', condition: ".='Hello'" },
        { path: 'class/method/body' }
      )
    );
    // LCA is class/method, both are children
    // With body as the deepest leaf, it becomes the target
    expect(result).toBe("//method[name[.='Hello']]/body");
  });

  it('ancestor selected with descendants uses deepest as LCA', () => {
    // When class AND its descendants are selected, LCA should be computed from leaves
    const result = buildQueryFromPaths(
      select(
        { path: 'class' },
        { path: 'class/method' },
        { path: 'class/method/name', condition: ".='Hello'" },
        { path: 'class/method/body' }
      )
    );
    // Leaves are name and body, LCA is method
    // class and method become ancestors in path
    expect(result).toBe("//class/method[name[.='Hello']]/body");
  });
});
