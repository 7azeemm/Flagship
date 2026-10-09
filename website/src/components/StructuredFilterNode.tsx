import { useEffect, useId, useRef, useState } from 'react';
import { ChevronDown } from 'lucide-react';
import type { FilterValue } from '../types';
import { filterValue, isWithin, nodeStatus } from '../lib/catalog';

interface Props {
  node: FilterValue;
  depth?: number;
  filterKey: string;
  selectedValues: string[];
  onToggle: (key: string, node: FilterValue) => void;
}
export function StructuredFilterNode({
  node,
  filterKey,
  selectedValues,
  onToggle,
  depth = 0,
}: Props) {
  const value = filterValue(node);
  const [expanded, setExpanded] = useState(() =>
    selectedValues.some((rule) => isWithin(rule.replace(/^!/, ''), value)),
  );
  const checkbox = useRef<HTMLInputElement>(null);
  const id = useId();
  const status = nodeStatus(node, selectedValues);
  const hasChildren = !!node.children?.length;
  useEffect(() => {
    if (checkbox.current) checkbox.current.indeterminate = status === 'partial';
  }, [status]);
  return (
    <div className="filter-node">
      {hasChildren ? (
        <div
          className="filter-node-row filter-node-parent"
          style={{ paddingLeft: Math.min(depth, 4) * 8 }}
        >
          <input
            ref={checkbox}
            type="checkbox"
            aria-label={node.label}
            checked={status === 'checked'}
            onChange={() => onToggle(filterKey, node)}
          />
          <button
            type="button"
            className="filter-parent-toggle"
            aria-expanded={expanded}
            aria-controls={id}
            onClick={() => setExpanded(!expanded)}
          >
            <span>{node.label}</span>
            <small className="filter-count">{node.count}</small>
            <ChevronDown size={15} />
          </button>
        </div>
      ) : (
        <div className="filter-node-row">
          <label
            className="checkbox-row"
            title={node.label}
            style={{ paddingLeft: Math.min(depth, 4) * 8 }}
          >
            <input
              ref={checkbox}
              type="checkbox"
              checked={status === 'checked'}
              onChange={() => onToggle(filterKey, node)}
            />
            <span>{node.label}</span>
          </label>
          <small className="filter-count">{node.count}</small>
          <span className="tree-toggle-spacer" />
        </div>
      )}
      {hasChildren && expanded && (
        <div id={id} className="filter-children">
          {node.children?.map((child) => (
            <StructuredFilterNode
              key={filterValue(child)}
              depth={depth + 1}
              node={child}
              filterKey={filterKey}
              selectedValues={selectedValues}
              onToggle={onToggle}
            />
          ))}
        </div>
      )}
    </div>
  );
}
