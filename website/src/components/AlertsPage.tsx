import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  Bell,
  BellOff,
  Check,
  Clock3,
  Mail,
  Plus,
  Search,
  SlidersHorizontal,
  Trash2,
  X,
} from 'lucide-react';
import { accountRequest, useAuth } from '../lib/auth';
import { fieldLabel, money, sectionLabel, stockLabels as statuses } from '../lib/catalog';
import { Dialog } from './Dialog';
import type { Product } from '../types';

type Offer = { id: string; title: string; retailer: string; price: number; status: string };
type Rule = {
  target:
    | { mode: 'product'; product_id: string }
    | { mode: 'variant'; section: string; criteria: Record<string, string[]> };
  target_price: number | null;
  restock: boolean;
  recurring_low: boolean;
  retailers: string[];
  statuses: string[];
};
type Alert = {
  id: string;
  rule: Rule;
  state: string;
  target_title: string | null;
  expires_at: string;
  last_match: Offer | null;
  last_matches: Offer[];
  lowest_seen: number | null;
  last_matched_at: string | null;
  last_delivery_at: string | null;
};
type Listing = { items: Alert[]; limit: number; used: number; expiry_months: number };
type Field = { key: string; values: string[] };
type Options = {
  sections: { id: string; fields: Field[] }[];
  retailers: string[];
  statuses: string[];
};
type Preview = {
  matching_offers: number;
  selected_status_offers: number;
  lowest_price: number | null;
  in_stock_offers: number;
  validation_message: string | null;
  variant_ids: string[];
};
const stateLabels: Record<string, string> = {
  active: 'Watching',
  paused: 'Disabled',
  pending: 'Sending email',
  delivered: 'Delivered · disabled',
  failed: 'Delivery failed',
  expired: 'Expired · disabled',
};
const date = (value: string) =>
  new Date(value).toLocaleDateString('en-GB', { day: 'numeric', month: 'short', year: 'numeric' });
const failure = (error: unknown) =>
  error instanceof Error ? error.message : 'Something went wrong. Please try again.';

function CriteriaField({
  field,
  selected,
  onChange,
}: {
  field: Field;
  selected: string[];
  onChange: (values: string[]) => void;
}) {
  const [search, setSearch] = useState('');
  const values = field.values.filter((value) => value.toLowerCase().includes(search.toLowerCase()));
  return (
    <details className="alert-field" open={selected.length ? true : undefined}>
      <summary>
        <span>{fieldLabel(field.key)}</span>
        <small>{selected.length ? `${selected.length} selected` : 'Any'}</small>
      </summary>
      <div className="alert-field-body">
        {field.values.length > 8 && (
          <input
            aria-label={`Find ${fieldLabel(field.key)}`}
            placeholder="Find a value…"
            value={search}
            onChange={(event) => setSearch(event.target.value)}
          />
        )}
        <div className="alert-value-list">
          {values.map((value) => (
            <label key={value} className="alert-check">
              <input
                type="checkbox"
                checked={selected.includes(value)}
                onChange={() =>
                  onChange(
                    selected.includes(value)
                      ? selected.filter((v) => v !== value)
                      : [...selected, value],
                  )
                }
              />
              <span>{value}</span>
            </label>
          ))}
        </div>
        {!values.length && <p className="alert-hint">No values found.</p>}
      </div>
    </details>
  );
}

export function ProductAlertButton({ product }: { product: Product }) {
  const { user, loading } = useAuth();
  const [open, setOpen] = useState(false);
  const [created, setCreated] = useState(false);
  return (
    <>
      <button
        type="button"
        className="secondary-button product-alert-button"
        disabled={loading}
        onClick={() => {
          if (!user?.email_verified) {
            window.dispatchEvent(new Event('flagship:signin'));
            return;
          }
          setCreated(false);
          setOpen(true);
        }}
      >
        <Bell size={16} /> Create alert
      </button>
      {created && (
        <p role="status" className="alert-hint">
          Your alert is watching. We’ll email you when it finds a match.
        </p>
      )}
      {open && user?.email_verified && (
        <AlertComposer
          initialOffer={{
            id: product.id,
            title: product.title,
            retailer: product.site,
            price: product.price,
            status: product.status,
          }}
          months={12}
          onClose={() => setOpen(false)}
          onCreated={() => {
            setOpen(false);
            setCreated(true);
          }}
        />
      )}
    </>
  );
}

function AlertComposer({
  initialProduct,
  initialOffer,
  months,
  onClose,
  onCreated,
}: {
  initialProduct?: string;
  initialOffer?: Offer;
  months: number;
  onClose: () => void;
  onCreated: () => void;
}) {
  const { user } = useAuth();
  const [recurringLow, setRecurringLow] = useState(false);
  const [mode, setMode] = useState<'product' | 'variant'>('product');
  const [options, setOptions] = useState<Options | null>(null);
  const [optionsRevision, setOptionsRevision] = useState(0);
  const [section, setSection] = useState('');
  const [criteria, setCriteria] = useState<Record<string, string[]>>({});
  const [query, setQuery] = useState('');
  const [offers, setOffers] = useState<Offer[]>([]);
  const [offer, setOffer] = useState<Offer | null>(initialOffer || null);
  const [searching, setSearching] = useState(false);
  const [priceOn, setPriceOn] = useState(!initialOffer || initialOffer.status === 'InStock');
  const [price, setPrice] = useState('');
  const [restock, setRestock] = useState(!!initialOffer && initialOffer.status !== 'InStock');
  const [selectedStatuses, setSelectedStatuses] = useState(['InStock', 'OnDemand', 'Arriving']);
  const [allRetailers, setAllRetailers] = useState(true);
  const [retailers, setRetailers] = useState<string[]>([]);
  const [checkedPreview, setCheckedPreview] = useState<{ rule: Rule; result: Preview } | null>(
    null,
  );
  const [previewing, setPreviewing] = useState(false);
  const [error, setError] = useState('');
  const [optionsError, setOptionsError] = useState('');
  const [previewError, setPreviewError] = useState('');
  const [saving, setSaving] = useState(false);
  useEffect(() => {
    if (mode !== 'variant') return;
    const controller = new AbortController();
    setOptionsError('');
    accountRequest<Options>('/alerts/options', { signal: controller.signal })
      .then((value) => {
        if (controller.signal.aborted) return;
        setOptions(value);
        setSection(
          (current) =>
            current || value.sections.find((s) => s.fields.some((f) => f.values.length))?.id || '',
        );
      })
      .catch((error) => {
        if (!controller.signal.aborted) setOptionsError(failure(error));
      });
    return () => controller.abort();
  }, [optionsRevision, mode]);
  useEffect(() => {
    if (!initialProduct) return;
    const controller = new AbortController();
    accountRequest<Offer[]>(`/alerts/products?id=${encodeURIComponent(initialProduct)}`, {
      signal: controller.signal,
    })
      .then((items) => {
        if (controller.signal.aborted) return;
        if (!items[0]) throw new Error('This product is no longer in the catalog.');
        setOffer(items[0]);
        setRestock(items[0].status !== 'InStock');
        setPriceOn(items[0].status === 'InStock');
      })
      .catch((error) => {
        if (!controller.signal.aborted) setError(failure(error));
      });
    return () => controller.abort();
  }, [initialProduct]);
  useEffect(() => {
    if (mode !== 'product' || query.trim().length < 2 || offer) {
      setOffers([]);
      setSearching(false);
      return;
    }
    const controller = new AbortController();
    setSearching(true);
    const timer = window.setTimeout(() => {
      accountRequest<Offer[]>(`/alerts/products?q=${encodeURIComponent(query)}`, {
        signal: controller.signal,
      })
        .then((items) => {
          if (!controller.signal.aborted) setOffers(items);
        })
        .catch((error) => {
          if (!controller.signal.aborted) setError(failure(error));
        })
        .finally(() => {
          if (!controller.signal.aborted) setSearching(false);
        });
    }, 250);
    return () => {
      clearTimeout(timer);
      controller.abort();
    };
  }, [mode, query, offer]);

  const rule = useMemo<Rule>(
    () => ({
      target:
        mode === 'product' ? { mode, product_id: offer?.id || '' } : { mode, section, criteria },
      target_price: priceOn && price ? Number(price) : null,
      restock,
      recurring_low: recurringLow,
      retailers: mode === 'product' || allRetailers ? [] : retailers,
      statuses: restock ? ['InStock'] : selectedStatuses,
    }),
    [
      mode,
      offer,
      section,
      criteria,
      priceOn,
      price,
      restock,
      recurringLow,
      allRetailers,
      retailers,
      selectedStatuses,
    ],
  );
  const ready =
    (mode === 'product'
      ? !!offer && (restock || priceOn || recurringLow)
      : !!section && Object.keys(criteria).length > 0) &&
    (!priceOn || (Number.isInteger(Number(price)) && Number(price) > 0)) &&
    rule.statuses.length > 0 &&
    (mode === 'product' || allRetailers || retailers.length > 0);
  const trigger = recurringLow ? 'low' : restock ? 'restock' : priceOn ? 'price' : 'appearance';
  const chooseTrigger = (value: string) => {
    setRecurringLow(value === 'low');
    setRestock(value === 'restock');
    setPriceOn(value === 'price');
    setError('');
  };
  const preview = checkedPreview?.rule === rule ? checkedPreview.result : null;
  useEffect(() => {
    setCheckedPreview(null);
    setPreviewError('');
    if (!ready) {
      setPreviewing(false);
      return;
    }
    const controller = new AbortController();
    setPreviewing(true);
    const timer = window.setTimeout(() => {
      accountRequest<Preview>('/alerts/preview', {
        method: 'POST',
        body: JSON.stringify(rule),
        signal: controller.signal,
      })
        .then((result) => {
          if (!controller.signal.aborted) setCheckedPreview({ rule, result });
        })
        .catch((error) => {
          if (!controller.signal.aborted) setPreviewError(failure(error));
        })
        .finally(() => {
          if (!controller.signal.aborted) setPreviewing(false);
        });
    }, 300);
    return () => {
      clearTimeout(timer);
      controller.abort();
    };
  }, [rule, ready]);
  const fields =
    options?.sections.find((s) => s.id === section)?.fields.filter((f) => f.values.length) || [];
  return (
    <Dialog
      title="Create an alert"
      className="alert-composer"
      onClose={saving ? () => {} : onClose}
    >
      <div className="alert-composer-heading">
        <span className="alert-eyebrow">
          <Bell size={15} /> YOUR NEXT FIND
        </span>
        <h2>Create an alert</h2>
        <p>Tell us what you’re waiting for. We’ll email you when it happens.</p>
      </div>
      {mode === 'variant' && optionsError ? (
        <div role="alert" className="alert-error">
          {optionsError} <button onClick={() => setOptionsRevision((v) => v + 1)}>Try again</button>
          <button
            onClick={() => {
              setMode('product');
              chooseTrigger('price');
            }}
          >
            Use an existing product instead
          </button>
        </div>
      ) : mode === 'variant' && !options ? (
        <p role="status">Loading alert options…</p>
      ) : (
        <form
          onSubmit={async (event) => {
            event.preventDefault();
            if (!ready || saving || !preview || preview.validation_message) return;
            setSaving(true);
            setError('');
            try {
              await accountRequest('/alerts', { method: 'POST', body: JSON.stringify(rule) });
              onCreated();
            } catch (error) {
              setError(failure(error));
              setSaving(false);
            }
          }}
        >
          <fieldset disabled={saving} className="alert-form-content">
            <div className="alert-mode" role="group" aria-label="Alert mode">
              <button
                type="button"
                aria-pressed={mode === 'product'}
                className={mode === 'product' ? 'selected' : ''}
                onClick={() => {
                  setMode('product');
                  setRecurringLow(false);
                  setRestock(false);
                  setPriceOn(true);
                }}
              >
                <Search size={18} />
                <span>
                  <strong>Existing product</strong>
                  <small>Follow one retailer’s offer</small>
                </span>
              </button>
              <button
                type="button"
                aria-pressed={mode === 'variant'}
                className={mode === 'variant' ? 'selected' : ''}
                onClick={() => {
                  setMode('variant');
                  setRecurringLow(false);
                  setRestock(false);
                  setPriceOn(false);
                }}
              >
                <SlidersHorizontal size={18} />
                <span>
                  <strong>Variant</strong>
                  <small>Describe your ideal configuration</small>
                </span>
              </button>
            </div>
            <section className="alert-form-section">
              <h3>
                <span>01</span> What are you looking for?
              </h3>
              {mode === 'product' ? (
                offer ? (
                  <div className="alert-selected-offer">
                    <div>
                      <strong>{offer.title}</strong>
                      <p>
                        {offer.retailer} · {money(offer.price)} TND · {statuses[offer.status]}
                      </p>
                    </div>
                    <button
                      type="button"
                      className="icon-button"
                      aria-label="Choose another product"
                      onClick={() => {
                        setOffer(null);
                        setRecurringLow(false);
                        setPriceOn(true);
                        setQuery('');
                        setRestock(false);
                        setPrice('');
                      }}
                    >
                      <X size={17} />
                    </button>
                  </div>
                ) : (
                  <div>
                    <label className="alert-label">
                      Find a product
                      <input
                        value={query}
                        onChange={(event) => setQuery(event.target.value)}
                        placeholder="Search by name or model…"
                        maxLength={120}
                      />
                    </label>
                    {searching && query.length >= 2 && (
                      <p role="status" className="alert-hint">
                        Searching offers…
                      </p>
                    )}
                    <div className="alert-search-results">
                      {offers.map((item) => (
                        <button
                          type="button"
                          key={item.id}
                          onClick={() => {
                            setOffer(item);
                            setRecurringLow(false);
                            setPrice('');
                            setRestock(item.status !== 'InStock');
                            setPriceOn(item.status === 'InStock');
                            setError('');
                          }}
                        >
                          <strong>{item.title}</strong>
                          <span>
                            {item.retailer} · {money(item.price)} TND · {statuses[item.status]}
                          </span>
                        </button>
                      ))}
                    </div>
                    {!searching && query.trim().length >= 2 && !offers.length && (
                      <p className="alert-hint">
                        No offers found. Try another name or create a variant alert.
                      </p>
                    )}
                  </div>
                )
              ) : (
                <>
                  <label className="alert-label">
                    Section
                    <select
                      value={section}
                      onChange={(event) => {
                        setSection(event.target.value);
                        setCriteria({});
                      }}
                    >
                      <option value="" disabled>
                        Choose a section
                      </option>
                      {options?.sections.map((s) => (
                        <option key={s.id} value={s.id}>
                          {sectionLabel(s.id)}
                        </option>
                      ))}
                    </select>
                  </label>
                  <p className="alert-hint">
                    Match every selected field. Within a field, any selected value can match.
                  </p>
                  <div className="alert-fields">
                    {fields.map((field) => (
                      <CriteriaField
                        key={`${section}:${field.key}`}
                        field={field}
                        selected={criteria[field.key] || []}
                        onChange={(values) =>
                          setCriteria((current) => {
                            const next = { ...current };
                            if (values.length) next[field.key] = values;
                            else delete next[field.key];
                            return next;
                          })
                        }
                      />
                    ))}
                  </div>
                  {!fields.length && (
                    <p className="alert-hint">
                      This section has no observed field values yet. Choose another section or
                      follow an existing product.
                    </p>
                  )}
                  {Object.keys(criteria).length > 0 && (
                    <div className="alert-chips">
                      {Object.entries(criteria).flatMap(([key, values]) =>
                        values.map((value) => (
                          <span key={`${key}:${value}`}>
                            {fieldLabel(key)}: {value}
                            <button
                              type="button"
                              aria-label={`Remove ${value}`}
                              onClick={() =>
                                setCriteria((current) => {
                                  const next = { ...current };
                                  next[key] = next[key].filter((v) => v !== value);
                                  if (!next[key].length) delete next[key];
                                  return next;
                                })
                              }
                            >
                              <X size={12} />
                            </button>
                          </span>
                        )),
                      )}
                    </div>
                  )}
                </>
              )}
            </section>
            <section className="alert-form-section">
              <h3>
                <span>02</span> When should we email you?
              </h3>
              <div
                className="alert-trigger-options"
                role="radiogroup"
                aria-label="Notification trigger"
              >
                {[
                  {
                    id: 'price',
                    name: 'Target price',
                    description: 'One email when an offer reaches your budget.',
                    disabled: false,
                  },
                  {
                    id: 'restock',
                    name: 'Back in stock',
                    description:
                      mode === 'product' && offer?.status === 'InStock'
                        ? 'This listing is already in stock.'
                        : 'One email when an unavailable offer returns.',
                    disabled: mode === 'product' && offer?.status === 'InStock',
                  },
                  ...(mode === 'variant'
                    ? [
                        {
                          id: 'appearance',
                          name: 'New matching offers',
                          description: 'One email when new or updated offers first qualify.',
                          disabled: false,
                        },
                      ]
                    : []),
                  {
                    id: 'low',
                    name: 'Recurring new-low prices',
                    description: 'Keep watching and email each new low until expiry.',
                    disabled: !user,
                  },
                ].map((option) => (
                  <label
                    className={'alert-trigger-option' + (option.disabled ? ' is-disabled' : '')}
                    key={option.id}
                  >
                    <input
                      type="radio"
                      name="alert-trigger"
                      value={option.id}
                      checked={trigger === option.id}
                      disabled={option.disabled}
                      onChange={() => chooseTrigger(option.id)}
                    />
                    <span>
                      <strong>{option.name}</strong>
                      <small>{option.description}</small>
                    </span>
                  </label>
                ))}
              </div>
              {(restock || recurringLow) && (
                <label className="alert-check alert-budget-toggle">
                  <input
                    type="checkbox"
                    checked={priceOn}
                    onChange={(event) => setPriceOn(event.target.checked)}
                  />
                  <span>Only notify at or below my budget</span>
                </label>
              )}
              {priceOn && (
                <label className="alert-price">
                  Target price{' '}
                  <div>
                    <input
                      type="number"
                      min="1"
                      step="1"
                      value={price}
                      onChange={(event) => setPrice(event.target.value)}
                      placeholder={offer ? `Below ${offer.price}` : 'Your budget'}
                      required
                      aria-label="Target price in TND"
                    />
                    <span>TND or less</span>
                  </div>
                </label>
              )}
              <p className="alert-hint">
                {recurringLow
                  ? 'A new low must beat the cheapest qualifying price seen since this watch started. Equal prices do not notify.'
                  : restock
                    ? 'The offer must change to in stock. If you set a budget, both conditions must be met together.'
                    : trigger === 'appearance'
                      ? 'Existing matches will not trigger immediately. We watch for new qualifying offers.'
                      : 'Your target must be below the current matching price.'}{' '}
                Each notification includes all qualifying offers from that scan.
              </p>
            </section>
            <section className="alert-form-section alert-scope">
              <h3>
                <span>03</span> Which offers should count?
              </h3>
              <h4>Availability</h4>
              <div className="alert-statuses" role="group" aria-label="Allowed stock statuses">
                {['InStock', 'OnDemand', 'Arriving'].map((status) => (
                  <label className="alert-check" key={status}>
                    <input
                      type="checkbox"
                      checked={rule.statuses.includes(status)}
                      disabled={
                        restock ||
                        (selectedStatuses.length === 1 && selectedStatuses.includes(status))
                      }
                      onChange={() =>
                        setSelectedStatuses((current) =>
                          current.includes(status)
                            ? current.filter((s) => s !== status)
                            : [...current, status],
                        )
                      }
                    />
                    <span>{statuses[status]}</span>
                  </label>
                ))}
              </div>
              <p className="alert-hint">
                {restock
                  ? 'Restock watches only count offers that become in stock.'
                  : 'Keep at least one status selected. Out-of-stock offers never trigger price alerts.'}
              </p>
              <h4>Retailers</h4>
              {mode === 'product' ? (
                <p className="alert-hint">
                  {offer
                    ? 'Following this listing at ' + offer.retailer + '.'
                    : 'Choose a product above to select its retailer.'}{' '}
                  To watch across stores, choose a variant.
                </p>
              ) : (
                <>
                  <div
                    className="alert-retailer-mode"
                    role="radiogroup"
                    aria-label="Retailer scope"
                  >
                    <label className="alert-check">
                      <input
                        type="radio"
                        name="retailer-scope"
                        checked={allRetailers}
                        onChange={() => setAllRetailers(true)}
                      />
                      <span>All retailers, including future additions</span>
                    </label>
                    <label className="alert-check">
                      <input
                        type="radio"
                        name="retailer-scope"
                        checked={!allRetailers}
                        onChange={() => setAllRetailers(false)}
                      />
                      <span>Choose retailers</span>
                    </label>
                  </div>
                  <div className="alert-retailers" role="group" aria-label="Retailers to monitor">
                    {options?.retailers.map((retailer) => (
                      <label className="alert-check" key={retailer}>
                        <input
                          type="checkbox"
                          checked={allRetailers || retailers.includes(retailer)}
                          disabled={allRetailers}
                          onChange={() =>
                            setRetailers((current) =>
                              current.includes(retailer)
                                ? current.filter((r) => r !== retailer)
                                : [...current, retailer],
                            )
                          }
                        />
                        <span>{retailer}</span>
                      </label>
                    ))}
                  </div>
                  {!allRetailers && (
                    <p className="alert-hint">
                      {retailers.length
                        ? retailers.length + ' retailers selected.'
                        : 'Select at least one retailer to continue.'}
                    </p>
                  )}
                </>
              )}
            </section>
            <div className="alert-preview" role="status">
              {previewing ? (
                'Checking current offers…'
              ) : preview ? (
                <>
                  <strong>
                    {preview.matching_offers
                      ? `${preview.matching_offers} current matching ${preview.matching_offers === 1 ? 'offer' : 'offers'}`
                      : 'No matching offers yet'}
                  </strong>
                  <p>
                    {preview.lowest_price !== null
                      ? `From ${money(preview.lowest_price)} TND before your target-price condition. `
                      : ''}
                    {preview.matching_offers > 0 && preview.selected_status_offers === 0
                      ? 'None are in your selected stock statuses yet. '
                      : ''}
                    {preview.matching_offers
                      ? 'We’ll watch for the next qualifying change.'
                      : 'Your alert will keep watching as the catalog changes.'}
                  </p>
                </>
              ) : (
                <p>Choose your criteria and conditions to preview current matches.</p>
              )}
            </div>
            {(previewError || preview?.validation_message || error) && (
              <p role="alert" className="alert-error">
                {error || previewError || preview?.validation_message}
              </p>
            )}
            <div className="alert-composer-footer">
              <p>
                <Mail size={15} />{' '}
                {recurringLow
                  ? 'Keep watching for new lows until expiry.'
                  : 'One email with all scan matches, then disabled.'}
                <br />
                <Clock3 size={15} /> Watches for {months} {months === 1 ? 'month' : 'months'}.
              </p>
              <button
                className="primary-button"
                disabled={
                  !ready ||
                  previewing ||
                  !preview ||
                  !!preview.validation_message ||
                  !!previewError ||
                  saving
                }
              >
                {saving ? 'Creating…' : 'Create alert'} <Bell size={16} />
              </button>
            </div>
          </fieldset>
        </form>
      )}
    </Dialog>
  );
}

export function AlertsPage() {
  const { user } = useAuth();
  return <AccountAlertsPage key={user?.id ?? 'guest'} />;
}

function AccountAlertsPage() {
  const { user, loading } = useAuth();
  const [data, setData] = useState<Listing | null>(null);
  const [error, setError] = useState('');
  const [notice, setNotice] = useState('');
  const [busy, setBusy] = useState('');
  const [composer, setComposer] = useState(false);
  const [initialProduct, setInitialProduct] = useState<string | undefined>();
  const [deleteId, setDeleteId] = useState<string | null>(null);
  const refreshVersion = useRef(0);
  const refresh = useCallback(async () => {
    const version = ++refreshVersion.current;
    setError('');
    try {
      const next = await accountRequest<Listing>('/alerts');
      if (version === refreshVersion.current) setData(next);
    } catch (error) {
      if (version === refreshVersion.current) setError(failure(error));
    }
  }, []);
  useEffect(() => {
    document.title = 'Your alerts — Flagship';
    if (user) void refresh();
    else if (!loading) window.dispatchEvent(new Event('flagship:signin'));
  }, [user, loading, refresh]);
  useEffect(() => {
    const product = new URLSearchParams(window.location.search).get('product');
    if (product && user?.email_verified) {
      setInitialProduct(product);
      setComposer(true);
      window.history.replaceState(null, '', '/alerts');
    }
  }, [user]);
  useEffect(() => {
    if (!user) return;
    const timer = window.setInterval(() => {
      if (!document.hidden) void refresh();
    }, 30000);
    return () => clearInterval(timer);
  }, [user, refresh]);
  const mutate = async (id: string, enabled?: boolean) => {
    setBusy(id);
    setError('');
    try {
      await accountRequest(`/alerts/${encodeURIComponent(id)}`, {
        method: enabled === undefined ? 'DELETE' : 'PATCH',
        ...(enabled === undefined ? {} : { body: JSON.stringify({ enabled }) }),
      });
      setDeleteId(null);
      await refresh();
    } catch (error) {
      setError(failure(error));
    } finally {
      setBusy('');
    }
  };
  return (
    <main className="alerts-page" id="alerts-content" tabIndex={-1}>
      <header className="alerts-heading">
        <div>
          <span className="alert-eyebrow">
            <Bell size={15} /> GOOD THINGS ARE WORTH WATCHING
          </span>
          <h1>
            Your alerts<span>.</span>
          </h1>
          <p>The right product. The right price. An email when it’s time.</p>
        </div>
        {user?.email_verified && (
          <button
            className="primary-button"
            disabled={!data || data.used >= data.limit}
            onClick={() => {
              setInitialProduct(undefined);
              setComposer(true);
            }}
          >
            <Plus size={17} /> Create alert
          </button>
        )}
      </header>
      {loading ? (
        <p role="status">Loading your account…</p>
      ) : !user ? (
        <div className="alerts-empty">
          <Bell size={34} />
          <h2>Let your next upgrade come to you.</h2>
          <p>Sign in to create product and variant alerts.</p>
          <button
            className="primary-button"
            onClick={() => window.dispatchEvent(new Event('flagship:signin'))}
          >
            Sign in
          </button>
        </div>
      ) : (
        <>
          {!user.email_verified && (
            <div className="alert-preview">
              Verify your email in <a href="/settings">account settings</a> before creating alerts.
            </div>
          )}
          {data && (
            <div className="alerts-usage">
              <div>
                <span>
                  {data.used} / {data.limit} active alerts
                </span>
                <div className="alerts-meter">
                  <i style={{ width: `${Math.min(100, (data.used / data.limit) * 100)}%` }} />
                </div>
              </div>
              <p>
                Only active alerts count toward your limit. Re-enable an alert to restart its{' '}
                {data.expiry_months}-month watch period.
              </p>
            </div>
          )}
          {error && (
            <div role="alert" className="alert-error">
              {error} <button onClick={() => void refresh()}>Refresh</button>
            </div>
          )}
          {notice && (
            <div role="status" className="alert-notice">
              <Check size={17} />
              {notice}
              <button
                className="icon-button"
                aria-label="Dismiss notification"
                onClick={() => setNotice('')}
              >
                <X size={15} />
              </button>
            </div>
          )}
          {!data && !error && <p role="status">Loading your alerts…</p>}
          <div className="alerts-list">
            {data?.items.map((alert) => {
              const target = alert.rule.target;
              const active = alert.state === 'active' || alert.state === 'pending';
              return (
                <article className={`alert-card alert-${alert.state}`} key={alert.id}>
                  <div className="alert-card-top">
                    <span className="alert-kind">
                      {target.mode === 'product' ? (
                        <Search size={15} />
                      ) : (
                        <SlidersHorizontal size={15} />
                      )}{' '}
                      {target.mode === 'product'
                        ? 'Existing product'
                        : sectionLabel(target.section)}
                    </span>
                    <span className="alert-state">
                      {active ? <span className="alert-dot" /> : <BellOff size={13} />}{' '}
                      {stateLabels[alert.state]}
                    </span>
                  </div>
                  <h2>
                    {target.mode === 'product'
                      ? alert.target_title || alert.last_match?.title || 'Tracked retailer offer'
                      : sectionLabel(target.section) + ' · custom configuration'}
                  </h2>
                  {target.mode === 'variant' && (
                    <div className="alert-chips">
                      {Object.entries(target.criteria).map(([key, values]) => (
                        <span key={key}>
                          {fieldLabel(key)}: {values.join(' or ')}
                        </span>
                      ))}
                    </div>
                  )}
                  <div className="alert-conditions">
                    <strong>
                      {alert.rule.target_price !== null
                        ? `${money(alert.rule.target_price)} TND or less`
                        : alert.rule.restock
                          ? 'Back in stock'
                          : 'A new matching offer'}
                    </strong>
                    {alert.rule.restock && alert.rule.target_price !== null && (
                      <span>and back in stock</span>
                    )}
                  </div>
                  <p className="alert-hint">
                    {alert.rule.retailers.length
                      ? alert.rule.retailers.join(', ')
                      : 'All retailers'}{' '}
                    · {alert.rule.statuses.map((s) => statuses[s]).join(', ')}
                  </p>
                  {alert.rule.recurring_low && (
                    <p className="alert-hint">
                      Recurring new lows
                      {alert.lowest_seen !== null
                        ? ' · Lowest watched: ' + money(alert.lowest_seen) + ' TND'
                        : ''}
                    </p>
                  )}
                  {(alert.last_matches || []).length > 1 && (
                    <details className="alert-hint">
                      <summary>
                        {alert.last_matches.length} matching offers in the last notification
                      </summary>
                      <ul>
                        {alert.last_matches.map((offer) => (
                          <li key={offer.id}>
                            <a href={'/product/' + encodeURIComponent(offer.id)}>{offer.title}</a> ·{' '}
                            {offer.retailer} · {money(offer.price)} TND · {statuses[offer.status]}
                          </li>
                        ))}
                      </ul>
                    </details>
                  )}
                  {alert.last_match && (
                    <p className="alert-last-match">
                      Last match:{' '}
                      <a href={`/product/${encodeURIComponent(alert.last_match.id)}`}>
                        {alert.last_match.title}
                      </a>{' '}
                      · {money(alert.last_match.price)} TND
                      {alert.last_delivery_at
                        ? ` · Email sent ${date(alert.last_delivery_at)}`
                        : alert.last_matched_at
                          ? ` · Observed ${date(alert.last_matched_at)}`
                          : ''}
                    </p>
                  )}
                  {alert.state === 'failed' && (
                    <p className="alert-error">
                      We couldn’t deliver the email after several attempts. Re-enable to watch for a
                      new match.
                    </p>
                  )}
                  <footer>
                    <span>
                      <Clock3 size={14} />
                      {alert.state === 'expired' ? 'Expired' : 'Watch expires'}{' '}
                      {date(alert.expires_at)}
                    </span>
                    <div>
                      {target.mode === 'product' && (
                        <a href={`/product/${encodeURIComponent(target.product_id)}`}>
                          View product
                        </a>
                      )}
                      <button
                        type="button"
                        className="alert-toggle"
                        role="switch"
                        aria-checked={active}
                        aria-label={`${active ? 'Disable' : 'Enable'} alert: ${alert.target_title || (target.mode === 'variant' ? sectionLabel(target.section) : 'product')}`}
                        disabled={busy === alert.id || !user.email_verified}
                        onClick={() => void mutate(alert.id, !active)}
                      >
                        <i />
                      </button>
                      <button
                        className="icon-button"
                        aria-label="Delete alert"
                        disabled={busy === alert.id}
                        onClick={() => setDeleteId(alert.id)}
                      >
                        <Trash2 size={16} />
                      </button>
                    </div>
                  </footer>
                </article>
              );
            })}
          </div>
        </>
      )}
      {composer && user?.email_verified && (
        <AlertComposer
          initialProduct={initialProduct}
          months={data?.expiry_months || 1}
          onClose={() => setComposer(false)}
          onCreated={() => {
            setComposer(false);
            setNotice('Your alert is watching. We’ll email you when it finds a match.');
            void refresh();
          }}
        />
      )}
      {deleteId && (
        <Dialog title="Delete alert" className="alert-delete" onClose={() => setDeleteId(null)}>
          <h2>Delete this alert?</h2>
          <p>This removes the saved rule and frees an alert slot.</p>
          {error && (
            <p className="alert-error" role="alert">
              {error}
            </p>
          )}
          <div>
            <button className="secondary-button" onClick={() => setDeleteId(null)}>
              Keep alert
            </button>
            <button
              className="primary-button"
              disabled={!!busy}
              onClick={() => void mutate(deleteId)}
            >
              Delete alert
            </button>
          </div>
        </Dialog>
      )}
    </main>
  );
}
