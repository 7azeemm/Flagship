import { useEffect, useMemo, useState } from 'react';
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
import { fieldLabel, groupingEnabled, hasFilterChanges, stockLabels, toggle, toggleNode } from '../lib/catalog';
import { RangeSlider } from './RangeSlider';
import { StructuredFilterNode } from './StructuredFilterNode';

interface Props {
  query: CatalogQuery;
  info: SectionInfo | null;
  baselineSectionInfo: SectionInfo | null;
  update: (changes: Partial<CatalogQuery>) => void;
  reset: () => void;
  disabled: boolean;
}
export function Filters({ query, info, baselineSectionInfo, update, reset, disabled }: Props) {
  const [search, setSearch] = useState(query.search);
  useEffect(() => setSearch(query.search), [query.search]);
  const [expanded, setExpanded] = useState<Record<string, boolean>>({});
  const [allRetailers, setAllRetailers] = useState(false);
  const groupFields = baselineSectionInfo?.group_by || [];
  const grouped = groupingEnabled(query, baselineSectionInfo);
  const selectedGroupFields = groupFields
    .filter((field) => field.required || (query.groupBy === null ? field.default : query.groupBy.includes(field.key)))
    .map((field) => field.key);
  const currentInfo = info || baselineSectionInfo;
  const sites = useMemo(() => {
    const counts = new Map((currentInfo?.sites || []).map((site) => [site.value, site.count]));
    for (const site of query.site) if (!counts.has(site)) counts.set(site, 0);
    return [...counts]
      .map(([value, count]) => ({ value, count }))
      .sort((left, right) => right.count - left.count || left.value.localeCompare(right.value));
  }, [currentInfo?.sites, query.site]);
  const visibleSites = allRetailers
    ? sites
    : sites.filter((site, index) => index < 5 || query.site.includes(site.value));
  const statuses =
    currentInfo?.statuses ||
    ['InStock', 'OutOfStock', 'Arriving', 'OnDemand'].map((value) => ({ value, count: 0 }));
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
            aria-checked={grouped}
            onClick={() => update({ grouped: !grouped })}
          >
            <span className="toggle-track">
              <Check size={10} />
            </span>
            <span>Group offers</span>
            <strong>{grouped ? 'On' : 'Off'}</strong>
          </button>
          <div
            className="group-fields"
            aria-label="Group products by"
            aria-disabled={!grouped}
          >
            {groupFields.map((field) => (
              <button
                key={field.key}
                aria-pressed={selectedGroupFields.includes(field.key)}
                disabled={!grouped || field.required}
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
      <div className="filter-search-section">
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
      </div>
      {baselineSectionInfo && (
        <section className="filter-section">
          <h3>
            Price range <span>DT</span>
          </h3>
          <RangeSlider
            min={baselineSectionInfo.min_price}
            max={baselineSectionInfo.max_price}
            valueMin={
              query.min_price ? Number(query.min_price) : baselineSectionInfo.min_price
            }
            valueMax={
              query.max_price ? Number(query.max_price) : baselineSectionInfo.max_price
            }
            onChange={(min, max) =>
              update({
                min_price: min === baselineSectionInfo.min_price ? '' : String(min),
                max_price: max === baselineSectionInfo.max_price ? '' : String(max),
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
          {statuses.map((status) => (
            <label className="checkbox-row" key={status.value}>
              <input
                type="checkbox"
                checked={query.stock.includes(status.value)}
                onChange={() => update({ stock: toggle(query.stock, status.value) })}
              />
              <span>{stockLabels[status.value] || status.value}</span>
              <small>{status.count}</small>
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
            <label className="checkbox-row" key={site.value}>
              <input
                type="checkbox"
                checked={query.site.includes(site.value)}
                onChange={() => update({ site: toggle(query.site, site.value) })}
              />
              <span>{site.value}</span>
              <small>{site.count}</small>
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
      {(info?.filters || baselineSectionInfo?.filters || []).map((group, index) => (
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
                key={node.id || node.label}
                node={node}
                filterKey={group.key}
                selectedValues={query.filters[group.key] || []}
                onToggle={(key, node) =>
                  update({
                    filters: { ...query.filters, [key]: toggleNode(query.filters[key] || [], node) },
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
    for (const value of query[key]) {
      if (key === 'stock' && value === 'InStock' && query.stock.length === 1) continue;
      chips.push({
        label: key === 'stock' ? stockLabels[value] || value : value,
        remove: () => update({ [key]: query[key].filter((item) => item !== value) }),
      });
    }
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
