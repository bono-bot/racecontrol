"use client";

import { useEffect, useState } from "react";
import { publicApi, api, getImageBaseUrl } from "@/lib/api";
import type { CafeMenuItem, CafeOrderResponse, CafeOrderItem, ActivePromo } from "@/lib/api";

function formatPrice(paise: number): string {
  if (paise % 100 === 0) {
    return `Rs. ${paise / 100}`;
  }
  return `Rs. ${(paise / 100).toFixed(2)}`;
}

// ─── Cart types ─────────────────────────────────────────────────────────────

interface CartItem {
  item: CafeMenuItem;
  quantity: number;
}

// ─── Sub-components ──────────────────────────────────────────────────────────

function CoffeePlaceholder() {
  return (
    <div className="w-full aspect-[4/3] bg-rp-dark flex items-center justify-center">
      <svg
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth={1.5}
        className="w-10 h-10 text-rp-grey"
      >
        <path d="M17 8h1a4 4 0 010 8h-1" strokeLinecap="round" strokeLinejoin="round" />
        <path
          d="M3 8h14v9a4 4 0 01-4 4H7a4 4 0 01-4-4V8z"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
        <path d="M6 2v3M10 2v3M14 2v3" strokeLinecap="round" strokeLinejoin="round" />
      </svg>
    </div>
  );
}

function ItemImage({ item }: { item: CafeMenuItem }) {
  const [imgError, setImgError] = useState(false);
  const imageBaseUrl = getImageBaseUrl();

  if (!item.image_path || imgError) {
    return <CoffeePlaceholder />;
  }

  return (
    <div className="w-full aspect-[4/3] overflow-hidden">
      <img
        src={`${imageBaseUrl}${item.image_path}`}
        alt={item.name}
        className="w-full h-full object-cover"
        onError={() => setImgError(true)}
      />
    </div>
  );
}

interface ItemCardProps {
  item: CafeMenuItem;
  cartQuantity: number;
  onAdd: (item: CafeMenuItem) => void;
  onRemove: (itemId: string) => void;
  onUpdateQty: (itemId: string, qty: number) => void;
}

function ItemCard({ item, cartQuantity, onAdd, onRemove, onUpdateQty }: ItemCardProps) {
  return (
    <div
      className={`bg-rp-card rounded-xl overflow-hidden border flex flex-col ${
        item.out_of_stock ? "border-rp-border opacity-75" : "border-rp-border"
      }`}
    >
      <div className="relative">
        <ItemImage item={item} />
        {item.out_of_stock && (
          <div className="absolute inset-0 flex items-center justify-center bg-black/40">
            <span className="bg-gray-700 text-gray-200 text-xs font-semibold px-2 py-1 rounded">
              Out of Stock
            </span>
          </div>
        )}
      </div>
      <div className="p-3 flex flex-col flex-1">
        <p className="text-sm font-medium text-white line-clamp-2">{item.name}</p>
        {item.description && (
          <p className="text-xs text-rp-grey line-clamp-2 mt-1">{item.description}</p>
        )}
        <p className="text-sm font-bold text-rp-red mt-2">
          {formatPrice(item.selling_price_paise)}
        </p>
        <div className="mt-2">
          {cartQuantity === 0 ? (
            <button
              disabled={item.out_of_stock}
              onClick={() => onAdd(item)}
              className={`w-full py-1.5 rounded-lg text-sm font-semibold transition-colors ${
                item.out_of_stock
                  ? "bg-gray-700 text-gray-500 cursor-not-allowed"
                  : "bg-rp-red text-white active:opacity-80"
              }`}
            >
              {item.out_of_stock ? "Unavailable" : "Add to Cart"}
            </button>
          ) : (
            <div className="flex items-center justify-between gap-2">
              <button
                onClick={() =>
                  cartQuantity === 1
                    ? onRemove(item.id)
                    : onUpdateQty(item.id, cartQuantity - 1)
                }
                className="w-8 h-8 rounded-lg bg-rp-dark border border-rp-border text-white font-bold text-lg flex items-center justify-center"
              >
                −
              </button>
              <span className="text-white font-semibold text-sm">{cartQuantity}</span>
              <button
                onClick={() => onAdd(item)}
                disabled={
                  item.is_countable && cartQuantity >= item.stock_quantity
                }
                className={`w-8 h-8 rounded-lg text-white font-bold text-lg flex items-center justify-center ${
                  item.is_countable && cartQuantity >= item.stock_quantity
                    ? "bg-gray-700 text-gray-500 cursor-not-allowed"
                    : "bg-rp-red"
                }`}
              >
                +
              </button>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

function SkeletonCard() {
  return (
    <div className="animate-pulse bg-rp-card rounded-xl h-48 border border-rp-border" />
  );
}

// ─── Promo Banner ────────────────────────────────────────────────────────────

function PromoBanner({ promos }: { promos: ActivePromo[] }) {
  if (promos.length === 0) return null;
  return (
    <div className="bg-rp-red/10 border border-rp-red/30 rounded-xl px-4 py-3 mb-4 space-y-1">
      {promos.map((promo) => (
        <div key={promo.id} className="flex items-center gap-2">
          <span className="text-rp-red font-bold text-xs uppercase tracking-wide">PROMO</span>
          <span className="text-white text-sm font-medium">{promo.name}</span>
          {promo.time_label && (
            <span className="ml-auto text-rp-grey text-xs">{promo.time_label}</span>
          )}
        </div>
      ))}
    </div>
  );
}

// ─── Cart Panel ──────────────────────────────────────────────────────────────

interface CartPanelProps {
  cart: CartItem[];
  onClose: () => void;
  onRemove: (itemId: string) => void;
  onUpdateQty: (itemId: string, qty: number) => void;
  onCheckout: () => void;
}

function CartPanel({ cart, onClose, onRemove, onUpdateQty, onCheckout }: CartPanelProps) {
  const cartTotal = cart.reduce(
    (sum, c) => sum + c.item.selling_price_paise * c.quantity,
    0
  );

  return (
    <div className="fixed inset-0 z-40 flex flex-col justify-end bg-black/60">
      <div className="bg-[#1A1A1A] rounded-t-2xl max-h-[80vh] flex flex-col">
        <div className="flex items-center justify-between px-4 py-3 border-b border-rp-border">
          <h2 className="text-white font-semibold text-lg">Your Cart</h2>
          <button onClick={onClose} className="text-rp-grey text-2xl leading-none">
            &times;
          </button>
        </div>
        <div className="overflow-y-auto flex-1 px-4 py-3 space-y-3">
          {cart.map((c) => (
            <div key={c.item.id} className="flex items-center gap-3">
              <div className="flex-1">
                <p className="text-white text-sm font-medium">{c.item.name}</p>
                <p className="text-rp-grey text-xs">
                  {formatPrice(c.item.selling_price_paise)} each
                </p>
              </div>
              <div className="flex items-center gap-2">
                <button
                  onClick={() =>
                    c.quantity === 1
                      ? onRemove(c.item.id)
                      : onUpdateQty(c.item.id, c.quantity - 1)
                  }
                  className="w-7 h-7 rounded-lg bg-rp-dark border border-rp-border text-white font-bold flex items-center justify-center text-sm"
                >
                  −
                </button>
                <span className="text-white font-semibold w-5 text-center text-sm">
                  {c.quantity}
                </span>
                <button
                  onClick={() => {
                    if (c.item.is_countable && c.quantity >= c.item.stock_quantity) return;
                    onUpdateQty(c.item.id, c.quantity + 1);
                  }}
                  disabled={c.item.is_countable && c.quantity >= c.item.stock_quantity}
                  className={`w-7 h-7 rounded-lg text-white font-bold flex items-center justify-center text-sm ${
                    c.item.is_countable && c.quantity >= c.item.stock_quantity
                      ? "bg-gray-700 text-gray-500 cursor-not-allowed"
                      : "bg-rp-red"
                  }`}
                >
                  +
                </button>
              </div>
              <p className="text-white font-semibold text-sm w-16 text-right">
                {formatPrice(c.item.selling_price_paise * c.quantity)}
              </p>
              <button
                onClick={() => onRemove(c.item.id)}
                className="text-rp-grey text-lg leading-none"
              >
                &times;
              </button>
            </div>
          ))}
        </div>
        <div className="px-4 py-4 border-t border-rp-border">
          <div className="flex items-center justify-between mb-3">
            <span className="text-rp-grey text-sm">Total</span>
            <span className="text-white font-bold text-lg">{formatPrice(cartTotal)}</span>
          </div>
          <button
            onClick={onCheckout}
            className="w-full py-3 bg-rp-red text-white font-semibold rounded-xl"
          >
            Checkout
          </button>
        </div>
      </div>
    </div>
  );
}

// ─── Checkout Panel ──────────────────────────────────────────────────────────

interface CheckoutPanelProps {
  cart: CartItem[];
  onClose: () => void;
  onSuccess: (result: CafeOrderResponse) => void;
}

function CheckoutPanel({ cart, onClose, onSuccess }: CheckoutPanelProps) {
  const [walletBalance, setWalletBalance] = useState<number | null>(null);
  const [walletLoading, setWalletLoading] = useState(true);
  const [ordering, setOrdering] = useState(false);
  const [orderError, setOrderError] = useState<string | null>(null);

  const cartTotal = cart.reduce(
    (sum, c) => sum + c.item.selling_price_paise * c.quantity,
    0
  );
  const hasSufficientBalance =
    walletBalance !== null && walletBalance >= cartTotal;

  useEffect(() => {
    api
      .wallet()
      .then((res) => {
        setWalletBalance(res.wallet?.balance_paise ?? null);
      })
      .catch(() => {
        setWalletBalance(null);
      })
      .finally(() => {
        setWalletLoading(false);
      });
  }, []);

  async function handlePlaceOrder() {
    if (ordering) return;
    setOrdering(true);
    setOrderError(null);

    const items: CafeOrderItem[] = cart.map((c) => ({
      item_id: c.item.id,
      quantity: c.quantity,
    }));

    try {
      const result = await api.placeCafeOrder(items);
      if ("error" in result && result.error) {
        setOrderError(result.error);
        setOrdering(false);
      } else {
        onSuccess(result as CafeOrderResponse);
      }
    } catch {
      setOrderError("Network error. Please try again.");
      setOrdering(false);
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex flex-col justify-end bg-black/60">
      <div className="bg-[#1A1A1A] rounded-t-2xl max-h-[85vh] flex flex-col">
        <div className="flex items-center justify-between px-4 py-3 border-b border-rp-border">
          <h2 className="text-white font-semibold text-lg">Order Summary</h2>
          <button onClick={onClose} className="text-rp-grey text-2xl leading-none">
            &times;
          </button>
        </div>
        <div className="overflow-y-auto flex-1 px-4 py-3 space-y-2">
          {cart.map((c) => (
            <div key={c.item.id} className="flex items-center justify-between">
              <div className="flex-1">
                <span className="text-white text-sm">{c.item.name}</span>
                <span className="text-rp-grey text-xs ml-2">x{c.quantity}</span>
              </div>
              <span className="text-white text-sm font-semibold">
                {formatPrice(c.item.selling_price_paise * c.quantity)}
              </span>
            </div>
          ))}
          <div className="border-t border-rp-border pt-2 flex items-center justify-between">
            <span className="text-white font-semibold">Total</span>
            <span className="text-white font-bold text-lg">{formatPrice(cartTotal)}</span>
          </div>
        </div>

        <div className="px-4 py-3 border-t border-rp-border">
          {walletLoading ? (
            <div className="text-rp-grey text-sm text-center py-2">
              Loading wallet balance...
            </div>
          ) : walletBalance === null ? (
            <div className="text-yellow-400 text-sm text-center py-2">
              Could not load wallet balance. You can still try to place the order.
            </div>
          ) : (
            <div className="flex items-center justify-between mb-2">
              <span className="text-rp-grey text-sm">Wallet Balance</span>
              <span
                className={`font-semibold text-sm ${
                  hasSufficientBalance ? "text-green-400" : "text-red-400"
                }`}
              >
                {formatPrice(walletBalance)}
              </span>
            </div>
          )}

          {!walletLoading && walletBalance !== null && !hasSufficientBalance && (
            <div className="bg-red-900/40 border border-red-700 rounded-lg px-3 py-2 mb-3 text-red-300 text-sm">
              Insufficient balance ({formatPrice(walletBalance)}). You need{" "}
              {formatPrice(cartTotal - walletBalance)} more. Please top up your wallet.
            </div>
          )}

          {orderError && (
            <div className="bg-red-900/40 border border-red-700 rounded-lg px-3 py-2 mb-3 text-red-300 text-sm">
              {orderError}
            </div>
          )}

          <button
            onClick={handlePlaceOrder}
            disabled={
              ordering ||
              (!walletLoading && walletBalance !== null && !hasSufficientBalance)
            }
            className={`w-full py-3 font-semibold rounded-xl flex items-center justify-center gap-2 ${
              ordering ||
              (!walletLoading && walletBalance !== null && !hasSufficientBalance)
                ? "bg-gray-700 text-gray-500 cursor-not-allowed"
                : "bg-rp-red text-white"
            }`}
          >
            {ordering ? (
              <>
                <svg className="animate-spin w-4 h-4" viewBox="0 0 24 24" fill="none">
                  <circle
                    className="opacity-25"
                    cx="12"
                    cy="12"
                    r="10"
                    stroke="currentColor"
                    strokeWidth="4"
                  />
                  <path
                    className="opacity-75"
                    fill="currentColor"
                    d="M4 12a8 8 0 018-8v4a4 4 0 00-4 4H4z"
                  />
                </svg>
                Placing Order...
              </>
            ) : (
              "Place Order"
            )}
          </button>
        </div>
      </div>
    </div>
  );
}

// ─── Order Confirmation ───────────────────────────────────────────────────────

interface OrderConfirmationProps {
  result: CafeOrderResponse;
  onOrderAnother: () => void;
}

function OrderConfirmation({ result, onOrderAnother }: OrderConfirmationProps) {
  return (
    <div className="fixed inset-0 z-50 flex flex-col justify-end bg-black/60">
      <div className="bg-[#1A1A1A] rounded-t-2xl px-4 py-6">
        <div className="flex flex-col items-center text-center mb-6">
          <div className="w-14 h-14 bg-green-500/20 rounded-full flex items-center justify-center mb-3">
            <svg
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth={2}
              className="w-7 h-7 text-green-400"
            >
              <path
                d="M5 13l4 4L19 7"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
            </svg>
          </div>
          <h2 className="text-white font-bold text-xl">Order Placed!</h2>
          <p className="text-rp-grey text-sm mt-1">
            Receipt: <span className="text-white font-medium">{result.receipt_number}</span>
          </p>
        </div>

        <div className="bg-rp-card border border-rp-border rounded-xl p-3 mb-4 space-y-1">
          {result.items.map((it) => (
            <div key={it.item_id} className="flex items-center justify-between">
              <span className="text-rp-grey text-sm">
                {it.name} x{it.quantity}
              </span>
              <span className="text-white text-sm font-semibold">
                {formatPrice(it.line_total_paise)}
              </span>
            </div>
          ))}
          {result.discount_paise > 0 && (
            <div className="border-t border-rp-border pt-1 flex items-center justify-between">
              <span className="text-green-400 text-sm">
                Promo discount ({result.applied_promo_name})
              </span>
              <span className="text-green-400 font-semibold text-sm">
                -{formatPrice(result.discount_paise)}
              </span>
            </div>
          )}
          <div className={`${result.discount_paise > 0 ? "" : "border-t border-rp-border pt-1"} flex items-center justify-between`}>
            <span className="text-white font-semibold text-sm">Total Paid</span>
            <span className="text-white font-bold">{formatPrice(result.total_paise)}</span>
          </div>
        </div>

        <div className="flex items-center justify-between mb-5">
          <span className="text-rp-grey text-sm">New Wallet Balance</span>
          <span className="text-green-400 font-semibold">
            {formatPrice(result.new_balance_paise)}
          </span>
        </div>

        <button
          onClick={onOrderAnother}
          className="w-full py-3 bg-rp-red text-white font-semibold rounded-xl"
        >
          Order Another
        </button>
      </div>
    </div>
  );
}

// ─── Main Page ────────────────────────────────────────────────────────────────

export default function CafePage() {
  const [items, setItems] = useState<CafeMenuItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [activeCategory, setActiveCategory] = useState<string | null>(null);

  // Cart state
  const [cart, setCart] = useState<CartItem[]>([]);
  const [cartOpen, setCartOpen] = useState(false);
  const [checkoutOpen, setCheckoutOpen] = useState(false);
  const [orderResult, setOrderResult] = useState<CafeOrderResponse | null>(null);
  const [activePromos, setActivePromos] = useState<ActivePromo[]>([]);

  useEffect(() => {
    publicApi
      .cafeMenu()
      .then((res) => {
        setItems(res.items ?? []);
      })
      .catch(() => {
        setItems([]);
      })
      .finally(() => {
        setLoading(false);
      });
  }, []);

  useEffect(() => {
    publicApi.activePromos()
      .then((promos) => setActivePromos(Array.isArray(promos) ? promos : []))
      .catch(() => setActivePromos([]));
  }, []);

  // ─── Cart helpers ──────────────────────────────────────────────────────────

  function addToCart(item: CafeMenuItem) {
    if (item.out_of_stock) return;
    setCart((prev) => {
      const existing = prev.find((c) => c.item.id === item.id);
      if (existing) {
        const newQty = existing.quantity + 1;
        if (item.is_countable && newQty > item.stock_quantity) return prev;
        return prev.map((c) =>
          c.item.id === item.id ? { ...c, quantity: newQty } : c
        );
      }
      return [...prev, { item, quantity: 1 }];
    });
  }

  function removeFromCart(itemId: string) {
    setCart((prev) => prev.filter((c) => c.item.id !== itemId));
  }

  function updateQuantity(itemId: string, qty: number) {
    if (qty <= 0) {
      removeFromCart(itemId);
      return;
    }
    setCart((prev) =>
      prev.map((c) => {
        if (c.item.id !== itemId) return c;
        const cappedQty =
          c.item.is_countable ? Math.min(qty, c.item.stock_quantity) : qty;
        return { ...c, quantity: cappedQty };
      })
    );
  }

  const cartTotal = cart.reduce(
    (sum, c) => sum + c.item.selling_price_paise * c.quantity,
    0
  );
  const cartItemCount = cart.reduce((sum, c) => sum + c.quantity, 0);

  // ─── Category grouping ─────────────────────────────────────────────────────

  const categories: string[] = [];
  const seen = new Set<string>();
  for (const item of items) {
    if (!seen.has(item.category_name)) {
      seen.add(item.category_name);
      categories.push(item.category_name);
    }
  }

  const grouped = new Map<string, CafeMenuItem[]>();
  for (const item of items) {
    const existing = grouped.get(item.category_name);
    if (existing) {
      existing.push(item);
    } else {
      grouped.set(item.category_name, [item]);
    }
  }

  const displayCategories =
    activeCategory !== null ? [activeCategory] : categories;

  function getCartQuantity(itemId: string): number {
    return cart.find((c) => c.item.id === itemId)?.quantity ?? 0;
  }

  function handleOrderSuccess(result: CafeOrderResponse) {
    setCart([]);
    setCheckoutOpen(false);
    setCartOpen(false);
    setOrderResult(result);
  }

  return (
    <div className="min-h-screen bg-rp-dark px-4 pt-6">
      <h1 className="text-2xl font-bold text-white mb-4">Cafe Menu</h1>

      <PromoBanner promos={activePromos} />

      {/* Category filter pills */}
      {!loading && categories.length > 0 && (
        <div
          className="flex gap-2 pb-2 overflow-x-auto"
          style={{ msOverflowStyle: "none", scrollbarWidth: "none" } as React.CSSProperties}
        >
          <button
            onClick={() => setActiveCategory(null)}
            className={`flex-shrink-0 px-4 py-1.5 rounded-full text-sm font-medium transition-colors ${
              activeCategory === null
                ? "bg-rp-red text-white"
                : "bg-rp-card text-rp-grey border border-rp-border"
            }`}
          >
            All
          </button>
          {categories.map((cat) => (
            <button
              key={cat}
              onClick={() => setActiveCategory(cat)}
              className={`flex-shrink-0 px-4 py-1.5 rounded-full text-sm font-medium transition-colors ${
                activeCategory === cat
                  ? "bg-rp-red text-white"
                  : "bg-rp-card text-rp-grey border border-rp-border"
              }`}
            >
              {cat}
            </button>
          ))}
        </div>
      )}

      {/* Loading skeletons */}
      {loading && (
        <div className="grid grid-cols-2 gap-3 mt-4">
          {[0, 1, 2, 3, 4, 5].map((i) => (
            <SkeletonCard key={i} />
          ))}
        </div>
      )}

      {/* Empty state */}
      {!loading && items.length === 0 && (
        <div className="flex flex-col items-center justify-center mt-20 gap-4">
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth={1.5}
            className="w-16 h-16 text-rp-grey"
          >
            <path d="M17 8h1a4 4 0 010 8h-1" strokeLinecap="round" strokeLinejoin="round" />
            <path
              d="M3 8h14v9a4 4 0 01-4 4H7a4 4 0 01-4-4V8z"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
            <path d="M6 2v3M10 2v3M14 2v3" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
          <p className="text-rp-grey text-center">No items available right now</p>
        </div>
      )}

      {/* Category sections */}
      {!loading &&
        items.length > 0 &&
        displayCategories.map((cat) => {
          const catItems = grouped.get(cat) ?? [];
          if (catItems.length === 0) return null;
          return (
            <div key={cat}>
              {activeCategory === null && (
                <h2 className="text-lg font-semibold text-white mb-3 mt-6">{cat}</h2>
              )}
              <div
                className={`grid grid-cols-2 gap-3 ${activeCategory !== null ? "mt-4" : ""}`}
              >
                {catItems.map((item) => (
                  <ItemCard
                    key={item.id}
                    item={item}
                    cartQuantity={getCartQuantity(item.id)}
                    onAdd={addToCart}
                    onRemove={removeFromCart}
                    onUpdateQty={updateQuantity}
                  />
                ))}
              </div>
            </div>
          );
        })}

      {/* Bottom spacer for floating bar */}
      <div className="h-24" />

      {/* Floating cart bar */}
      {cartItemCount > 0 && !cartOpen && !checkoutOpen && !orderResult && (
        <div className="fixed bottom-0 left-0 right-0 z-30 px-4 pb-4">
          <button
            onClick={() => setCartOpen(true)}
            className="w-full bg-rp-red text-white py-3 rounded-xl font-semibold flex items-center justify-between px-4"
          >
            <span className="bg-white/20 rounded-lg px-2 py-0.5 text-sm font-bold">
              {cartItemCount} item{cartItemCount !== 1 ? "s" : ""}
            </span>
            <span>View Cart</span>
            <span className="font-bold">{formatPrice(cartTotal)}</span>
          </button>
        </div>
      )}

      {/* Cart panel */}
      {cartOpen && !checkoutOpen && (
        <CartPanel
          cart={cart}
          onClose={() => setCartOpen(false)}
          onRemove={removeFromCart}
          onUpdateQty={updateQuantity}
          onCheckout={() => {
            setCartOpen(false);
            setCheckoutOpen(true);
          }}
        />
      )}

      {/* Checkout panel */}
      {checkoutOpen && (
        <CheckoutPanel
          cart={cart}
          onClose={() => setCheckoutOpen(false)}
          onSuccess={handleOrderSuccess}
        />
      )}

      {/* Order confirmation */}
      {orderResult && (
        <OrderConfirmation
          result={orderResult}
          onOrderAnother={() => setOrderResult(null)}
        />
      )}
    </div>
  );
}
