/**
 * Path-based query builder
 *
 * Works purely with path arrays - no tree traversal needed.
 * Each path is an array of node names like ['class', 'body', 'method', 'name'].
 */

import { keyToPath, pathToKey } from './xmlTree';
import { SelectionState } from './queryState';

export interface PathSelection {
  path: string[];
  isTarget?: boolean;
  condition?: string;
}

/**
 * Build XPath query from path-based selections.
 */
export function buildQueryFromPaths(selections: PathSelection[]): string {
  if (selections.length === 0) return '';

  // Find target path (explicit or computed)
  const targetResult = findTargetPath(selections);
  if (!targetResult || targetResult.path.length === 0) return '';

  const targetPath = targetResult.path;
  const isImplicitTarget = targetResult.isImplicit;

  // Find the explicit target selection (if any) for its condition
  const targetSelection = selections.find(s => arraysEqual(s.path, targetPath));

  // Categorize all selections relative to target
  const ancestors: PathSelection[] = [];      // Prefixes of target path
  const descendants: PathSelection[] = [];    // Extensions of target path
  const uncles: PathSelection[] = [];         // Neither prefix nor extension

  for (const sel of selections) {
    if (arraysEqual(sel.path, targetPath)) {
      // This is the target itself, skip (handled separately)
      continue;
    }

    if (isStrictPrefix(sel.path, targetPath)) {
      ancestors.push(sel);
    } else if (isStrictPrefix(targetPath, sel.path)) {
      descendants.push(sel);
    } else {
      // For implicit targets (LCA), nodes that would be "uncles" of the LCA
      // but are actually descendants of LCA should be treated as descendants
      if (isImplicitTarget && isPrefix(targetPath, sel.path)) {
        descendants.push(sel);
      } else {
        uncles.push(sel);
      }
    }
  }

  // Sort ancestors by depth (shallowest first)
  ancestors.sort((a, b) => a.path.length - b.path.length);

  // Group uncles by their attachment point (common prefix length with target)
  // The uncle attaches to the node at commonPrefixLen position in the target path
  const unclesByDepth = new Map<number, PathSelection[]>();
  for (const uncle of uncles) {
    const commonLen = commonPrefixLength(uncle.path, targetPath);
    const list = unclesByDepth.get(commonLen) || [];
    list.push(uncle);
    unclesByDepth.set(commonLen, list);
  }

  // Build the XPath string
  let xpath = '';
  let lastOutputDepth = 0;  // Track what depth we've output to

  // Output ancestors in order
  for (const anc of ancestors) {
    const ancDepth = anc.path.length;
    const nodeName = anc.path[ancDepth - 1];

    // Separator: // if first or gap > 1, / if direct child
    const sep = lastOutputDepth === 0 ? '//' :
                (ancDepth === lastOutputDepth + 1 ? '/' : '//');

    xpath += sep + nodeName;

    // Add any uncle predicates that attach at this depth
    const unclesHere = unclesByDepth.get(ancDepth);
    if (unclesHere) {
      for (const uncle of unclesHere) {
        xpath += buildUncleOrDescendantPredicate(uncle.path, ancDepth, uncle.condition);
      }
    }

    // Add ancestor's own condition
    if (anc.condition) {
      xpath += `[${anc.condition}]`;
    }

    lastOutputDepth = ancDepth;
  }

  // Check for uncles that attach between last ancestor and target
  // These need implicit path nodes
  for (let depth = lastOutputDepth + 1; depth < targetPath.length; depth++) {
    const unclesHere = unclesByDepth.get(depth);
    if (unclesHere && unclesHere.length > 0) {
      // Need to output the intermediate node to attach uncles
      const nodeName = targetPath[depth - 1];
      const sep = lastOutputDepth === 0 ? '//' :
                  (depth === lastOutputDepth + 1 ? '/' : '//');
      xpath += sep + nodeName;

      for (const uncle of unclesHere) {
        xpath += buildUncleOrDescendantPredicate(uncle.path, depth, uncle.condition);
      }

      lastOutputDepth = depth;
    }
  }

  // Output target
  const targetDepth = targetPath.length;
  const targetName = targetPath[targetDepth - 1];
  const sep = lastOutputDepth === 0 ? '//' :
              (targetDepth === lastOutputDepth + 1 ? '/' : '//');
  xpath += sep + targetName;

  // Add target's own condition
  if (targetSelection?.condition) {
    xpath += `[${targetSelection.condition}]`;
  }

  // Add descendant predicates
  // Group descendants that form chains (ancestor-descendant relationships)
  const descChains = buildDescendantChains(descendants, targetPath.length);
  for (const chain of descChains) {
    xpath += buildChainPredicate(chain, targetPath.length);
  }

  return xpath;
}

/**
 * Build a predicate for an uncle or descendant node.
 */
function buildUncleOrDescendantPredicate(
  path: string[],
  attachDepth: number,
  condition?: string
): string {
  const relPath = path.slice(attachDepth);
  const isDirectChild = relPath.length === 1;

  let inner = isDirectChild ? relPath[0] : './/' + relPath[relPath.length - 1];

  if (condition) {
    inner += `[${condition}]`;
  }

  return `[${inner}]`;
}

/**
 * Build chains of descendants that are ancestor/descendant of each other.
 */
function buildDescendantChains(
  descendants: PathSelection[],
  _targetDepth: number
): PathSelection[][] {
  if (descendants.length === 0) return [];

  // Sort by depth
  const sorted = [...descendants].sort((a, b) => a.path.length - b.path.length);
  const used = new Set<PathSelection>();
  const chains: PathSelection[][] = [];

  for (const desc of sorted) {
    if (used.has(desc)) continue;

    const chain: PathSelection[] = [desc];
    used.add(desc);

    // Find descendants of this node
    let current = desc;
    for (const candidate of sorted) {
      if (used.has(candidate)) continue;
      if (isStrictPrefix(current.path, candidate.path)) {
        chain.push(candidate);
        used.add(candidate);
        current = candidate;
      }
    }

    chains.push(chain);
  }

  return chains;
}

/**
 * Build predicate for a chain of descendants.
 */
function buildChainPredicate(chain: PathSelection[], targetDepth: number): string {
  if (chain.length === 0) return '';

  let predPath = '';
  let prevDepth = targetDepth;

  for (let i = 0; i < chain.length; i++) {
    const node = chain[i];
    const nodeDepth = node.path.length;
    const nodeName = node.path[nodeDepth - 1];

    if (i === 0) {
      // First in chain - relative to target
      const isDirectChild = nodeDepth === prevDepth + 1;
      predPath = isDirectChild ? nodeName : './/' + nodeName;
    } else {
      // Continuing chain
      const isDirectChild = nodeDepth === prevDepth + 1;
      predPath += isDirectChild ? '/' + nodeName : '//' + nodeName;
    }

    if (node.condition) {
      predPath += `[${node.condition}]`;
    }

    prevDepth = nodeDepth;
  }

  return `[${predPath}]`;
}

/**
 * Find the target path - explicit or computed based on selection topology.
 *
 * Returns { path, isImplicit } where:
 * - path: the target path
 * - isImplicit: true if the target is not explicitly selected (LCA used as implicit target)
 */
interface TargetResult {
  path: string[];
  isImplicit: boolean;
}

function findTargetPath(selections: PathSelection[]): TargetResult | null {
  // Check for explicit target
  const explicit = selections.find(s => s.isTarget);
  if (explicit) return { path: explicit.path, isImplicit: false };

  if (selections.length === 0) return null;
  if (selections.length === 1) return { path: selections[0].path, isImplicit: false };

  // Filter to leaf selections (those without selected descendants)
  const leafSelections = selections.filter(sel =>
    !selections.some(other => other !== sel && isStrictPrefix(sel.path, other.path))
  );

  if (leafSelections.length === 0) return { path: selections[0].path, isImplicit: false };
  if (leafSelections.length === 1) return { path: leafSelections[0].path, isImplicit: false };

  // Find LCA (common prefix) of leaf selections
  const paths = leafSelections.map(s => s.path);
  const lca = findCommonPrefix(paths);

  // Check if paths are LINEAR (one path contains all others as prefixes)
  if (checkIfLinear(paths)) {
    // Linear: use deepest leaf as target
    const deepest = leafSelections.reduce((a, b) =>
      b.path.length > a.path.length ? b : a
    );
    return { path: deepest.path, isImplicit: false };
  }

  // Check if all leaves are SIBLINGS (same depth, same parent = LCA)
  const areSiblings = paths.every(p => p.length === paths[0].length) &&
                      paths.every(p => arraysEqual(p.slice(0, -1), lca));

  if (areSiblings) {
    // Siblings: prefer one WITHOUT condition as target, else first
    const noCondition = leafSelections.find(s => !s.condition);
    const target = noCondition || leafSelections[0];
    return { path: target.path, isImplicit: false };
  }

  // BRANCHING: leaves are in different subtrees at different depths
  // Return LCA as implicit target - all leaves become predicates
  return { path: lca, isImplicit: true };
}

/**
 * Check if paths form a linear chain (each is prefix of the next).
 */
function checkIfLinear(paths: string[][]): boolean {
  const sorted = [...paths].sort((a, b) => a.length - b.length);

  for (let i = 0; i < sorted.length - 1; i++) {
    const shorter = sorted[i];
    const longer = sorted[i + 1];
    // shorter must be prefix of longer (not strict - can be equal)
    if (!isPrefix(shorter, longer)) {
      return false;
    }
  }
  return true;
}

/**
 * Check if `prefix` is a prefix of `path` (can be equal).
 */
function isPrefix(prefix: string[], path: string[]): boolean {
  if (prefix.length > path.length) return false;
  return prefix.every((name, i) => path[i] === name);
}

/**
 * Check if `prefix` is a strict prefix of `path` (not equal).
 */
function isStrictPrefix(prefix: string[], path: string[]): boolean {
  if (prefix.length >= path.length) return false;
  return prefix.every((name, i) => path[i] === name);
}

/**
 * Check if two arrays are equal.
 */
function arraysEqual(a: string[], b: string[]): boolean {
  if (a.length !== b.length) return false;
  return a.every((val, i) => val === b[i]);
}

/**
 * Find the common prefix of multiple paths.
 */
function findCommonPrefix(paths: string[][]): string[] {
  if (paths.length === 0) return [];
  if (paths.length === 1) return paths[0];

  const prefix: string[] = [];
  const minLen = Math.min(...paths.map(p => p.length));

  for (let i = 0; i < minLen; i++) {
    const name = paths[0][i];
    if (paths.every(p => p[i] === name)) {
      prefix.push(name);
    } else {
      break;
    }
  }

  return prefix;
}

/**
 * Get the length of common prefix between two paths.
 */
function commonPrefixLength(a: string[], b: string[]): number {
  let len = 0;
  const minLen = Math.min(a.length, b.length);
  for (let i = 0; i < minLen; i++) {
    if (a[i] === b[i]) len++;
    else break;
  }
  return len;
}

/**
 * Convert SelectionState (keyed by path strings) to PathSelection array.
 */
export function selectionStateToPathSelections(state: SelectionState): PathSelection[] {
  const selections: PathSelection[] = [];

  for (const [pathKey, nodeState] of state) {
    if (!nodeState.selected) continue;

    selections.push({
      path: keyToPath(pathKey),
      isTarget: nodeState.isTarget,
      condition: nodeState.condition,
    });
  }

  return selections;
}

/**
 * Get the effective target path for a selection state.
 */
export function getEffectiveTargetPath(state: SelectionState): string | null {
  const selections = selectionStateToPathSelections(state);
  const result = findTargetPath(selections);
  return result ? pathToKey(result.path) : null;
}

/**
 * Main entry point: build query from selection state.
 */
export function buildQuery(state: SelectionState): string {
  const selections = selectionStateToPathSelections(state);
  return buildQueryFromPaths(selections);
}
