/**
 * Query builder - constructs XPath from tree selection state
 */

import { XmlNode, getPathToNode } from './xmlTree';
import { SelectionState } from './queryState';

interface SelectedNode {
  id: string;
  name: string;
  condition?: string;
  isTarget: boolean;
}

/**
 * Build XPath query from selection state and tree structure
 *
 * Rules:
 * - Selected ancestors of target go in the path (//ancestor//target)
 * - Selected descendants of target become predicates (//target[descendant])
 * - Conditions on descendants use the descendant as context (//target[name[.='foo']])
 * - Direct parent-child uses /, otherwise //
 */
export function buildQuery(
  tree: XmlNode | null,
  selectionState: SelectionState,
  nodeInfoMap: Map<string, { name: string }>
): string {
  if (!tree || selectionState.size === 0) return '';

  // Collect selected nodes with their info
  const selectedNodes: SelectedNode[] = [];
  let targetId: string | null = null;

  for (const [id, state] of selectionState) {
    if (!state.selected) continue;

    const info = nodeInfoMap.get(id);
    if (!info) continue;

    selectedNodes.push({
      id,
      name: info.name,
      condition: state.condition,
      isTarget: state.isTarget,
    });

    if (state.isTarget) {
      targetId = id;
    }
  }

  if (selectedNodes.length === 0) return '';

  // If no explicit target, use LCA of selected nodes (or deepest if no branching)
  if (!targetId) {
    const defaultTarget = findDefaultTarget(tree, selectedNodes);
    if (defaultTarget) {
      targetId = defaultTarget.id;
    } else {
      targetId = selectedNodes[0].id;
    }
  }

  // Get path from root to target
  const pathToTarget = getPathToNode(tree, targetId) || [];
  const pathIds = new Set(pathToTarget);

  // Separate into:
  // - pathNodes: ancestors of target (go in main path)
  // - predicateNodes: descendants of target (become predicates on target)
  // - uncleNodes: neither ancestor nor descendant (become predicates on their common ancestor with target)
  const pathNodes: SelectedNode[] = [];
  const predicateNodes: SelectedNode[] = [];
  const uncleNodes: { node: SelectedNode; commonAncestorId: string }[] = [];

  for (const node of selectedNodes) {
    if (pathIds.has(node.id)) {
      pathNodes.push(node);
    } else {
      // Check if this node is a descendant of the target
      const pathToNode = getPathToNode(tree, node.id) || [];
      if (pathToNode.some(id => id === targetId)) {
        predicateNodes.push(node);
      } else {
        // Uncle node - find common ancestor with target
        const commonAncestorId = findCommonAncestor(pathToTarget, pathToNode);
        if (commonAncestorId) {
          uncleNodes.push({ node, commonAncestorId });
        }
      }
    }
  }

  // Ensure common ancestors of uncle nodes are included in the path
  // Group uncle nodes by their common ancestor
  const unclesByAncestor = new Map<string, SelectedNode[]>();
  for (const { node, commonAncestorId } of uncleNodes) {
    const existing = unclesByAncestor.get(commonAncestorId) || [];
    existing.push(node);
    unclesByAncestor.set(commonAncestorId, existing);
  }

  // Add implicit path nodes for ancestors that have uncle predicates but aren't selected
  const implicitPathNodes: { id: string; name: string }[] = [];
  for (const ancestorId of unclesByAncestor.keys()) {
    if (!pathNodes.some(n => n.id === ancestorId)) {
      const info = nodeInfoMap.get(ancestorId);
      if (info) {
        implicitPathNodes.push({ id: ancestorId, name: info.name });
      }
    }
  }

  // Combine explicit and implicit path nodes
  type PathEntry = { id: string; name: string; condition?: string; isTarget: boolean };
  const allPathNodes: PathEntry[] = [
    ...pathNodes,
    ...implicitPathNodes.map(n => ({ ...n, isTarget: false })),
  ];

  // Sort path nodes by their position in the path
  allPathNodes.sort((a, b) => {
    const aIdx = pathToTarget.indexOf(a.id);
    const bIdx = pathToTarget.indexOf(b.id);
    return aIdx - bIdx;
  });

  // Build the main path
  let xpath = '';
  let prevId: string | null = null;

  for (const node of allPathNodes) {
    // Determine separator
    let sep = '//';
    if (prevId !== null) {
      // Check if prevId is direct parent of node.id
      const pathToNode = getPathToNode(tree, node.id) || [];
      const prevIdx = pathToNode.indexOf(prevId);
      const nodeIdx = pathToNode.indexOf(node.id);
      if (prevIdx >= 0 && nodeIdx >= 0 && nodeIdx === prevIdx + 1) {
        sep = '/';
      }
    }

    xpath += sep + node.name;

    // Add uncle predicates for this ancestor
    const unclesForThis = unclesByAncestor.get(node.id);
    if (unclesForThis) {
      for (const uncle of unclesForThis) {
        const pathToUncle = getPathToNode(tree, uncle.id) || [];
        const ancestorIdx = pathToUncle.indexOf(node.id);
        const uncleIdx = pathToUncle.indexOf(uncle.id);

        // Determine if direct child or descendant
        const isDirectChild = ancestorIdx >= 0 && uncleIdx >= 0 && uncleIdx === ancestorIdx + 1;
        const sep = isDirectChild ? '' : './/';

        if (uncle.condition) {
          xpath += `[${sep}${uncle.name}[${uncle.condition}]]`;
        } else {
          xpath += `[${sep}${uncle.name}]`;
        }
      }
    }

    // Add condition if present and this is not the target with predicates
    if (node.condition && (node.id !== targetId || predicateNodes.length === 0)) {
      xpath += `[${node.condition}]`;
    }

    prevId = node.id;
  }

  // Add predicate nodes as conditions on the target
  // Build chains from predicate nodes that are ancestors/descendants of each other
  if (predicateNodes.length > 0 && targetId) {
    // Sort predicate nodes by depth (closest to target first)
    const sortedPreds = [...predicateNodes].sort((a, b) => {
      const pathA = getPathToNode(tree, a.id) || [];
      const pathB = getPathToNode(tree, b.id) || [];
      return pathA.length - pathB.length;
    });

    // Build chains: group nodes where one is ancestor of another
    const chains: SelectedNode[][] = [];
    const used = new Set<string>();

    for (const node of sortedPreds) {
      if (used.has(node.id)) continue;

      // Start a new chain with this node
      const chain: SelectedNode[] = [node];
      used.add(node.id);

      // Find descendants of this node that are also selected
      let currentId = node.id;
      for (const candidate of sortedPreds) {
        if (used.has(candidate.id)) continue;
        const pathToCandidate = getPathToNode(tree, candidate.id) || [];
        if (pathToCandidate.includes(currentId)) {
          chain.push(candidate);
          used.add(candidate.id);
          currentId = candidate.id;
        }
      }

      chains.push(chain);
    }

    // Build predicate for each chain
    for (const chain of chains) {
      let predPath = '';
      let prevNodeId = targetId;

      for (let i = 0; i < chain.length; i++) {
        const node = chain[i];
        const pathToNode = getPathToNode(tree, node.id) || [];
        const prevIdx = pathToNode.indexOf(prevNodeId);
        const nodeIdx = pathToNode.indexOf(node.id);

        // Determine separator
        let sep: string;
        if (i === 0) {
          // First node in chain - relative to target
          if (prevIdx >= 0 && nodeIdx >= 0 && nodeIdx === prevIdx + 1) {
            sep = '';  // Direct child, no prefix needed
          } else {
            sep = './/';  // Descendant
          }
        } else {
          // Subsequent nodes - relative to previous in chain
          if (prevIdx >= 0 && nodeIdx >= 0 && nodeIdx === prevIdx + 1) {
            sep = '/';
          } else {
            sep = '//';
          }
        }

        predPath += sep + node.name;
        if (node.condition) {
          predPath += `[${node.condition}]`;
        }

        prevNodeId = node.id;
      }

      xpath += `[${predPath}]`;
    }

    // Add target's own condition if it has one
    const targetNode = pathNodes.find(n => n.id === targetId);
    if (targetNode?.condition) {
      xpath += `[${targetNode.condition}]`;
    }
  }

  return xpath || '//';
}

/**
 * Get the effective target ID for a selection.
 * Returns explicit target if set, otherwise computes default using LCA.
 */
export function getEffectiveTarget(
  tree: XmlNode | null,
  selectionState: SelectionState,
  nodeInfoMap: Map<string, { name: string }>
): string | null {
  if (!tree || selectionState.size === 0) return null;

  // Check for explicit target
  for (const [id, state] of selectionState) {
    if (state.selected && state.isTarget) {
      return id;
    }
  }

  // Collect selected nodes
  const selectedNodes: { id: string; name: string }[] = [];
  for (const [id, state] of selectionState) {
    if (!state.selected) continue;
    const info = nodeInfoMap.get(id);
    if (info) {
      selectedNodes.push({ id, name: info.name });
    }
  }

  if (selectedNodes.length === 0) return null;

  // Use LCA logic to find default target
  const defaultTarget = findDefaultTarget(tree, selectedNodes.map(n => ({
    id: n.id,
    name: n.name,
    isTarget: false,
  })));

  return defaultTarget?.id ?? null;
}

/**
 * Find the best default target node using LCA (Lowest Common Ancestor) logic.
 *
 * LINEAR SELECTION (all nodes on same path):
 *   class > method > param
 *   Select: [class, param] → target = param (deepest)
 *   Result: //class//param
 *
 * BRANCHING SELECTION (nodes in different subtrees):
 *   method > [params > param, body > return]
 *   Select: [method, param, return] → target = method (LCA)
 *   Result: //method[param][.//return]
 *
 * The LCA is the deepest selected node that is an ancestor of all other
 * selected nodes - i.e., where the selection "branches out".
 */
function findDefaultTarget(tree: XmlNode, selectedNodes: SelectedNode[]): SelectedNode | null {
  if (selectedNodes.length === 0) return null;
  if (selectedNodes.length === 1) return selectedNodes[0];

  const selectedIds = new Set(selectedNodes.map(n => n.id));

  // Get paths to all selected nodes
  const allPaths = selectedNodes.map(n => ({ node: n, path: getPathToNode(tree, n.id) || [] }));

  // Filter to "leaf" selected nodes - those that don't have selected descendants
  // This way, if class and method are both selected, we use method (not class) for LCA
  const leafNodes = allPaths.filter(({ path }) => {
    // Check if any other selected node has this node as an ancestor
    const nodeId = path[path.length - 1];
    const hasSelectedDescendant = allPaths.some(other => {
      if (other.path === path) return false;
      // Check if nodeId appears in other's path (meaning other is a descendant)
      return other.path.slice(0, -1).includes(nodeId);
    });
    return !hasSelectedDescendant;
  });

  // Use leaf nodes for LCA calculation (fall back to all if somehow empty)
  const pathsForLCA = leafNodes.length > 0 ? leafNodes : allPaths;
  const paths = pathsForLCA.map(p => p.path);

  // Find common prefix of all paths
  const commonPrefix: string[] = [];
  const minLength = Math.min(...paths.map(p => p.length));

  for (let i = 0; i < minLength; i++) {
    const nodeId = paths[0][i];
    if (paths.every(p => p[i] === nodeId)) {
      commonPrefix.push(nodeId);
    } else {
      break;
    }
  }

  // Check if this is a linear selection (all nodes on one path)
  // Linear means for any two paths, one must be a prefix of the other
  const isLinear = paths.every(p1 =>
    paths.every(p2 => {
      const shorter = p1.length <= p2.length ? p1 : p2;
      const longer = p1.length <= p2.length ? p2 : p1;
      // shorter must be a prefix of longer
      return shorter.every((id, i) => longer[i] === id);
    })
  );

  if (isLinear) {
    // Linear selection: use deepest selected node
    let deepestNode: SelectedNode | null = null;
    let deepestDepth = -1;
    for (const node of selectedNodes) {
      const path = getPathToNode(tree, node.id) || [];
      if (path.length > deepestDepth) {
        deepestDepth = path.length;
        deepestNode = node;
      }
    }
    return deepestNode;
  }

  // Branching selection: find deepest selected node in common prefix (LCA)
  for (let i = commonPrefix.length - 1; i >= 0; i--) {
    const nodeId = commonPrefix[i];
    if (selectedIds.has(nodeId)) {
      return selectedNodes.find(n => n.id === nodeId) || null;
    }
  }

  // Fallback
  return selectedNodes[0];
}

/**
 * Find the deepest common ancestor between two paths.
 * Returns the ID of the common ancestor, or null if none found.
 */
function findCommonAncestor(path1: string[], path2: string[]): string | null {
  let commonAncestor: string | null = null;
  const minLength = Math.min(path1.length, path2.length);

  for (let i = 0; i < minLength; i++) {
    if (path1[i] === path2[i]) {
      commonAncestor = path1[i];
    } else {
      break;
    }
  }

  return commonAncestor;
}
