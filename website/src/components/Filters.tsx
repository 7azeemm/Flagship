import { useEffect, useState } from 'react';
import {
  Search,
  Check,
  LockKeyhole,
  ChevronDown,
  RotateCcw,
  SlidersHorizontal,
  X,
} from 'lucide-react';
import type { CatalogQuery, SectionInfo } from '../types';
import { fieldLabel, hasFilterChanges, stockLabels, toggle, toggleNode } from '../lib/catalog';
import { RangeSlider } from './RangeSlider';
import { StructuredFilterNode } from './StructuredFilterNode';

interface Props {
  query: CatalogQuery;
  info: SectionInfo | null;
  base: SectionInfo | null;
  update: (changes: Partial<CatalogQuery>) => void;
  reset: () => void;
  disabled: boolean;
}
export function Filters({ query, info, base, update, reset, disabled }: Props) {
  const [search, setSearch] = useState(query.search);
  useEffect(() => setSearch(query.search), [query.search]);
  const [expanded, setExpanded] = useState<Record<string, boolean>>({});
  const [allRetailers, setAllRetailers] = useState(false);
  const groupFields = base?.group_by || [];
  const selectedGroupFields = groupFields
    .filter(
      (field) => field.required || query.groupBy === null || query.groupBy.includes(field.key),
    )
    .map((field) => field.key);
  const sites = [...new Set([...(base?.sites || []), ...query.site])].sort();
  const visibleSites = allRetailers
    ? sites
    : sites.filter((site, index) => index < 5 || query.site.includes(site));
  return (
    <fieldset className="filters-content" disabled={disabled}>
      <div className="filters-heading">
        <h2>
          <SlidersHorizontal size={17} /> Filters
        </h2>
        <button className="text-button" onClick={reset} disabled={!hasFilterChanges(query)}>
          <RotateCcw size={13} /> Reset
        </button>
      </div>
      {!!groupFields.length && (
        <section className="filter-section grouping-section">
          <button
            type="button"
            className="group-toggle"
            role="switch"
            aria-checked={query.grouped}
            onClick={() => update({ grouped: !query.grouped })}
          >
            <span className="toggle-track">
              <Check size={10} />
            </span>
            <span>Group offers</span>
            <strong>{query.grouped ? 'On' : 'Off'}</strong>
          </button>
          <p className="group-help">Compare matching configurations across retailers.</p>
          <div
            className="group-fields"
            aria-label="Group products by"
            aria-disabled={!query.grouped}
          >
            {groupFields.map((field) => (
              <button
                key={field.key}
                aria-pressed={selectedGroupFields.includes(field.key)}
                disabled={!query.grouped || field.required}
                title={
                  field.required ? 'Required for grouping' : `Match by ${fieldLabel(field.key)}`
                }
                onClick={() =>
                  update({
                    groupBy: selectedGroupFields.includes(field.key)
                      ? selectedGroupFields.filter((key) => key !== field.key)
                      : [...selectedGroupFields, field.key],
                  })
                }
              >
                {field.required ? (
                  <LockKeyhole size={10} />
                ) : selectedGroupFields.includes(field.key) ? (
                  <Check size={10} />
                ) : null}
                {fieldLabel(field.key)}
              </button>
            ))}
          </div>
        </section>
      )}
      <form
        className="catalog-search filter-search"
        role="search"
        onSubmit={(event) => {
          event.preventDefault();
          update({ search: search.trim() });
        }}
      >
        <Search size={15} />
        <input
          aria-label="Search this category"
          placeholder="Search this category…"
          value={search}
          onChange={(event) => setSearch(event.target.value)}
        />
        {search && (
          <button
            type="button"
            className="search-clear"
            aria-label="Clear search"
            onClick={() => {
              setSearch('');
              update({ search: '' });
            }}
          >
            <X size={14} />
          </button>
        )}
        <button type="submit" className="search-submit" aria-label="Search category">
          <Search size={14} />
        </button>
      </form>
      {base && (
        <section className="filter-section">
          <h3>
            Price range <span>DT</span>
          </h3>
          <RangeSlider
            min={base.min_price}
            max={base.max_price}
            valueMin={query.min_price ? Number(query.min_price) : base.min_price}
            valueMax={query.max_price ? Number(query.max_price) : base.max_price}
            onChange={(min, max) =>
              update({
                min_price: min === base.min_price ? '' : String(min),
                max_price: max === base.max_price ? '' : String(max),
              })
            }
          />
        </section>
      )}
      <details className="filter-section" open>
        <summary>
          Availability <ChevronDown size={15} />
        </summary>
        <div className="filter-options">
          {['InStock', 'OutOfStock', 'Arriving', 'OnRequest'].map((status) => (
            <label className="checkbox-row" key={status}>
              <input
                type="checkbox"
                checked={query.stock.includes(status)}
                onChange={() => update({ stock: toggle(query.stock, status) })}
              />
              <span>{stockLabels[status]}</span>
            </label>
          ))}
        </div>
      </details>
      <details className="filter-section" open>
        <summary>
          Retailers <ChevronDown size={15} />
        </summary>
        <div className="filter-options">
          {visibleSites.map((site) => (
            <label className="checkbox-row" key={site}>
              <input
                type="checkbox"
                checked={query.site.includes(site)}
                onChange={() => update({ site: toggle(query.site, site) })}
              />
              <span>{site}</span>
            </label>
          ))}
          {sites.length > 5 && (
            <button
              className="text-button more-retailers"
              onClick={() => setAllRetailers(!allRetailers)}
            >
              {allRetailers ? 'Show fewer' : `Show all ${sites.length} retailers`}
            </button>
          )}
        </div>
      </details>
      {(info?.filters || base?.filters || []).map((group, index) => (
        <details
          className="filter-section"
          key={group.key}
          open={expanded[group.key] ?? index === 0}
          onToggle={(event) => {
            const open = event.currentTarget.open;
            setExpanded((previous) =>
              previous[group.key] === open ? previous : { ...previous, [group.key]: open },
            );
          }}
        >
          <summary>
            {fieldLabel(group.key)}
            {query.filters[group.key]?.length ? (
              <span className="selection-count">{query.filters[group.key].length}</span>
            ) : null}
            <ChevronDown size={15} />
          </summary>
          <div className="filter-options tree-options">
            {group.options.map((node) => (
              <StructuredFilterNode
                key={node.id}
                node={node}
                filterKey={group.key}
                selectedIds={query.filters[group.key] || []}
                onToggle={(key, id) =>
                  update({
                    filters: { ...query.filters, [key]: toggleNode(query.filters[key] || [], id) },
                  })
                }
              />
            ))}
            {!group.options.length && <p className="muted">No matching options.</p>}
          </div>
        </details>
      ))}
    </fieldset>
  );
}

export function ActiveFilters({ query, update, reset }: Pick<Props, 'query' | 'update' | 'reset'>) {
  const chips: { label: string; remove: () => void }[] = [];
  if (query.search)
    chips.push({ label: `“${query.search}”`, remove: () => update({ search: '' }) });
  if (query.min_price || query.max_price)
    chips.push({
      label: `${query.min_price || '0'}–${query.max_price || 'any'} DT`,
      remove: () => update({ min_price: '', max_price: '' }),
    });
  for (const key of ['site', 'stock'] as const)
    for (const value of query[key])
      chips.push({
        label: key === 'stock' ? stockLabels[value] || value : value,
        remove: () => update({ [key]: query[key].filter((item) => item !== value) }),
      });
  for (const [key, values] of Object.entries(query.filters))
    for (const value of values)
      chips.push({
        label: `${value.startsWith('!') ? 'Exclude ' : ''}${fieldLabel(key)}: ${value.split('/').pop()}`,
        remove: () =>
          update({ filters: { ...query.filters, [key]: values.filter((item) => item !== value) } }),
      });
  if (!chips.length) return null;
  return (
    <div className="active-filters" aria-label="Active filters">
      {chips.map((chip, index) => (
        <button
          key={`${chip.label}-${index}`}
          onClick={chip.remove}
          aria-label={`Remove ${chip.label} filter`}
        >
          {chip.label}
          <X size={12} />
        </button>
      ))}
      <button className="clear-all" onClick={reset} disabled={!hasFilterChanges(query)}>
        Clear all
      </button>
    </div>
  );
}
