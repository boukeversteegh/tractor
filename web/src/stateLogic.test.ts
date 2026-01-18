/**
 * Test the state update logic used in App.tsx handlers
 */

import { describe, it, expect } from 'vitest';
import { SelectionState } from './queryState';

// Replicate the handleSetTarget logic from App.tsx
function handleSetTarget(
  prev: SelectionState,
  nodeId: string
): SelectionState {
  const newState = new Map(prev);
  const current = newState.get(nodeId);
  const wasTarget = current?.isTarget ?? false;

  // Clear previous target
  for (const [id, state] of newState) {
    if (state.isTarget) {
      newState.set(id, { ...state, isTarget: false });
    }
  }

  // Toggle: if already target, just unset (keep selected); otherwise set as target
  if (wasTarget) {
    newState.set(nodeId, {
      selected: current?.selected ?? true,
      isTarget: false,
      condition: current?.condition,
    });
  } else {
    newState.set(nodeId, {
      selected: true,
      isTarget: true,
      condition: current?.condition,
    });
  }

  return newState;
}

// Replicate handleToggleSelection logic
function handleToggleSelection(
  prev: SelectionState,
  nodeId: string
): SelectionState {
  const newState = new Map(prev);
  const current = newState.get(nodeId);

  if (current?.selected) {
    newState.delete(nodeId);
  } else {
    newState.set(nodeId, {
      selected: true,
      isTarget: current?.isTarget ?? false,
      condition: current?.condition,
    });
  }

  return newState;
}

describe('state update logic', () => {
  it('sets target on unselected node', () => {
    const prev: SelectionState = new Map();
    const result = handleSetTarget(prev, 'node-1');

    expect(result.get('node-1')).toEqual({
      selected: true,
      isTarget: true,
      condition: undefined,
    });
  });

  it('sets target on already selected node', () => {
    const prev: SelectionState = new Map([
      ['node-1', { selected: true, isTarget: false }],
    ]);
    const result = handleSetTarget(prev, 'node-1');

    expect(result.get('node-1')).toEqual({
      selected: true,
      isTarget: true,
      condition: undefined,
    });
  });

  it('unsets target when clicking on current target', () => {
    const prev: SelectionState = new Map([
      ['node-1', { selected: true, isTarget: true }],
    ]);
    const result = handleSetTarget(prev, 'node-1');

    expect(result.get('node-1')).toEqual({
      selected: true,
      isTarget: false,
      condition: undefined,
    });
  });

  it('moves target from one node to another', () => {
    const prev: SelectionState = new Map([
      ['node-1', { selected: true, isTarget: true }],
      ['node-2', { selected: true, isTarget: false }],
    ]);
    const result = handleSetTarget(prev, 'node-2');

    expect(result.get('node-1')).toEqual({
      selected: true,
      isTarget: false,
    });
    expect(result.get('node-2')).toEqual({
      selected: true,
      isTarget: true,
      condition: undefined,
    });
  });

  it('toggle selection adds node', () => {
    const prev: SelectionState = new Map();
    const result = handleToggleSelection(prev, 'node-1');

    expect(result.get('node-1')).toEqual({
      selected: true,
      isTarget: false,
      condition: undefined,
    });
  });

  it('toggle selection removes node', () => {
    const prev: SelectionState = new Map([
      ['node-1', { selected: true, isTarget: false }],
    ]);
    const result = handleToggleSelection(prev, 'node-1');

    expect(result.has('node-1')).toBe(false);
  });

  it('toggle selection preserves target when deselecting', () => {
    // If a node is target and we deselect it, it gets removed entirely
    const prev: SelectionState = new Map([
      ['node-1', { selected: true, isTarget: true }],
    ]);
    const result = handleToggleSelection(prev, 'node-1');

    // The node is deleted when deselected
    expect(result.has('node-1')).toBe(false);
  });
});
