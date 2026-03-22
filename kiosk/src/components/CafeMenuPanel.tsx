"use client";

import { useState, useEffect, useRef } from "react";
import { api } from "@/lib/api";
import type { CafeMenuItem, Driver, CafeOrderResponse, CafeOrderItem } from "@/lib/types";

function formatPrice(paise: number): string {
  const rupees = paise / 100;
  return rupees % 1 === 0 ? `Rs. ${rupees}` : `Rs. ${rupees.toFixed(2)}`;
}

function groupByCategory(items: CafeMenuItem[]): Map<string, CafeMenuItem[]> {
  const map = new Map<string, CafeMenuItem[]>();
  for (const item of items) {
    const key = item.category_name;
    if (!map.has(key)) map.set(key, []);
    map.get(key)!.push(item);
  }
  return map;
}

interface OrderItem {
  item: CafeMenuItem;
  quantity: number;
}

export function CafeMenuPanel() {
  const [items, setItems] = useState<CafeMenuItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [activeCategory, setActiveCategory] = useState<string>("All");

  // Order state
  const [order, setOrder] = useState<OrderItem[]>([]);
  const [selectedDriverId, setSelectedDriverId] = useState<string>("");
  const [selectedDriverName, setSelectedDriverName] = useState<string>("");
  const [drivers, setDrivers] = useState<Driver[]>([]);
  const [driverSearch, setDriverSearch] = useState("");
  const [showDriverDropdown, setShowDriverDropdown] = useState(false);
  const [ordering, setOrdering] = useState(false);
  const [orderResult, setOrderResult] = useState<CafeOrderResponse | null>(null);
  const [orderError, setOrderError] = useState<string | null>(null);

  const searchRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    api.publicCafeMenu()
      .then((res) => {
        setItems(res.items ?? []);
      })
      .catch((err: unknown) => {
        setError(err instanceof Error ? err.message : "Failed to load menu");
      })
      .finally(() => setLoading(false));

    api.listDrivers().then((res) => {
      if (res.drivers) setDrivers(res.drivers);
    }).catch(() => {
      // drivers list failure is non-fatal
    });
  }, []);

  // Close driver dropdown when clicking outside
  useEffect(() => {
    function handleClick(e: MouseEvent) {
      if (searchRef.current && !searchRef.current.contains(e.target as Node)) {
        setShowDriverDropdown(false);
      }
    }
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, []);

  const grouped = groupByCategory(items);
  const categories = ["All", ...Array.from(grouped.keys())];

  const displayItems: CafeMenuItem[] =
    activeCategory === "All"
      ? items
      : (grouped.get(activeCategory) ?? []);

  const filteredDrivers = driverSearch.trim()
    ? drivers.filter((d) =>
        d.name.toLowerCase().includes(driverSearch.toLowerCase())
      ).slice(0, 8)
    : [];

  const orderTotal = order.reduce(
    (sum, o) => sum + o.item.selling_price_paise * o.quantity,
    0
  );

  function addItem(item: CafeMenuItem) {
    if (item.out_of_stock) return;
    setOrder((prev) => {
      const existing = prev.find((o) => o.item.id === item.id);
      if (existing) {
        return prev.map((o) =>
          o.item.id === item.id ? { ...o, quantity: o.quantity + 1 } : o
        );
      }
      return [...prev, { item, quantity: 1 }];
    });
  }

  function removeItem(itemId: string) {
    setOrder((prev) => prev.filter((o) => o.item.id !== itemId));
  }

  function changeQty(itemId: string, delta: number) {
    setOrder((prev) =>
      prev
        .map((o) =>
          o.item.id === itemId ? { ...o, quantity: o.quantity + delta } : o
        )
        .filter((o) => o.quantity > 0)
    );
  }

  function getOrderQty(itemId: string): number {
    return order.find((o) => o.item.id === itemId)?.quantity ?? 0;
  }

  function selectDriver(driver: Driver) {
    setSelectedDriverId(driver.id);
    setSelectedDriverName(driver.name);
    setDriverSearch(driver.name);
    setShowDriverDropdown(false);
  }

  function clearDriver() {
    setSelectedDriverId("");
    setSelectedDriverName("");
    setDriverSearch("");
  }

  async function placeOrder() {
    if (!selectedDriverId || order.length === 0) return;
    setOrdering(true);
    setOrderError(null);

    const orderItems: CafeOrderItem[] = order.map((o) => ({
      item_id: o.item.id,
      quantity: o.quantity,
    }));

    try {
      const result = await api.placeCafeOrder(selectedDriverId, orderItems);
      if ("error" in result) {
        setOrderError(result.error);
      } else {
        setOrderResult(result);
      }
    } catch (err: unknown) {
      setOrderError(err instanceof Error ? err.message : "Order failed");
    } finally {
      setOrdering(false);
    }
  }

  function resetOrder() {
    setOrder([]);
    setOrderResult(null);
    setOrderError(null);
    setSelectedDriverId("");
    setSelectedDriverName("");
    setDriverSearch("");
  }

  if (loading) {
    return (
      <div className="p-4">
        <div className="flex gap-2 mb-4 overflow-x-auto pb-1">
          {[1, 2, 3].map((i) => (
            <div
              key={i}
              className="h-7 w-20 rounded-full bg-rp-border animate-pulse shrink-0"
            />
          ))}
        </div>
        <div className="grid grid-cols-2 gap-3">
          {[1, 2, 3, 4].map((i) => (
            <div
              key={i}
              className="bg-rp-border rounded-lg p-3 h-16 animate-pulse"
            />
          ))}
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="p-4 text-center text-rp-grey text-sm">{error}</div>
    );
  }

  if (items.length === 0) {
    return (
      <div className="p-4 text-center text-rp-grey text-sm">
        No cafe items available
      </div>
    );
  }

  return (
    <div className="flex h-full">
      {/* LEFT — Menu (60%) */}
      <div className="flex flex-col" style={{ width: "60%" }}>
        {/* Category tabs */}
        <div className="flex gap-2 px-4 py-3 overflow-x-auto border-b border-rp-border shrink-0">
          {categories.map((cat) => (
            <button
              key={cat}
              onClick={() => setActiveCategory(cat)}
              className={`px-3 py-1 rounded-full text-xs font-semibold whitespace-nowrap shrink-0 transition-colors ${
                activeCategory === cat
                  ? "bg-rp-red text-white"
                  : "bg-rp-card border border-rp-border text-rp-grey hover:text-white hover:border-rp-red"
              }`}
            >
              {cat}
            </button>
          ))}
        </div>

        {/* Item grid */}
        <div className="flex-1 overflow-y-auto p-4">
          {activeCategory === "All" ? (
            Array.from(grouped.entries()).map(([catName, catItems]) => (
              <div key={catName} className="mb-5">
                <h3 className="text-xs font-bold text-rp-grey uppercase tracking-wider mb-2">
                  {catName}
                </h3>
                <div className="grid grid-cols-2 gap-3">
                  {catItems.map((item) => (
                    <ItemCard
                      key={item.id}
                      item={item}
                      qty={getOrderQty(item.id)}
                      onAdd={() => addItem(item)}
                      onChangeQty={(delta) => changeQty(item.id, delta)}
                    />
                  ))}
                </div>
              </div>
            ))
          ) : (
            <div className="grid grid-cols-2 gap-3">
              {displayItems.map((item) => (
                <ItemCard
                  key={item.id}
                  item={item}
                  qty={getOrderQty(item.id)}
                  onAdd={() => addItem(item)}
                  onChangeQty={(delta) => changeQty(item.id, delta)}
                />
              ))}
            </div>
          )}
        </div>
      </div>

      {/* RIGHT — Order sidebar (40%) */}
      <div
        className="flex flex-col border-l border-rp-border bg-rp-card"
        style={{ width: "40%" }}
      >
        {orderResult ? (
          /* Success state */
          <div className="flex flex-col items-center justify-center flex-1 p-6 text-center gap-4">
            <div className="text-green-400 text-4xl">✓</div>
            <div>
              <p className="text-white font-bold text-lg">Order Placed!</p>
              <p className="text-rp-grey text-xs mt-1">Receipt #{orderResult.receipt_number}</p>
            </div>
            <div className="bg-rp-dark rounded-lg p-4 w-full text-sm">
              <div className="flex justify-between text-rp-grey mb-1">
                <span>Total charged</span>
                <span className="text-white font-semibold">{formatPrice(orderResult.total_paise)}</span>
              </div>
              <div className="flex justify-between text-rp-grey">
                <span>New balance</span>
                <span className="text-green-400 font-semibold">{formatPrice(orderResult.new_balance_paise)}</span>
              </div>
            </div>
            <button
              onClick={resetOrder}
              className="w-full py-2 rounded-lg text-sm font-semibold bg-rp-red text-white hover:bg-red-700 transition-colors"
            >
              New Order
            </button>
          </div>
        ) : (
          <>
            {/* Customer selector */}
            <div className="px-4 py-3 border-b border-rp-border shrink-0" ref={searchRef}>
              <p className="text-rp-grey text-[10px] uppercase tracking-wider mb-1 font-semibold">
                Customer
              </p>
              <div className="relative">
                <input
                  type="text"
                  value={driverSearch}
                  onChange={(e) => {
                    setDriverSearch(e.target.value);
                    setShowDriverDropdown(true);
                    if (e.target.value !== selectedDriverName) {
                      setSelectedDriverId("");
                      setSelectedDriverName("");
                    }
                  }}
                  onFocus={() => {
                    if (driverSearch.trim()) setShowDriverDropdown(true);
                  }}
                  placeholder="Search customer by name…"
                  className="w-full bg-rp-dark border border-rp-border rounded-lg px-3 py-2 text-sm text-white placeholder-rp-grey focus:outline-none focus:border-rp-red"
                />
                {selectedDriverId && (
                  <button
                    onClick={clearDriver}
                    className="absolute right-2 top-1/2 -translate-y-1/2 text-rp-grey hover:text-white text-xs"
                  >
                    ✕
                  </button>
                )}
                {showDriverDropdown && filteredDrivers.length > 0 && (
                  <div className="absolute z-10 top-full left-0 right-0 mt-1 bg-rp-dark border border-rp-border rounded-lg shadow-lg overflow-hidden">
                    {filteredDrivers.map((d) => (
                      <button
                        key={d.id}
                        onClick={() => selectDriver(d)}
                        className="w-full text-left px-3 py-2 text-sm text-white hover:bg-rp-border transition-colors"
                      >
                        {d.name}
                        {d.phone && (
                          <span className="text-rp-grey text-xs ml-2">{d.phone}</span>
                        )}
                      </button>
                    ))}
                  </div>
                )}
              </div>
              {selectedDriverId && (
                <p className="text-green-400 text-[10px] mt-1">
                  Selected: {selectedDriverName}
                </p>
              )}
            </div>

            {/* Order items list */}
            <div className="flex-1 overflow-y-auto px-4 py-3">
              {order.length === 0 ? (
                <p className="text-rp-grey text-xs text-center mt-8">
                  Add items from the menu
                </p>
              ) : (
                <div className="flex flex-col gap-2">
                  {order.map((o) => (
                    <div
                      key={o.item.id}
                      className="bg-rp-dark border border-rp-border rounded-lg p-2"
                    >
                      <div className="flex items-start justify-between gap-2">
                        <p className="text-white text-xs font-medium leading-snug flex-1">
                          {o.item.name}
                        </p>
                        <button
                          onClick={() => removeItem(o.item.id)}
                          className="text-rp-grey hover:text-red-400 text-xs shrink-0"
                        >
                          ✕
                        </button>
                      </div>
                      <div className="flex items-center justify-between mt-2">
                        {/* Qty controls */}
                        <div className="flex items-center gap-2">
                          <button
                            onClick={() => changeQty(o.item.id, -1)}
                            className="w-6 h-6 rounded bg-rp-border text-white text-xs flex items-center justify-center hover:bg-rp-grey transition-colors"
                          >
                            −
                          </button>
                          <span className="text-white text-xs w-4 text-center">{o.quantity}</span>
                          <button
                            onClick={() => changeQty(o.item.id, +1)}
                            className="w-6 h-6 rounded bg-rp-border text-white text-xs flex items-center justify-center hover:bg-rp-grey transition-colors"
                          >
                            +
                          </button>
                        </div>
                        <div className="text-right">
                          <p className="text-rp-grey text-[10px]">
                            {formatPrice(o.item.selling_price_paise)} × {o.quantity}
                          </p>
                          <p className="text-white text-xs font-semibold">
                            {formatPrice(o.item.selling_price_paise * o.quantity)}
                          </p>
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>

            {/* Order total + action */}
            <div className="px-4 py-3 border-t border-rp-border shrink-0">
              {orderError && (
                <div className="mb-3 px-3 py-2 bg-red-900/40 border border-red-700 rounded-lg text-red-400 text-xs">
                  {orderError}
                </div>
              )}
              <div className="flex items-center justify-between mb-3">
                <span className="text-rp-grey text-sm">Total</span>
                <span className="text-white font-bold text-base">
                  {formatPrice(orderTotal)}
                </span>
              </div>
              <button
                onClick={placeOrder}
                disabled={!selectedDriverId || order.length === 0 || ordering}
                className={`w-full py-2.5 rounded-lg text-sm font-bold transition-colors ${
                  !selectedDriverId || order.length === 0 || ordering
                    ? "bg-rp-border text-rp-grey cursor-not-allowed"
                    : "bg-rp-red text-white hover:bg-red-700"
                }`}
              >
                {ordering ? "Placing Order…" : "Place Order"}
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}

interface ItemCardProps {
  item: CafeMenuItem;
  qty: number;
  onAdd: () => void;
  onChangeQty: (delta: number) => void;
}

function ItemCard({ item, qty, onAdd, onChangeQty }: ItemCardProps) {
  const outOfStock = item.out_of_stock;

  return (
    <div
      className={`relative bg-rp-dark border rounded-lg p-3 transition-colors ${
        outOfStock
          ? "border-rp-border opacity-60 cursor-not-allowed"
          : "border-rp-border hover:border-rp-red cursor-default"
      }`}
    >
      {outOfStock && (
        <div className="absolute inset-0 flex items-center justify-center rounded-lg bg-black/50">
          <span className="text-rp-grey text-[10px] font-bold uppercase tracking-wider">
            Out of Stock
          </span>
        </div>
      )}
      <p className="font-medium text-white text-sm leading-snug">{item.name}</p>
      <p className="text-rp-red text-xs font-semibold mt-1">
        {formatPrice(item.selling_price_paise)}
      </p>
      {item.description && (
        <p className="text-rp-grey text-[10px] mt-1 line-clamp-2">{item.description}</p>
      )}

      {!outOfStock && (
        <div className="mt-2 flex items-center justify-end">
          {qty === 0 ? (
            <button
              onClick={onAdd}
              className="px-2 py-0.5 rounded bg-rp-red text-white text-xs font-semibold hover:bg-red-700 transition-colors"
            >
              + Add
            </button>
          ) : (
            <div className="flex items-center gap-1">
              <button
                onClick={() => onChangeQty(-1)}
                className="w-5 h-5 rounded bg-rp-border text-white text-xs flex items-center justify-center hover:bg-rp-grey transition-colors"
              >
                −
              </button>
              <span className="text-white text-xs w-4 text-center">{qty}</span>
              <button
                onClick={() => onChangeQty(+1)}
                className="w-5 h-5 rounded bg-rp-red text-white text-xs flex items-center justify-center hover:bg-red-700 transition-colors"
              >
                +
              </button>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
