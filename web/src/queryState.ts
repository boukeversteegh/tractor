import { SerializedNode } from './parser';

// Unique ID for each node based on position
export function getNodeId(node: SerializedNode): string {
  return `${node.kind}:${node.startByte}:${node.endByte}`;
}

// Selection state for a node
export interface NodeState {
  selected: boolean;      // Part of query path
  isTarget: boolean;      // The output node (what gets returned)
  condition?: string;     // Text predicate like .='value'
}

// Map of nodeId -> state
export type SelectionState = Map<string, NodeState>;

// Get path from root to a specific node
export function getPathToNode(
  root: SerializedNode,
  targetId: string,
  path: SerializedNode[] = []
): SerializedNode[] | null {
  const currentId = getNodeId(root);
  const currentPath = [...path, root];

  if (currentId === targetId) {
    return currentPath;
  }

  for (const child of root.children) {
    if (child.isNamed) {
      const result = getPathToNode(child, targetId, currentPath);
      if (result) return result;
    }
  }

  return null;
}

// Check if nodeA is an ancestor of nodeB
export function isAncestor(
  root: SerializedNode,
  ancestorId: string,
  descendantId: string
): boolean {
  const pathToDescendant = getPathToNode(root, descendantId);
  if (!pathToDescendant) return false;

  return pathToDescendant.some(n => getNodeId(n) === ancestorId);
}

// Check if nodeA is a direct parent of nodeB
export function isDirectParent(
  root: SerializedNode,
  parentId: string,
  childId: string
): boolean {
  const pathToChild = getPathToNode(root, childId);
  if (!pathToChild || pathToChild.length < 2) return false;

  const parent = pathToChild[pathToChild.length - 2];
  return getNodeId(parent) === parentId;
}

// Get the element name for XPath (fieldName or kind)
function getElementName(node: SerializedNode): string {
  return node.fieldName || node.kind;
}

// Build XPath from selection state
export function buildXPathFromState(
  root: SerializedNode,
  state: SelectionState
): string {
  // Find all selected nodes and the target
  const selectedNodes: { node: SerializedNode; nodeState: NodeState; depth: number }[] = [];
  let targetNode: SerializedNode | null = null;

  function traverse(node: SerializedNode, depth: number) {
    if (!node.isNamed) {
      for (const child of node.children) {
        traverse(child, depth);
      }
      return;
    }

    const nodeId = getNodeId(node);
    const nodeState = state.get(nodeId);

    if (nodeState?.selected) {
      selectedNodes.push({ node, nodeState, depth });
    }

    if (nodeState?.isTarget) {
      targetNode = node;
    }

    for (const child of node.children) {
      if (child.isNamed) {
        traverse(child, depth + 1);
      }
    }
  }

  traverse(root, 0);

  if (selectedNodes.length === 0) {
    return '';
  }

  // If no explicit target, use the deepest selected node
  if (!targetNode) {
    const deepest = selectedNodes.reduce((a, b) => a.depth > b.depth ? a : b);
    targetNode = deepest.node;
  }

  const targetId = getNodeId(targetNode);

  // Separate nodes into path nodes (ancestors of target) and condition nodes (descendants)
  const pathNodes: typeof selectedNodes = [];
  const conditionNodes: typeof selectedNodes = [];

  for (const item of selectedNodes) {
    const itemId = getNodeId(item.node);
    if (itemId === targetId) {
      pathNodes.push(item);
    } else if (isAncestor(root, itemId, targetId)) {
      pathNodes.push(item);
    } else if (isAncestor(root, targetId, itemId)) {
      conditionNodes.push(item);
    }
  }

  // Sort path nodes by depth
  pathNodes.sort((a, b) => a.depth - b.depth);

  // Build the main path
  let xpath = '';
  let prevNode: SerializedNode | null = null;

  for (const { node, nodeState } of pathNodes) {
    const elementName = getElementName(node);
    const nodeId = getNodeId(node);

    // Determine separator
    let separator = '//';
    if (prevNode) {
      if (isDirectParent(root, getNodeId(prevNode), nodeId)) {
        separator = '/';
      }
    }

    xpath += separator + elementName;

    // Add text condition if present
    if (nodeState.condition) {
      xpath += `[${nodeState.condition}]`;
    }

    prevNode = node;
  }

  // Add descendant conditions (nodes selected below the target)
  for (const { node, nodeState } of conditionNodes) {
    const elementName = getElementName(node);
    const conditionPath = isDirectParent(root, targetId, getNodeId(node))
      ? elementName
      : `.//${elementName}`;

    let condition = conditionPath;
    if (nodeState.condition) {
      condition += `[${nodeState.condition}]`;
    }
    xpath += `[${condition}]`;
  }

  return xpath || '//';
}

// Helper to create initial empty state
export function createEmptyState(): SelectionState {
  return new Map();
}

// Toggle selection on a node
export function toggleSelection(
  state: SelectionState,
  nodeId: string
): SelectionState {
  const newState = new Map(state);
  const current = newState.get(nodeId);

  if (current?.selected) {
    // Deselect
    if (current.isTarget) {
      newState.delete(nodeId);
    } else {
      newState.set(nodeId, { ...current, selected: false });
      if (!current.condition) {
        newState.delete(nodeId);
      }
    }
  } else {
    // Select
    newState.set(nodeId, {
      ...current,
      selected: true,
      isTarget: current?.isTarget ?? false
    });
  }

  return newState;
}

// Set a node as the target
export function setTarget(
  state: SelectionState,
  nodeId: string
): SelectionState {
  const newState = new Map(state);

  // Clear previous target
  for (const [id, nodeState] of newState) {
    if (nodeState.isTarget) {
      newState.set(id, { ...nodeState, isTarget: false });
    }
  }

  // Set new target (also selects it)
  const current = newState.get(nodeId);
  newState.set(nodeId, {
    ...current,
    selected: true,
    isTarget: true
  });

  return newState;
}

// Add a condition to a node
export function addCondition(
  state: SelectionState,
  nodeId: string,
  condition: string
): SelectionState {
  const newState = new Map(state);
  const current = newState.get(nodeId);

  newState.set(nodeId, {
    ...current,
    selected: current?.selected ?? true,
    isTarget: current?.isTarget ?? false,
    condition,
  });

  return newState;
}

// Clear all selections
export function clearSelection(): SelectionState {
  return new Map();
}
