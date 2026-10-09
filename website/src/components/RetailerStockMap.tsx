import { useState, type CSSProperties } from 'react';
import type { StatisticsData } from '../types';
import { money, sectionLabel } from '../lib/catalog';

export function RetailerStockMap({
  overviews,
  sections,
  onSelect,
}: {
  overviews: StatisticsData['retailer_overviews'];
  sections: string[];
  onSelect: (section: string, retailer: string) => void;
}) {
  const [search, setSearch] = useState('');
  const [expanded, setExpanded] = useState(false);
  const names = [
    ...new Set(
      sections.flatMap((section) => overviews[section]?.retailers.map((item) => item.name) || []),
    ),
  ]
    .sort((a, b) => {
      const stock = (name: string) =>
        sections.reduce(
          (sum, section) =>
            sum +
            (overviews[section]?.retailers.find((item) => item.name === name)?.in_stock_offers ||
              0),
          0,
        );
      return stock(b) - stock(a) || a.localeCompare(b);
    })
    .filter((name) => name.toLowerCase().includes(search.toLowerCase()));
  const ready = Object.values(overviews).some((overview) =>
    overview.retailers.some((item) => item.in_stock_offers !== undefined),
  );
  return (
    <section className="retailer-strengths">
      <div className="insight-section-heading">
        <div>
          <h2>Stock by retailer & category</h2>
          <p>
            Where is there stock to choose from? Each cell shows available offers and their share of
            that store’s listings. Select a cell to explore its price ranges.
          </p>
        </div>
        <label className="retailer-strengths-search">
          <span>Find a retailer</span>
          <input
            type="search"
            value={search}
            placeholder="Retailer name…"
            onChange={(event) => {
              setSearch(event.target.value);
              setExpanded(false);
            }}
          />
        </label>
      </div>
      <div className="strengths-key">
        <span>Share in stock</span>
        <span className="strengths-key-scale" /> <span>Higher share</span>
        <small>Stores with the most stock first</small>
      </div>
      {!ready ? (
        <div className="statistics-empty">
          <strong>Retailer stock data is waiting for the updated backend.</strong>
        </div>
      ) : !names.length ? (
        <div className="statistics-empty">
          <strong>No matching retailers</strong>
        </div>
      ) : (
        <>
          <div
            className="retailer-strengths-scroll"
            tabIndex={0}
            role="region"
            aria-label="Retailer stock grid; scroll horizontally for more categories"
          >
            <table aria-label="Stock by retailer and category">
              <thead>
                <tr>
                  <th scope="col">Retailer</th>
                  {sections.map((section) => (
                    <th key={section} scope="col">
                      {sectionLabel(section)}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {(expanded ? names : names.slice(0, 10)).map((name) => (
                  <tr key={name}>
                    <th scope="row">{name}</th>
                    {sections.map((section) => {
                      const item = overviews[section]?.retailers.find((item) => item.name === name);
                      const eligible =
                        !!item && item.offers > 0 && item.in_stock_offers !== undefined;
                      const count = item?.in_stock_offers || 0;
                      const value = eligible ? (100 * count) / item.offers : 0;
                      return (
                        <td key={section}>
                          {eligible ? (
                            <button
                              style={{ '--strength': `${8 + value * 0.3}%` } as CSSProperties}
                              onClick={() => onSelect(section, name)}
                              aria-label={`${name}, ${sectionLabel(section)}: ${count} in-stock offers, ${value.toFixed(0)}% of ${item.offers} listings. Explore configuration statistics.`}
                            >
                              <strong>{money(count)}</strong>
                              <small>{value.toFixed(0)}% in stock</small>
                            </button>
                          ) : (
                            <span
                              className="strength-insufficient"
                              title="No tracked offers in this category."
                            >
                              <strong>—</strong>
                              <small>No listings</small>
                            </span>
                          )}
                        </td>
                      );
                    })}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <div className="retailer-comparison-foot">
            <span>
              {names.length} retailers{!expanded && names.length > 10 ? ' · first ten shown' : ''}
            </span>
            {names.length > 10 && (
              <button onClick={() => setExpanded(!expanded)}>
                {expanded ? 'Show first ten' : `Show all ${names.length} retailers`}
              </button>
            )}
          </div>
        </>
      )}
      <p className="strengths-note">
        Counts are in-stock listings, not distinct configurations. Stock is the latest recorded
        status and may change between scans.
      </p>
    </section>
  );
}
