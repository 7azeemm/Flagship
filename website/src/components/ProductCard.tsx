import { useEffect, useState, type ReactNode } from 'react';
import {
  ArrowUpRight,
  ChevronLeft,
  ChevronRight,
  Cpu,
  HardDrive,
  ImageOff,
  Layers3,
  Monitor,
  Tag,
  Zap,
} from 'lucide-react';
import type { Product, VariantSummary } from '../types';
import { money, productSpecs, safeURL, stockLabels } from '../lib/catalog';

export function ProductImage({
  product,
  src = product.image,
  eager = false,
  alt = product.title,
}: {
  product: Product;
  src?: string;
  eager?: boolean;
  alt?: string;
}) {
  const [failed, setFailed] = useState(false);
  const url = safeURL(src);
  return !url || failed ? (
    <span className="image-fallback">
      <ImageOff size={28} />
      <span>Image unavailable</span>
    </span>
  ) : (
    <img
      src={url}
      alt={alt}
      loading={eager ? 'eager' : 'lazy'}
      referrerPolicy="no-referrer"
      onError={() => setFailed(true)}
    />
  );
}
export function ProductGallery({
  product,
  className,
  onOpen,
  label,
  eager = false,
  pressed,
  children,
}: {
  product: Product;
  className: string;
  onOpen: () => void;
  label: string;
  eager?: boolean;
  pressed?: boolean;
  children?: ReactNode;
}) {
  const images = [
    ...new Set(
      [product.image, ...product.images].map(safeURL).filter((url): url is string => !!url),
    ),
  ];
  const [index, setIndex] = useState(0);
  useEffect(() => setIndex(0), [product.id]);
  const current = images[Math.min(index, images.length - 1)];
  const move = (step: number) =>
    setIndex((value) => (value + step + images.length) % images.length);
  return (
    <div
      className={`product-gallery ${className === 'product-image' ? 'card-gallery' : 'detail-gallery'}`}
    >
      <button className={className} onClick={onOpen} aria-label={label} aria-pressed={pressed}>
        <ProductImage
          key={current || 'missing'}
          product={product}
          src={current}
          eager={eager}
          alt={
            images.length > 1
              ? `${product.title}, image ${Math.min(index, images.length - 1) + 1} of ${images.length}`
              : product.title
          }
        />
        {children}
      </button>
      {images.length > 1 && (
        <>
          <button
            type="button"
            className="gallery-arrow gallery-prev"
            onClick={() => move(-1)}
            aria-label={`Previous image of ${product.title}`}
          >
            <ChevronLeft size={20} />
          </button>
          <button
            type="button"
            className="gallery-arrow gallery-next"
            onClick={() => move(1)}
            aria-label={`Next image of ${product.title}`}
          >
            <ChevronRight size={20} />
          </button>
        </>
      )}
    </div>
  );
}
const icons = {
  cpu: Cpu,
  gpu: Zap,
  memory: Layers3,
  ram: Layers3,
  storage: HardDrive,
  display: Monitor,
  model: Tag,
};
export function Specifications({ product, fields }: { product: Product; fields: string[] }) {
  return (
    <dl className={`specification-grid ${fields.length === 1 ? 'single-spec' : ''}`}>
      {productSpecs(product, fields).map(({ key, label, value, unknown }) => {
        const Icon = icons[key as keyof typeof icons] || Tag;
        return (
          <div
            key={key}
            data-component={key}
            className={`specification ${unknown ? 'specification-unknown' : ''}`}
            title={`${label}: ${value}`}
          >
            <Icon size={15} />
            <div>
              <dt>{label}</dt>
              <dd>{value}</dd>
            </div>
          </div>
        );
      })}
    </dl>
  );
}
export function Price({ product, from = false }: { product: Product; from?: boolean }) {
  return (
    <div className="product-price">
      {from && <span className="price-prefix">From</span>}
      <strong>
        {money(product.price)} <small>DT</small>
      </strong>
      {!from && product.original_price != null && product.original_price > product.price && (
        <>
          <del>{money(product.original_price)} DT</del>
          <span className="price-saving">
            Save {money(product.original_price - product.price)} DT
          </span>
        </>
      )}
    </div>
  );
}
export function ProductCard({
  product,
  variant,
  fields,
  onOpen,
}: {
  product: Product;
  variant?: VariantSummary;
  fields: string[];
  onOpen: () => void;
}) {
  const grouped = !!variant;
  const name = variant?.title ?? product.title;
  const discount =
    !grouped &&
    product.original_price &&
    product.price > 0 &&
    product.original_price > product.price
      ? Math.round((1 - product.price / product.original_price) * 100)
      : 0;
  const url = safeURL(product.url);
  return (
    <article className="product-card">
      <ProductGallery
        product={product}
        className="product-image"
        onOpen={onOpen}
        label={`View ${name}`}
      >
        {discount > 0 && <span className="discount">−{discount}%</span>}
        {grouped && (
          <span className="offer-badge">
            <Layers3 size={13} />
            {variant.offer_count} offers
          </span>
        )}
      </ProductGallery>
      <div className="product-meta">
        <span className="retailer-badge">
          {grouped ? `${variant.retailer_count} retailers` : product.site}
        </span>
        <span className={`stock stock-${grouped ? 'grouped' : product.status}`}>
          {grouped ? 'Compare offers' : stockLabels[product.status] || 'Availability unknown'}
        </span>
      </div>
      <h2>
        <button onClick={onOpen}>{name}</button>
      </h2>
      <Price product={product} from={grouped} />
      <Specifications product={product} fields={fields} />
      <div className="card-actions">
        <button className="secondary-button" onClick={onOpen}>
          {grouped ? 'Compare offers' : 'View details'} <ArrowUpRight size={16} />
        </button>
        {!grouped && url && (
          <a
            className="retailer-button"
            href={url}
            target="_blank"
            rel="noopener noreferrer"
            aria-label={`Visit ${product.site} for ${product.title}`}
          >
            <ArrowUpRight size={18} />
          </a>
        )}
      </div>
    </article>
  );
}
