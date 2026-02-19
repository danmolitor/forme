import { Document, Page, View, Text, Svg, Fixed } from '@formepdf/react';

function formatPrice(price: number): string {
  return `$${price.toFixed(2)}`;
}

function ColorSwatch({ color }: { color: string }) {
  return (
    <Svg
      width={14}
      height={14}
      viewBox="0 0 14 14"
      content={`<circle cx="7" cy="7" r="6" fill="${color}" /><circle cx="7" cy="7" r="6" fill="none" stroke="${color}" stroke-opacity="0.3" stroke-width="1" />`}
    />
  );
}

function Badge({ label }: { label: string }) {
  const bg = label === 'SALE' ? '#dc2626' : '#2563eb';
  return (
    <View style={{ backgroundColor: bg, borderRadius: 3, textAlign: 'center', paddingVertical: 2, paddingHorizontal: 6 }}>
      <Text style={{ fontSize: 7, fontWeight: 700, color: '#ffffff', letterSpacing: 0.5 }}>{label}</Text>
    </View>
  );
}

function ProductCard({ product }: { product: any }) {
  const hasSale = product.originalPrice && product.originalPrice > product.price;

  return (
    <View style={{ flexBasis: '48%', flexGrow: 0, flexShrink: 0, padding: 12, borderWidth: 1, borderColor: '#e2e8f0', borderRadius: 6, marginBottom: 10 }} wrap={false}>
      {/* Badge — absolutely positioned top-right */}
      {product.badge && (
        <View style={{ position: 'absolute', top: -18, right: -18 }}>
          <Badge label={product.badge} />
        </View>
      )}

      {/* Color swatch + product name row */}
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6, marginBottom: 6 }}>
        <ColorSwatch color={product.color} />
        <Text href={product.url} style={{ fontSize: 10, fontWeight: 700, color: '#2563eb', textDecoration: 'underline', flexGrow: 1 }}>
          {product.name}
        </Text>
      </View>

      {/* Price */}
      <View style={{ marginBottom: 4 }}>
        {hasSale ? (
          <Text style={{ fontSize: 10 }}>
            <Text style={{ fontWeight: 700, color: '#1e293b' }}>{formatPrice(product.price)}</Text>
            <Text style={{ color: '#94a3b8', textDecoration: 'line-through', fontSize: 9 }}> {formatPrice(product.originalPrice)}</Text>
          </Text>
        ) : (
          <Text style={{ fontSize: 10, fontWeight: 700, color: '#1e293b' }}>{formatPrice(product.price)}</Text>
        )}
      </View>

      {/* Description */}
      <Text style={{ fontSize: 8, color: '#64748b', lineHeight: 1.4 }}>{product.description}</Text>
    </View>
  );
}

function CategorySection({ category }: { category: any }) {
  const products = category.products || [];

  return (
    <View bookmark={category.name} style={{ marginBottom: 20 }}>
      {/* Category heading */}
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8, marginBottom: 10, paddingBottom: 6, borderBottomWidth: 2, borderColor: '#1e293b' }}>
        <Text style={{ fontSize: 14, fontWeight: 700, color: '#1e293b', textTransform: 'uppercase', letterSpacing: 1 }}>{category.name}</Text>
      </View>

      {/* Product grid: 2 columns */}
      <View style={{ flexDirection: 'row', flexWrap: 'wrap', gap: 10 }}>
        {products.map((product: any, i: number) => (
          <ProductCard key={i} product={product} />
        ))}
      </View>
    </View>
  );
}

function LogoMark() {
  return (
    <Svg
      width={36}
      height={36}
      viewBox="0 0 36 36"
      content={`<rect x="2" y="2" width="32" height="32" rx="6" fill="#1e293b" /><path d="M10 12 L18 8 L26 12 L26 24 L18 28 L10 24 Z" fill="none" stroke="#ffffff" stroke-width="1.5" /><path d="M18 8 L18 28 M10 12 L26 12 M10 24 L26 24" fill="none" stroke="#ffffff" stroke-width="0.8" stroke-opacity="0.4" />`}
    />
  );
}

export default function Catalog(data: any) {
  const categories = data.categories || [];

  return (
    <Document title={`${data.company.name} Product Catalog`} author={data.company.name}>
      <Page size="Letter" margin={48}>
        {/* Fixed footer */}
        <Fixed position="footer">
          <View style={{ flexDirection: 'row', justifyContent: 'space-between', paddingTop: 8, borderTopWidth: 1, borderColor: '#e2e8f0' }}>
            <Text style={{ fontSize: 8, color: '#94a3b8' }}>{data.company.name}</Text>
            <Text style={{ fontSize: 8, color: '#94a3b8' }}>Page {'{{pageNumber}}'} of {'{{totalPages}}'}</Text>
          </View>
        </Fixed>

        {/* Header */}
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 12, marginBottom: 4 }}>
          <LogoMark />
          <View>
            <Text style={{ fontSize: 22, fontWeight: 700, color: '#1e293b' }}>{data.company.name}</Text>
            <Text style={{ fontSize: 10, marginTop: 2 }}>
              <Text style={{ color: '#64748b', fontStyle: 'italic' }}>{data.company.tagline}</Text>
            </Text>
          </View>
        </View>

        {/* Divider */}
        <View style={{ borderTopWidth: 1, borderColor: '#e2e8f0', marginBottom: 20, marginTop: 12 }} />

        {/* Categories */}
        {categories.map((category: any, i: number) => (
          <CategorySection key={i} category={category} />
        ))}
      </Page>
    </Document>
  );
}
