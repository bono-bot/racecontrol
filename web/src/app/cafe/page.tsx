"use client";

import { useEffect, useState } from "react";
import DashboardLayout from "@/components/DashboardLayout";
import { api } from "@/lib/api";
import type {
  CafeItem,
  CafeCategory,
  CreateCafeItemRequest,
  ImportPreview,
  ConfirmedImportRow,
  LowStockItem,
  CafePromo,
  CreateCafePromoRequest,
  PromoType,
  ComboConfig,
  HappyHourConfig,
  GamingBundleConfig,
} from "@/lib/api";
import {
  listCafePromos,
  createCafePromo,
  updateCafePromo,
  deleteCafePromo,
  toggleCafePromo,
  generatePromoGraphic,
  broadcastPromo,
} from "@/lib/api";
import type { BroadcastResult } from "@/lib/api";

const formatRupees = (paise: number) => `\u20b9${(paise / 100).toFixed(2)}`;

interface FormData {
  name: string;
  description: string;
  category_id: string;
  selling_price_rupees: string;
  cost_price_rupees: string;
  is_countable: boolean;
  stock_quantity: string;
  low_stock_threshold: string;
}

const emptyForm: FormData = {
  name: "",
  description: "",
  category_id: "",
  selling_price_rupees: "",
  cost_price_rupees: "",
  is_countable: false,
  stock_quantity: "0",
  low_stock_threshold: "0",
};

type ActiveTab = "items" | "inventory" | "promos" | "marketing";

function getStockStatus(item: CafeItem): {
  label: string;
  color: string;
  priority: number;
} {
  if (!item.is_countable) {
    return { label: "N/A", color: "bg-neutral-700/40 text-neutral-400", priority: 4 };
  }
  if (item.stock_quantity === 0) {
    return { label: "Out of Stock", color: "bg-red-500/20 text-red-400", priority: 0 };
  }
  if (item.stock_quantity <= item.low_stock_threshold) {
    return { label: "Low Stock", color: "bg-red-500/20 text-red-400", priority: 1 };
  }
  if (item.stock_quantity <= item.low_stock_threshold * 2) {
    return { label: "Warning", color: "bg-amber-500/20 text-amber-400", priority: 2 };
  }
  return { label: "In Stock", color: "bg-emerald-500/20 text-emerald-400", priority: 3 };
}

export default function CafePage() {
  const [items, setItems] = useState<CafeItem[]>([]);
  const [categories, setCategories] = useState<CafeCategory[]>([]);
  const [loading, setLoading] = useState(true);
  const [showPanel, setShowPanel] = useState(false);
  const [editItem, setEditItem] = useState<CafeItem | null>(null);
  const [formData, setFormData] = useState<FormData>(emptyForm);
  const [newCategoryName, setNewCategoryName] = useState("");
  const [showNewCategory, setShowNewCategory] = useState(false);
  const [saving, setSaving] = useState(false);
  const [creatingCategory, setCreatingCategory] = useState(false);

  // Tab state
  const [activeTab, setActiveTab] = useState<ActiveTab>("items");

  // Promo state
  const [promos, setPromos] = useState<CafePromo[]>([]);
  const [promoLoading, setPromoLoading] = useState(false);
  const [showPromoPanel, setShowPromoPanel] = useState(false);
  const [editPromo, setEditPromo] = useState<CafePromo | null>(null);
  const [selectedPromoType, setSelectedPromoType] = useState<PromoType>("combo");
  const [promoSaving, setPromoSaving] = useState(false);

  // Marketing tab state
  const [broadcastMessage, setBroadcastMessage] = useState<string>("");
  const [broadcastPromoName, setBroadcastPromoName] = useState<string>("");
  const [broadcastResult, setBroadcastResult] = useState<BroadcastResult | null>(null);
  const [broadcastError, setBroadcastError] = useState<string | null>(null);
  const [broadcastLoading, setBroadcastLoading] = useState<boolean>(false);
  const [generatingPromoId, setGeneratingPromoId] = useState<string | null>(null);
  const [menuGraphicGenerating, setMenuGraphicGenerating] = useState<boolean>(false);

  // Restock inline flow state
  const [restockItemId, setRestockItemId] = useState<string | null>(null);
  const [restockQty, setRestockQty] = useState("");
  const [restocking, setRestocking] = useState(false);

  // Import modal state
  const [showImportModal, setShowImportModal] = useState(false);
  const [importPreview, setImportPreview] = useState<ImportPreview | null>(null);
  const [importLoading, setImportLoading] = useState(false);
  const [importConfirming, setImportConfirming] = useState(false);
  const [uploadingImageId, setUploadingImageId] = useState<string | null>(null);

  // Low-stock banner state
  const [lowStockItems, setLowStockItems] = useState<LowStockItem[]>([]);

  async function loadData() {
    try {
      const [itemsRes, categoriesRes] = await Promise.all([
        api.listCafeItems(),
        api.listCafeCategories(),
      ]);
      setItems(itemsRes.items || []);
      setCategories(categoriesRes.categories || []);
    } catch (err) {
      alert(err instanceof Error ? err.message : "Failed to load cafe data");
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    loadData();
  }, []);

  useEffect(() => {
    if (activeTab !== "promos" && activeTab !== "marketing") return;
    setPromoLoading(true);
    listCafePromos()
      .then(setPromos)
      .catch(console.error)
      .finally(() => setPromoLoading(false));
  }, [activeTab]);

  useEffect(() => {
    let cancelled = false;

    async function fetchLowStock() {
      try {
        const res = await api.listLowStockItems();
        if (!cancelled) {
          setLowStockItems(res.items ?? []);
        }
      } catch {
        // Best-effort: banner simply doesn't show on fetch failure
      }
    }

    fetchLowStock();
    const interval = setInterval(fetchLowStock, 60_000);
    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, []);

  function openAddPanel() {
    setEditItem(null);
    setFormData(emptyForm);
    setShowNewCategory(false);
    setNewCategoryName("");
    setShowPanel(true);
  }

  function openEditPanel(item: CafeItem) {
    setEditItem(item);
    setFormData({
      name: item.name,
      description: item.description || "",
      category_id: item.category_id,
      selling_price_rupees: (item.selling_price_paise / 100).toFixed(2),
      cost_price_rupees: (item.cost_price_paise / 100).toFixed(2),
      is_countable: item.is_countable,
      stock_quantity: String(item.stock_quantity),
      low_stock_threshold: String(item.low_stock_threshold),
    });
    setShowNewCategory(false);
    setNewCategoryName("");
    setShowPanel(true);
  }

  function closePanel() {
    setShowPanel(false);
    setEditItem(null);
    setFormData(emptyForm);
    setShowNewCategory(false);
    setNewCategoryName("");
  }

  async function handleSave() {
    if (!formData.name.trim() || !formData.category_id) return;
    setSaving(true);
    try {
      const payload: CreateCafeItemRequest = {
        name: formData.name.trim(),
        description: formData.description.trim() || undefined,
        category_id: formData.category_id,
        selling_price_paise: Math.round(parseFloat(formData.selling_price_rupees || "0") * 100),
        cost_price_paise: Math.round(parseFloat(formData.cost_price_rupees || "0") * 100),
        is_countable: formData.is_countable,
        stock_quantity: parseInt(formData.stock_quantity || "0", 10),
        low_stock_threshold: parseInt(formData.low_stock_threshold || "0", 10),
      };
      if (editItem) {
        await api.updateCafeItem(editItem.id, payload);
      } else {
        await api.createCafeItem(payload);
      }
      closePanel();
      await loadData();
    } catch (err) {
      alert(err instanceof Error ? err.message : "Failed to save item");
    } finally {
      setSaving(false);
    }
  }

  async function handleDelete(item: CafeItem) {
    if (!window.confirm(`Delete "${item.name}"?`)) return;
    try {
      await api.deleteCafeItem(item.id);
      setItems((prev) => prev.filter((i) => i.id !== item.id));
    } catch (err) {
      alert(err instanceof Error ? err.message : "Failed to delete item");
    }
  }

  async function handleToggle(item: CafeItem) {
    try {
      const res = await api.toggleCafeItem(item.id);
      setItems((prev) =>
        prev.map((i) => (i.id === item.id ? { ...i, is_available: res.is_available } : i))
      );
    } catch (err) {
      alert(err instanceof Error ? err.message : "Failed to toggle item");
    }
  }

  async function handleCreateCategory() {
    if (!newCategoryName.trim()) return;
    setCreatingCategory(true);
    try {
      const res = await api.createCafeCategory(newCategoryName.trim());
      await loadData();
      setFormData((prev) => ({ ...prev, category_id: res.id }));
      setNewCategoryName("");
      setShowNewCategory(false);
    } catch (err) {
      alert(err instanceof Error ? err.message : "Failed to create category");
    } finally {
      setCreatingCategory(false);
    }
  }

  async function handleImageUpload(itemId: string, file: File) {
    setUploadingImageId(itemId);
    try {
      const res = await api.uploadCafeItemImage(itemId, file);
      const filename = res.image_url.split("/").pop() || "";
      setItems((prev) =>
        prev.map((i) => (i.id === itemId ? { ...i, image_path: filename } : i))
      );
    } catch (err) {
      alert(err instanceof Error ? err.message : "Failed to upload image");
    } finally {
      setUploadingImageId(null);
    }
  }

  async function handleImportPreview(file: File) {
    setImportLoading(true);
    try {
      const preview = await api.importCafePreview(file);
      setImportPreview(preview);
    } catch (err) {
      alert(err instanceof Error ? err.message : "Failed to parse file");
    } finally {
      setImportLoading(false);
    }
  }

  async function handleImportConfirm() {
    if (!importPreview) return;
    setImportConfirming(true);
    try {
      const validRows: ConfirmedImportRow[] = importPreview.rows
        .filter((r) => r.valid)
        .map((r) => ({
          name: r.name,
          category: r.category,
          selling_price_paise: Math.round(parseFloat(r.selling_price) * 100),
          cost_price_paise: Math.round(parseFloat(r.cost_price) * 100),
          description: r.description || null,
        }));
      const res = await api.confirmCafeImport(validRows);
      alert(`Successfully imported ${res.imported} items`);
      setShowImportModal(false);
      setImportPreview(null);
      await loadData();
    } catch (err) {
      alert(err instanceof Error ? err.message : "Import failed");
    } finally {
      setImportConfirming(false);
    }
  }

  function closeImportModal() {
    setShowImportModal(false);
    setImportPreview(null);
    setImportLoading(false);
    setImportConfirming(false);
  }

  function getCategoryName(categoryId: string): string {
    return categories.find((c) => c.id === categoryId)?.name || categoryId;
  }

  async function handleRestock(itemId: string) {
    const qty = parseInt(restockQty, 10);
    if (isNaN(qty) || qty <= 0) return;
    setRestocking(true);
    try {
      const updated = await api.restockCafeItem(itemId, qty);
      setItems((prev) => prev.map((i) => (i.id === itemId ? updated : i)));
      setRestockItemId(null);
      setRestockQty("");
    } catch (err) {
      alert(err instanceof Error ? err.message : "Failed to restock item");
    } finally {
      setRestocking(false);
    }
  }

  const inputClass =
    "w-full bg-[#1A1A1A] border border-[#333333] rounded-lg px-3 py-2 text-sm text-neutral-200 placeholder-[#5A5A5A] focus:outline-none focus:border-[#E10600] transition-colors";

  const displayRows = importPreview ? importPreview.rows.slice(0, 100) : [];

  // Inventory tab computed values
  const countableItems = items.filter((i) => i.is_countable);
  const belowThreshold = countableItems.filter(
    (i) => i.stock_quantity > 0 && i.stock_quantity <= i.low_stock_threshold
  );
  const outOfStock = countableItems.filter((i) => i.stock_quantity === 0);

  const sortedInventoryItems = [...items].sort((a, b) => {
    const pa = getStockStatus(a).priority;
    const pb = getStockStatus(b).priority;
    return pa - pb;
  });

  return (
    <DashboardLayout>
      {/* Header */}
      <div className="flex items-center justify-between mb-4">
        <div>
          <h1 className="text-2xl font-bold text-white">Cafe Menu</h1>
          <p className="text-sm text-[#5A5A5A]">Manage cafe items and categories</p>
        </div>
        <div className="flex items-center gap-3">
          <button
            onClick={() => setShowImportModal(true)}
            className="bg-[#222222] border border-[#333333] text-neutral-300 text-sm font-semibold px-4 py-2 rounded-lg hover:border-[#E10600] hover:text-white transition-colors"
          >
            Import
          </button>
          <button
            onClick={openAddPanel}
            className="bg-[#E10600] text-white text-sm font-semibold px-4 py-2 rounded-lg hover:bg-[#c40500] transition-colors"
          >
            Add Item
          </button>
        </div>
      </div>

      {/* Low Stock Warning Banner */}
      {lowStockItems.length > 0 && (
        <div
          className="mb-4 rounded-lg border border-red-500/40 bg-red-500/10 px-4 py-3"
          role="alert"
          aria-label="Low stock warning"
        >
          <div className="flex items-center gap-2 mb-2">
            <span className="text-red-400 font-semibold text-sm uppercase tracking-wide">
              Low Stock Warning
            </span>
            <span className="text-red-400/70 text-xs">
              ({lowStockItems.length} item{lowStockItems.length !== 1 ? "s" : ""})
            </span>
          </div>
          <ul className="space-y-1">
            {lowStockItems.map((item) => (
              <li key={item.id} className="text-sm text-red-300">
                <span className="font-medium">{item.name}</span>
                {" — "}
                <span className="text-red-400/80">
                  {item.stock_quantity} remaining (threshold: {item.low_stock_threshold})
                </span>
              </li>
            ))}
          </ul>
        </div>
      )}

      {/* Tab Navigation */}
      <div className="flex border-b border-[#333333] mb-6">
        <button
          onClick={() => setActiveTab("items")}
          className={`px-5 py-2.5 text-sm font-semibold transition-colors ${
            activeTab === "items"
              ? "border-b-2 border-[#E10600] text-white"
              : "text-[#5A5A5A] hover:text-neutral-300"
          }`}
        >
          Items
        </button>
        <button
          onClick={() => setActiveTab("inventory")}
          className={`px-5 py-2.5 text-sm font-semibold transition-colors ${
            activeTab === "inventory"
              ? "border-b-2 border-[#E10600] text-white"
              : "text-[#5A5A5A] hover:text-neutral-300"
          }`}
        >
          Inventory
        </button>
        <button
          onClick={() => setActiveTab("promos")}
          className={`px-5 py-2.5 text-sm font-semibold transition-colors ${
            activeTab === "promos"
              ? "border-b-2 border-[#E10600] text-white"
              : "text-[#5A5A5A] hover:text-neutral-300"
          }`}
        >
          Promos
        </button>
        <button
          onClick={() => setActiveTab("marketing")}
          className={`px-5 py-2.5 text-sm font-semibold transition-colors ${
            activeTab === "marketing"
              ? "border-b-2 border-[#E10600] text-white"
              : "text-[#5A5A5A] hover:text-neutral-300"
          }`}
        >
          Marketing
        </button>
      </div>

      {/* ── ITEMS TAB ── */}
      {activeTab === "items" && (
        <div className="flex gap-6">
          {/* Items table */}
          <div className="flex-1 min-w-0">
            {loading ? (
              <div className="text-center py-12 text-[#5A5A5A] text-sm">Loading...</div>
            ) : items.length === 0 ? (
              <div className="bg-[#222222] border border-[#333333] rounded-lg p-8 text-center">
                <p className="text-neutral-400 mb-2">No cafe items yet.</p>
                <p className="text-[#5A5A5A] text-sm">
                  Click &apos;Add Item&apos; to get started.
                </p>
              </div>
            ) : (
              <div className="bg-[#222222] border border-[#333333] rounded-lg overflow-hidden">
                <table className="w-full text-sm">
                  <thead>
                    <tr className="border-b border-[#333333]">
                      <th className="text-left px-4 py-3 text-xs font-medium text-[#5A5A5A] uppercase tracking-wider">
                        Name
                      </th>
                      <th className="text-left px-4 py-3 text-xs font-medium text-[#5A5A5A] uppercase tracking-wider">
                        Image
                      </th>
                      <th className="text-left px-4 py-3 text-xs font-medium text-[#5A5A5A] uppercase tracking-wider">
                        Category
                      </th>
                      <th className="text-left px-4 py-3 text-xs font-medium text-[#5A5A5A] uppercase tracking-wider">
                        Selling Price
                      </th>
                      <th className="text-left px-4 py-3 text-xs font-medium text-[#5A5A5A] uppercase tracking-wider">
                        Cost Price
                      </th>
                      <th className="text-left px-4 py-3 text-xs font-medium text-[#5A5A5A] uppercase tracking-wider">
                        Status
                      </th>
                      <th className="text-left px-4 py-3 text-xs font-medium text-[#5A5A5A] uppercase tracking-wider">
                        Type
                      </th>
                      <th className="text-left px-4 py-3 text-xs font-medium text-[#5A5A5A] uppercase tracking-wider">
                        Stock
                      </th>
                      <th className="text-right px-4 py-3 text-xs font-medium text-[#5A5A5A] uppercase tracking-wider">
                        Actions
                      </th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-[#333333]/50">
                    {items.map((item) => (
                      <tr key={item.id} className="hover:bg-[#1A1A1A]/50 transition-colors">
                        <td className="px-4 py-3 text-neutral-200 font-medium">{item.name}</td>
                        <td className="px-4 py-3">
                          <div className="flex items-center gap-2">
                            {item.image_path ? (
                              <img
                                src={`/static/cafe-images/${item.image_path}`}
                                alt={item.name}
                                className="w-10 h-10 rounded object-cover"
                              />
                            ) : (
                              <div className="w-10 h-10 rounded bg-[#1A1A1A] border border-[#333333] flex items-center justify-center text-[#5A5A5A] text-xs">
                                No img
                              </div>
                            )}
                            <label
                              className="cursor-pointer text-[#5A5A5A] hover:text-[#E10600] transition-colors"
                              title="Upload image"
                            >
                              <svg
                                xmlns="http://www.w3.org/2000/svg"
                                className="w-4 h-4"
                                fill="none"
                                viewBox="0 0 24 24"
                                stroke="currentColor"
                                strokeWidth={2}
                              >
                                <path
                                  strokeLinecap="round"
                                  strokeLinejoin="round"
                                  d="M3 9a2 2 0 012-2h.93a2 2 0 001.664-.89l.812-1.22A2 2 0 0110.07 4h3.86a2 2 0 011.664.89l.812 1.22A2 2 0 0018.07 7H19a2 2 0 012 2v9a2 2 0 01-2 2H5a2 2 0 01-2-2V9z"
                                />
                                <path
                                  strokeLinecap="round"
                                  strokeLinejoin="round"
                                  d="M15 13a3 3 0 11-6 0 3 3 0 016 0z"
                                />
                              </svg>
                              <input
                                type="file"
                                accept="image/jpeg,image/png,image/webp"
                                className="hidden"
                                disabled={uploadingImageId === item.id}
                                onChange={(e) => {
                                  const file = e.target.files?.[0];
                                  if (file) handleImageUpload(item.id, file);
                                  e.target.value = "";
                                }}
                              />
                            </label>
                          </div>
                        </td>
                        <td className="px-4 py-3 text-neutral-400">
                          {getCategoryName(item.category_id)}
                        </td>
                        <td className="px-4 py-3 text-neutral-300 font-mono">
                          {formatRupees(item.selling_price_paise)}
                        </td>
                        <td className="px-4 py-3 text-neutral-400 font-mono">
                          {formatRupees(item.cost_price_paise)}
                        </td>
                        <td className="px-4 py-3">
                          {item.is_available ? (
                            <span className="bg-emerald-500/20 text-emerald-400 text-xs font-semibold px-2 py-0.5 rounded">
                              Available
                            </span>
                          ) : (
                            <span className="bg-red-500/20 text-red-400 text-xs font-semibold px-2 py-0.5 rounded">
                              Unavailable
                            </span>
                          )}
                        </td>
                        {/* Type column */}
                        <td className="px-4 py-3">
                          {item.is_countable ? (
                            <span className="bg-blue-500/20 text-blue-400 text-xs font-semibold px-2 py-0.5 rounded">
                              Countable
                            </span>
                          ) : (
                            <span className="bg-neutral-700/40 text-neutral-400 text-xs font-semibold px-2 py-0.5 rounded">
                              Uncountable
                            </span>
                          )}
                        </td>
                        {/* Stock column */}
                        <td className="px-4 py-3">
                          {item.is_countable ? (
                            <div className="flex items-center gap-2">
                              {restockItemId === item.id ? (
                                <div className="flex items-center gap-1">
                                  <input
                                    type="number"
                                    min={1}
                                    value={restockQty}
                                    onChange={(e) => setRestockQty(e.target.value)}
                                    onKeyDown={(e) => {
                                      if (e.key === "Enter") handleRestock(item.id);
                                      if (e.key === "Escape") {
                                        setRestockItemId(null);
                                        setRestockQty("");
                                      }
                                    }}
                                    placeholder="qty"
                                    className="w-16 bg-[#1A1A1A] border border-[#333333] rounded px-2 py-1 text-xs text-neutral-200 focus:outline-none focus:border-[#E10600]"
                                    autoFocus
                                  />
                                  <button
                                    onClick={() => handleRestock(item.id)}
                                    disabled={restocking}
                                    className="px-2 py-1 bg-[#E10600] text-white text-xs font-semibold rounded hover:bg-[#c40500] disabled:opacity-50 transition-colors"
                                  >
                                    {restocking ? "..." : "Add"}
                                  </button>
                                  <button
                                    onClick={() => {
                                      setRestockItemId(null);
                                      setRestockQty("");
                                    }}
                                    className="px-2 py-1 text-[#5A5A5A] hover:text-neutral-300 text-xs transition-colors"
                                  >
                                    Cancel
                                  </button>
                                </div>
                              ) : (
                                <>
                                  <span className="text-neutral-300 font-mono text-sm">
                                    {item.stock_quantity}
                                  </span>
                                  <button
                                    onClick={() => {
                                      setRestockItemId(item.id);
                                      setRestockQty("");
                                    }}
                                    className="text-xs text-[#5A5A5A] hover:text-[#E10600] transition-colors"
                                    title="Restock"
                                  >
                                    +
                                  </button>
                                </>
                              )}
                            </div>
                          ) : (
                            <span className="text-[#5A5A5A]">—</span>
                          )}
                        </td>
                        <td className="px-4 py-3 text-right">
                          <div className="flex items-center justify-end gap-3">
                            <button
                              onClick={() => openEditPanel(item)}
                              className="text-xs text-neutral-400 hover:text-[#E10600] font-medium transition-colors"
                            >
                              Edit
                            </button>
                            <button
                              onClick={() => handleToggle(item)}
                              className="text-xs text-neutral-400 hover:text-amber-400 font-medium transition-colors"
                            >
                              {item.is_available ? "Disable" : "Enable"}
                            </button>
                            <button
                              onClick={() => handleDelete(item)}
                              className="text-xs text-[#5A5A5A] hover:text-red-400 font-medium transition-colors"
                            >
                              Delete
                            </button>
                          </div>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </div>

          {/* Side panel */}
          {showPanel && (
            <div className="w-96 bg-[#222222] border border-[#333333] rounded-lg p-5 flex-shrink-0">
              <h2 className="text-lg font-bold text-white mb-4">
                {editItem ? "Edit Item" : "Add Item"}
              </h2>

              <div className="space-y-4">
                {/* Name */}
                <div>
                  <label className="block text-xs text-[#5A5A5A] mb-1">
                    Name <span className="text-[#E10600]">*</span>
                  </label>
                  <input
                    type="text"
                    placeholder="Item name"
                    value={formData.name}
                    onChange={(e) => setFormData((prev) => ({ ...prev, name: e.target.value }))}
                    className={inputClass}
                  />
                </div>

                {/* Description */}
                <div>
                  <label className="block text-xs text-[#5A5A5A] mb-1">Description</label>
                  <textarea
                    placeholder="Brief description"
                    value={formData.description}
                    onChange={(e) =>
                      setFormData((prev) => ({ ...prev, description: e.target.value }))
                    }
                    rows={2}
                    className={inputClass + " resize-none"}
                  />
                </div>

                {/* Category */}
                <div>
                  <label className="block text-xs text-[#5A5A5A] mb-1">
                    Category <span className="text-[#E10600]">*</span>
                  </label>
                  <div className="flex gap-2">
                    <select
                      value={formData.category_id}
                      onChange={(e) =>
                        setFormData((prev) => ({ ...prev, category_id: e.target.value }))
                      }
                      className={inputClass}
                    >
                      <option value="">Select category</option>
                      {categories.map((cat) => (
                        <option key={cat.id} value={cat.id}>
                          {cat.name}
                        </option>
                      ))}
                    </select>
                    <button
                      onClick={() => setShowNewCategory(!showNewCategory)}
                      title="Add new category"
                      className="flex-shrink-0 w-9 h-9 flex items-center justify-center bg-[#1A1A1A] border border-[#333333] rounded-lg text-neutral-400 hover:text-[#E10600] hover:border-[#E10600] transition-colors text-base font-bold"
                    >
                      +
                    </button>
                  </div>

                  {/* Inline new category creation */}
                  {showNewCategory && (
                    <div className="mt-2 flex gap-2">
                      <input
                        type="text"
                        placeholder="New category name"
                        value={newCategoryName}
                        onChange={(e) => setNewCategoryName(e.target.value)}
                        onKeyDown={(e) => {
                          if (e.key === "Enter") handleCreateCategory();
                        }}
                        className={inputClass}
                      />
                      <button
                        onClick={handleCreateCategory}
                        disabled={!newCategoryName.trim() || creatingCategory}
                        className="flex-shrink-0 px-3 py-2 bg-[#E10600] text-white text-xs font-semibold rounded-lg hover:bg-[#c40500] disabled:opacity-50 transition-colors"
                      >
                        {creatingCategory ? "..." : "Add"}
                      </button>
                    </div>
                  )}
                </div>

                {/* Selling Price */}
                <div>
                  <label className="block text-xs text-[#5A5A5A] mb-1">Selling Price (rupees)</label>
                  <input
                    type="number"
                    min={0}
                    step={0.5}
                    placeholder="0.00"
                    value={formData.selling_price_rupees}
                    onChange={(e) =>
                      setFormData((prev) => ({ ...prev, selling_price_rupees: e.target.value }))
                    }
                    className={inputClass}
                  />
                </div>

                {/* Cost Price */}
                <div>
                  <label className="block text-xs text-[#5A5A5A] mb-1">Cost Price (rupees)</label>
                  <input
                    type="number"
                    min={0}
                    step={0.5}
                    placeholder="0.00"
                    value={formData.cost_price_rupees}
                    onChange={(e) =>
                      setFormData((prev) => ({ ...prev, cost_price_rupees: e.target.value }))
                    }
                    className={inputClass}
                  />
                </div>

                {/* Countable toggle */}
                <div>
                  <label className="flex items-center gap-3 cursor-pointer">
                    <input
                      type="checkbox"
                      checked={formData.is_countable}
                      onChange={(e) =>
                        setFormData((prev) => ({ ...prev, is_countable: e.target.checked }))
                      }
                      className="w-4 h-4 accent-[#E10600]"
                    />
                    <span className="text-sm text-neutral-300">Countable item (track stock)</span>
                  </label>
                </div>

                {/* Stock fields — only when countable */}
                {formData.is_countable && (
                  <>
                    <div>
                      <label className="block text-xs text-[#5A5A5A] mb-1">
                        {editItem ? "Add to Stock" : "Initial Stock"}
                      </label>
                      <input
                        type="number"
                        min={0}
                        placeholder="0"
                        value={formData.stock_quantity}
                        onChange={(e) =>
                          setFormData((prev) => ({ ...prev, stock_quantity: e.target.value }))
                        }
                        className={inputClass}
                      />
                    </div>
                    <div>
                      <label className="block text-xs text-[#5A5A5A] mb-1">Low Stock Threshold</label>
                      <input
                        type="number"
                        min={0}
                        placeholder="0"
                        value={formData.low_stock_threshold}
                        onChange={(e) =>
                          setFormData((prev) => ({ ...prev, low_stock_threshold: e.target.value }))
                        }
                        className={inputClass}
                      />
                    </div>
                  </>
                )}

                {/* Buttons */}
                <div className="flex gap-3 pt-2">
                  <button
                    onClick={handleSave}
                    disabled={!formData.name.trim() || !formData.category_id || saving}
                    className="flex-1 bg-[#E10600] text-white text-sm font-semibold py-2 rounded-lg hover:bg-[#c40500] disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
                  >
                    {saving ? "Saving..." : "Save"}
                  </button>
                  <button
                    onClick={closePanel}
                    className="flex-1 bg-[#1A1A1A] border border-[#333333] text-neutral-400 text-sm font-semibold py-2 rounded-lg hover:text-white hover:border-neutral-500 transition-colors"
                  >
                    Cancel
                  </button>
                </div>
              </div>
            </div>
          )}
        </div>
      )}

      {/* ── INVENTORY TAB ── */}
      {activeTab === "inventory" && (
        <div>
          {loading ? (
            <div className="text-center py-12 text-[#5A5A5A] text-sm">Loading...</div>
          ) : (
            <>
              {/* Summary stats */}
              <div className="grid grid-cols-4 gap-4 mb-6">
                <div className="bg-[#222222] border border-[#333333] rounded-lg p-4">
                  <div className="text-xs text-[#5A5A5A] uppercase tracking-wider mb-1">
                    Total Items
                  </div>
                  <div className="text-2xl font-bold text-white">{items.length}</div>
                </div>
                <div className="bg-[#222222] border border-[#333333] rounded-lg p-4">
                  <div className="text-xs text-[#5A5A5A] uppercase tracking-wider mb-1">
                    Countable
                  </div>
                  <div className="text-2xl font-bold text-blue-400">{countableItems.length}</div>
                </div>
                <div className="bg-[#222222] border border-[#333333] rounded-lg p-4">
                  <div className="text-xs text-[#5A5A5A] uppercase tracking-wider mb-1">
                    Below Threshold
                  </div>
                  <div className="text-2xl font-bold text-amber-400">{belowThreshold.length}</div>
                </div>
                <div className="bg-[#222222] border border-[#333333] rounded-lg p-4">
                  <div className="text-xs text-[#5A5A5A] uppercase tracking-wider mb-1">
                    Out of Stock
                  </div>
                  <div className="text-2xl font-bold text-red-400">{outOfStock.length}</div>
                </div>
              </div>

              {/* Inventory table */}
              {sortedInventoryItems.length === 0 ? (
                <div className="bg-[#222222] border border-[#333333] rounded-lg p-8 text-center">
                  <p className="text-neutral-400">No items yet.</p>
                </div>
              ) : (
                <div className="bg-[#222222] border border-[#333333] rounded-lg overflow-hidden">
                  <table className="w-full text-sm">
                    <thead>
                      <tr className="border-b border-[#333333]">
                        <th className="text-left px-4 py-3 text-xs font-medium text-[#5A5A5A] uppercase tracking-wider">
                          Name
                        </th>
                        <th className="text-left px-4 py-3 text-xs font-medium text-[#5A5A5A] uppercase tracking-wider">
                          Category
                        </th>
                        <th className="text-left px-4 py-3 text-xs font-medium text-[#5A5A5A] uppercase tracking-wider">
                          Type
                        </th>
                        <th className="text-left px-4 py-3 text-xs font-medium text-[#5A5A5A] uppercase tracking-wider">
                          Stock
                        </th>
                        <th className="text-left px-4 py-3 text-xs font-medium text-[#5A5A5A] uppercase tracking-wider">
                          Threshold
                        </th>
                        <th className="text-left px-4 py-3 text-xs font-medium text-[#5A5A5A] uppercase tracking-wider">
                          Status
                        </th>
                      </tr>
                    </thead>
                    <tbody className="divide-y divide-[#333333]/50">
                      {sortedInventoryItems.map((item) => {
                        const status = getStockStatus(item);
                        return (
                          <tr
                            key={item.id}
                            className="hover:bg-[#1A1A1A]/50 transition-colors"
                          >
                            <td className="px-4 py-3 text-neutral-200 font-medium">{item.name}</td>
                            <td className="px-4 py-3 text-neutral-400">
                              {getCategoryName(item.category_id)}
                            </td>
                            <td className="px-4 py-3">
                              {item.is_countable ? (
                                <span className="bg-blue-500/20 text-blue-400 text-xs font-semibold px-2 py-0.5 rounded">
                                  Countable
                                </span>
                              ) : (
                                <span className="bg-neutral-700/40 text-neutral-400 text-xs font-semibold px-2 py-0.5 rounded">
                                  Uncountable
                                </span>
                              )}
                            </td>
                            <td className="px-4 py-3 font-mono">
                              {item.is_countable ? (
                                <span className="text-neutral-300">{item.stock_quantity}</span>
                              ) : (
                                <span className="text-[#5A5A5A]">—</span>
                              )}
                            </td>
                            <td className="px-4 py-3 font-mono">
                              {item.is_countable ? (
                                <span className="text-neutral-400">{item.low_stock_threshold}</span>
                              ) : (
                                <span className="text-[#5A5A5A]">—</span>
                              )}
                            </td>
                            <td className="px-4 py-3">
                              <span
                                className={`text-xs font-semibold px-2 py-0.5 rounded ${status.color}`}
                              >
                                {status.label}
                              </span>
                            </td>
                          </tr>
                        );
                      })}
                    </tbody>
                  </table>
                </div>
              )}
            </>
          )}
        </div>
      )}

      {/* ── PROMOS TAB ── */}
      {activeTab === "promos" && (
        <div>
          <div className="flex items-center justify-between mb-4">
            <h2 className="text-lg font-semibold text-white">Promotions</h2>
            <button
              onClick={() => { setEditPromo(null); setSelectedPromoType("combo"); setShowPromoPanel(true); }}
              className="px-4 py-2 text-sm font-semibold bg-[#E10600] text-white rounded hover:bg-red-700 transition-colors"
            >
              + New Promo
            </button>
          </div>

          {promoLoading ? (
            <p className="text-[#5A5A5A]">Loading...</p>
          ) : promos.length === 0 ? (
            <p className="text-[#5A5A5A] text-sm">No promos yet. Create one to get started.</p>
          ) : (
            <table className="w-full text-sm">
              <thead>
                <tr className="text-left text-[#5A5A5A] border-b border-[#333333]">
                  <th className="pb-2 pr-4">Name</th>
                  <th className="pb-2 pr-4">Type</th>
                  <th className="pb-2 pr-4">Window</th>
                  <th className="pb-2 pr-4">Group</th>
                  <th className="pb-2 pr-4">Status</th>
                  <th className="pb-2">Actions</th>
                </tr>
              </thead>
              <tbody>
                {promos.map((promo) => (
                  <tr key={promo.id} className="border-b border-[#333333]/50 hover:bg-[#222222]/50">
                    <td className="py-2 pr-4 text-white font-medium">{promo.name}</td>
                    <td className="py-2 pr-4 text-neutral-300 capitalize">{promo.promo_type.replace("_", " ")}</td>
                    <td className="py-2 pr-4 text-neutral-400 text-xs">
                      {promo.start_time && promo.end_time
                        ? `${promo.start_time}–${promo.end_time} IST`
                        : "—"}
                    </td>
                    <td className="py-2 pr-4 text-neutral-400 text-xs">{promo.stacking_group ?? "—"}</td>
                    <td className="py-2 pr-4">
                      <span className={`px-2 py-0.5 rounded text-xs font-medium ${
                        promo.is_active
                          ? "bg-emerald-500/20 text-emerald-400"
                          : "bg-neutral-700/40 text-neutral-400"
                      }`}>
                        {promo.is_active ? "Active" : "Inactive"}
                      </span>
                    </td>
                    <td className="py-2 flex gap-2">
                      <button
                        onClick={() => toggleCafePromo(promo.id).then(() =>
                          listCafePromos().then(setPromos)
                        )}
                        className="text-xs text-[#5A5A5A] hover:text-white transition-colors"
                      >
                        {promo.is_active ? "Deactivate" : "Activate"}
                      </button>
                      <button
                        onClick={() => {
                          setEditPromo(promo);
                          setSelectedPromoType(promo.promo_type);
                          setShowPromoPanel(true);
                        }}
                        className="text-xs text-[#5A5A5A] hover:text-white transition-colors"
                      >
                        Edit
                      </button>
                      <button
                        onClick={() => {
                          if (!confirm(`Delete "${promo.name}"?`)) return;
                          deleteCafePromo(promo.id).then(() =>
                            setPromos((prev) => prev.filter((p) => p.id !== promo.id))
                          );
                        }}
                        className="text-xs text-red-500 hover:text-red-400 transition-colors"
                      >
                        Delete
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}

          {/* Promo create/edit panel */}
          {showPromoPanel && (
            <PromoPanel
              promo={editPromo}
              promoType={selectedPromoType}
              items={items}
              categories={categories}
              onTypeChange={setSelectedPromoType}
              onSave={async (data: CreateCafePromoRequest) => {
                setPromoSaving(true);
                try {
                  if (editPromo) {
                    await updateCafePromo(editPromo.id, data);
                  } else {
                    await createCafePromo(data);
                  }
                  const updated = await listCafePromos();
                  setPromos(updated);
                  setShowPromoPanel(false);
                  setEditPromo(null);
                } catch (e) {
                  console.error(e);
                } finally {
                  setPromoSaving(false);
                }
              }}
              onClose={() => { setShowPromoPanel(false); setEditPromo(null); }}
              saving={promoSaving}
            />
          )}
        </div>
      )}

      {/* ── MARKETING TAB ── */}
      {activeTab === "marketing" && (
        <div className="space-y-8">

          {/* Section A — Promo Graphics */}
          <div>
            <h2 className="text-lg font-bold text-white mb-4" style={{ fontFamily: "Montserrat, sans-serif" }}>
              Promo Graphics
            </h2>
            {promoLoading ? (
              <div className="text-center py-8 text-[#5A5A5A] text-sm">Loading promos...</div>
            ) : promos.length === 0 ? (
              <div className="bg-[#222222] border border-[#333333] rounded-lg p-6 text-center text-neutral-400 text-sm">
                No promos yet. Create promos in the Promos tab first.
              </div>
            ) : (
              <div className="grid grid-cols-1 gap-3">
                {promos.map((promo) => (
                  <div
                    key={promo.id}
                    className="bg-[#222222] border border-[#333333] rounded-lg px-4 py-3 flex items-center justify-between"
                  >
                    <div className="flex items-center gap-3">
                      <span className="text-white font-medium">{promo.name}</span>
                      <span className="bg-neutral-700/40 text-neutral-400 text-xs font-semibold px-2 py-0.5 rounded capitalize">
                        {promo.promo_type.replace(/_/g, " ")}
                      </span>
                    </div>
                    <button
                      disabled={generatingPromoId === promo.id}
                      onClick={async () => {
                        setGeneratingPromoId(promo.id);
                        try {
                          const blob = await generatePromoGraphic({
                            template: "promo",
                            promo_name: promo.name,
                            price_label: promo.promo_type === "happy_hour" ? "Special Offer" : "Bundle Deal",
                            time_label: promo.start_time ?? undefined,
                          });
                          const url = URL.createObjectURL(blob);
                          const a = document.createElement("a");
                          a.href = url;
                          a.download = `${promo.name.replace(/\s+/g, "_")}_promo.png`;
                          a.click();
                          URL.revokeObjectURL(url);
                        } catch (err) {
                          alert(err instanceof Error ? err.message : "Failed to generate graphic");
                        } finally {
                          setGeneratingPromoId(null);
                        }
                      }}
                      className="flex items-center gap-2 bg-[#E10600] text-white text-xs font-semibold px-3 py-1.5 rounded hover:bg-[#c40500] disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
                    >
                      {generatingPromoId === promo.id ? (
                        <>
                          <svg className="animate-spin w-3 h-3" fill="none" viewBox="0 0 24 24">
                            <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                            <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8H4z" />
                          </svg>
                          Generating...
                        </>
                      ) : (
                        "Generate PNG"
                      )}
                    </button>
                  </div>
                ))}
              </div>
            )}
          </div>

          {/* Section B — Daily Menu Graphic */}
          <div>
            <h2 className="text-lg font-bold text-white mb-4" style={{ fontFamily: "Montserrat, sans-serif" }}>
              Daily Menu Graphic
            </h2>
            <div className="bg-[#222222] border border-[#333333] rounded-lg px-4 py-4 flex items-center justify-between">
              <p className="text-neutral-400 text-sm">Generate a daily menu PNG with Racing Point branding.</p>
              <button
                disabled={menuGraphicGenerating}
                onClick={async () => {
                  setMenuGraphicGenerating(true);
                  try {
                    const blob = await generatePromoGraphic({ template: "daily_menu" });
                    const url = URL.createObjectURL(blob);
                    const a = document.createElement("a");
                    a.href = url;
                    a.download = "daily_menu.png";
                    a.click();
                    URL.revokeObjectURL(url);
                  } catch (err) {
                    alert(err instanceof Error ? err.message : "Failed to generate menu graphic");
                  } finally {
                    setMenuGraphicGenerating(false);
                  }
                }}
                className="flex items-center gap-2 bg-[#E10600] text-white text-xs font-semibold px-3 py-1.5 rounded hover:bg-[#c40500] disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
              >
                {menuGraphicGenerating ? (
                  <>
                    <svg className="animate-spin w-3 h-3" fill="none" viewBox="0 0 24 24">
                      <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                      <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8H4z" />
                    </svg>
                    Generating...
                  </>
                ) : (
                  "Generate Menu PNG"
                )}
              </button>
            </div>
          </div>

          {/* Section C — WhatsApp Broadcast */}
          <div>
            <h2 className="text-lg font-bold text-white mb-1" style={{ fontFamily: "Montserrat, sans-serif" }}>
              WhatsApp Broadcast
            </h2>
            <p className="text-red-400 text-xs font-medium mb-4">
              Sends to ALL customers with a phone number. 24-hour cooldown per customer.
            </p>
            <div className="bg-[#222222] border border-[#333333] rounded-lg p-4 space-y-4">
              <div>
                <label className="block text-xs text-[#5A5A5A] mb-1">Message</label>
                <textarea
                  rows={4}
                  placeholder="Enter your promo message..."
                  value={broadcastMessage}
                  onChange={(e) => {
                    setBroadcastMessage(e.target.value);
                    setBroadcastResult(null);
                    setBroadcastError(null);
                  }}
                  className="w-full bg-[#1A1A1A] border border-[#333333] rounded-lg px-3 py-2 text-sm text-neutral-200 placeholder-[#5A5A5A] focus:outline-none focus:border-[#E10600] transition-colors resize-none"
                />
              </div>
              <div>
                <label className="block text-xs text-[#5A5A5A] mb-1">Promo Name (optional)</label>
                <input
                  type="text"
                  placeholder="e.g. Happy Hour Deal"
                  value={broadcastPromoName}
                  onChange={(e) => setBroadcastPromoName(e.target.value)}
                  className="w-full bg-[#1A1A1A] border border-[#333333] rounded-lg px-3 py-2 text-sm text-neutral-200 placeholder-[#5A5A5A] focus:outline-none focus:border-[#E10600] transition-colors"
                />
              </div>

              {broadcastError && (
                <p className="text-red-400 text-sm">{broadcastError}</p>
              )}

              {broadcastResult && (
                <div className="bg-emerald-500/10 border border-emerald-500/30 rounded-lg px-4 py-3 text-sm text-emerald-300">
                  <span className="font-semibold">Sent:</span> {broadcastResult.sent}
                  {" | "}
                  <span className="font-semibold">Skipped (cooldown):</span> {broadcastResult.skipped_cooldown}
                  {" | "}
                  <span className="font-semibold">No phone:</span> {broadcastResult.skipped_no_phone}
                  {" | "}
                  <span className="font-semibold">Total:</span> {broadcastResult.attempted}
                </div>
              )}

              <button
                disabled={broadcastLoading}
                onClick={async () => {
                  if (!broadcastMessage.trim()) {
                    setBroadcastError("Message cannot be empty");
                    return;
                  }
                  setBroadcastLoading(true);
                  setBroadcastError(null);
                  setBroadcastResult(null);
                  try {
                    const result = await broadcastPromo(
                      broadcastMessage,
                      broadcastPromoName.trim() || undefined
                    );
                    setBroadcastResult(result);
                  } catch (err) {
                    setBroadcastError(err instanceof Error ? err.message : "Broadcast failed");
                  } finally {
                    setBroadcastLoading(false);
                  }
                }}
                className="flex items-center gap-2 bg-[#E10600] text-white text-sm font-semibold px-4 py-2 rounded-lg hover:bg-[#c40500] disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
              >
                {broadcastLoading ? (
                  <>
                    <svg className="animate-spin w-4 h-4" fill="none" viewBox="0 0 24 24">
                      <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                      <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8H4z" />
                    </svg>
                    Sending...
                  </>
                ) : (
                  "Send Broadcast"
                )}
              </button>
            </div>
          </div>

        </div>
      )}

      {/* Import Modal */}
      {showImportModal && (
        <div
          className="fixed inset-0 bg-black/60 z-50 flex items-center justify-center p-4"
          onClick={closeImportModal}
        >
          <div
            className="bg-[#222222] border border-[#333333] rounded-xl max-w-4xl w-full max-h-[85vh] overflow-hidden flex flex-col"
            onClick={(e) => e.stopPropagation()}
          >
            {/* Modal Header */}
            <div className="flex items-center justify-between px-6 py-4 border-b border-[#333333]">
              <h2 className="text-lg font-bold text-white">Import Menu Items</h2>
              <button
                onClick={closeImportModal}
                className="text-[#5A5A5A] hover:text-white transition-colors text-xl leading-none"
              >
                &times;
              </button>
            </div>

            {/* Modal Body */}
            <div className="flex-1 overflow-y-auto p-6">
              {importPreview === null ? (
                /* Step 1: File Upload */
                <div>
                  {importLoading ? (
                    <div className="text-center py-16 text-[#5A5A5A] text-sm">
                      Parsing file...
                    </div>
                  ) : (
                    <label className="block cursor-pointer">
                      <div className="border-2 border-dashed border-[#333333] rounded-xl p-16 text-center hover:border-[#E10600] transition-colors">
                        <div className="text-[#5A5A5A] text-sm mb-2">
                          Drop an XLSX or CSV file here, or click to browse
                        </div>
                        <div className="text-[#5A5A5A] text-xs">
                          Supported columns: Name, Category, Selling Price, Cost Price, Description
                        </div>
                      </div>
                      <input
                        type="file"
                        accept=".xlsx,.csv"
                        className="hidden"
                        onChange={(e) => {
                          const file = e.target.files?.[0];
                          if (file) handleImportPreview(file);
                          e.target.value = "";
                        }}
                      />
                    </label>
                  )}
                </div>
              ) : (
                /* Step 2: Preview Table */
                <div>
                  {/* Summary */}
                  <div className="flex items-center gap-4 mb-4">
                    <span className="text-sm text-neutral-300">
                      <span className="text-emerald-400 font-semibold">{importPreview.valid_rows} valid</span>
                      {importPreview.invalid_rows > 0 && (
                        <>, <span className="text-red-400 font-semibold">{importPreview.invalid_rows} invalid</span></>
                      )}
                      <span className="text-[#5A5A5A]"> of {importPreview.total_rows} total rows</span>
                    </span>
                  </div>

                  {/* Column mapping bar */}
                  {importPreview.columns.length > 0 && (
                    <div className="flex flex-wrap gap-2 mb-4">
                      {importPreview.columns.map((col) => (
                        <span
                          key={col.index}
                          className={`text-xs px-2 py-1 rounded font-medium ${
                            col.mapped_to
                              ? "bg-[#1A1A1A] border border-[#333333] text-neutral-300"
                              : "bg-amber-500/10 border border-amber-500/30 text-amber-400"
                          }`}
                        >
                          {col.mapped_to ? `${col.mapped_to}: ${col.header}` : `? ${col.header}`}
                        </span>
                      ))}
                    </div>
                  )}

                  {/* Preview table */}
                  <div className="max-h-96 overflow-y-auto border border-[#333333] rounded-lg">
                    <table className="w-full text-xs">
                      <thead className="sticky top-0 bg-[#1A1A1A]">
                        <tr className="border-b border-[#333333]">
                          <th className="text-left px-3 py-2 text-[#5A5A5A] uppercase tracking-wider font-medium">#</th>
                          <th className="text-left px-3 py-2 text-[#5A5A5A] uppercase tracking-wider font-medium">Name</th>
                          <th className="text-left px-3 py-2 text-[#5A5A5A] uppercase tracking-wider font-medium">Category</th>
                          <th className="text-left px-3 py-2 text-[#5A5A5A] uppercase tracking-wider font-medium">Sell Price</th>
                          <th className="text-left px-3 py-2 text-[#5A5A5A] uppercase tracking-wider font-medium">Cost Price</th>
                          <th className="text-left px-3 py-2 text-[#5A5A5A] uppercase tracking-wider font-medium">Description</th>
                          <th className="text-left px-3 py-2 text-[#5A5A5A] uppercase tracking-wider font-medium">Status</th>
                        </tr>
                      </thead>
                      <tbody className="divide-y divide-[#333333]/50">
                        {displayRows.map((row) => (
                          <tr
                            key={row.row_num}
                            className={row.valid ? "" : "bg-red-500/10"}
                          >
                            <td className="px-3 py-2 text-[#5A5A5A]">{row.row_num}</td>
                            <td className="px-3 py-2 text-neutral-200">{row.name || <span className="text-red-400 italic">empty</span>}</td>
                            <td className="px-3 py-2 text-neutral-400">{row.category}</td>
                            <td className="px-3 py-2 text-neutral-300 font-mono">{row.selling_price}</td>
                            <td className="px-3 py-2 text-neutral-400 font-mono">{row.cost_price}</td>
                            <td className="px-3 py-2 text-[#5A5A5A] max-w-xs truncate">{row.description || ""}</td>
                            <td className="px-3 py-2">
                              {row.valid ? (
                                <span className="text-emerald-400 font-semibold">OK</span>
                              ) : (
                                <span className="text-red-400 font-semibold" title={row.errors.join("; ")}>
                                  {row.errors.join(", ")}
                                </span>
                              )}
                            </td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                  {importPreview.total_rows > 100 && (
                    <p className="text-xs text-[#5A5A5A] mt-2">
                      Showing first 100 of {importPreview.total_rows} rows
                    </p>
                  )}
                </div>
              )}
            </div>

            {/* Modal Footer */}
            {importPreview !== null && (
              <div className="flex items-center justify-between px-6 py-4 border-t border-[#333333]">
                <button
                  onClick={() => setImportPreview(null)}
                  disabled={importConfirming}
                  className="bg-[#1A1A1A] border border-[#333333] text-neutral-400 text-sm font-semibold px-4 py-2 rounded-lg hover:text-white hover:border-neutral-500 disabled:opacity-50 transition-colors"
                >
                  Back
                </button>
                <button
                  onClick={handleImportConfirm}
                  disabled={importPreview.valid_rows === 0 || importConfirming}
                  className="bg-[#E10600] text-white text-sm font-semibold px-5 py-2 rounded-lg hover:bg-[#c40500] disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
                >
                  {importConfirming
                    ? "Importing..."
                    : `Import ${importPreview.valid_rows} Item${importPreview.valid_rows !== 1 ? "s" : ""}`}
                </button>
              </div>
            )}
          </div>
        </div>
      )}
    </DashboardLayout>
  );
}

// ─── PromoPanel ───────────────────────────────────────────────────────────────

interface PromoPanelProps {
  promo: CafePromo | null;
  promoType: PromoType;
  items: CafeItem[];
  categories: CafeCategory[];
  onTypeChange: (t: PromoType) => void;
  onSave: (data: CreateCafePromoRequest) => Promise<void>;
  onClose: () => void;
  saving: boolean;
}

function PromoPanel({
  promo,
  promoType,
  items,
  categories,
  onTypeChange,
  onSave,
  onClose,
  saving,
}: PromoPanelProps) {
  const [name, setName] = useState(promo?.name ?? "");
  const [stackingGroup, setStackingGroup] = useState(promo?.stacking_group ?? "");
  const [startTime, setStartTime] = useState(promo?.start_time ?? "");
  const [endTime, setEndTime] = useState(promo?.end_time ?? "");

  // Combo state
  const [comboItems, setComboItems] = useState<Array<{ id: string; qty: number }>>(() => {
    if (promo?.promo_type === "combo") {
      const cfg = JSON.parse(promo.config) as ComboConfig;
      return cfg.items;
    }
    return [];
  });
  const [comboBundlePriceRupees, setComboBundlePriceRupees] = useState<string>(() => {
    if (promo?.promo_type === "combo") {
      const cfg = JSON.parse(promo.config) as ComboConfig;
      return (cfg.bundle_price_paise / 100).toFixed(2);
    }
    return "";
  });

  // Happy hour state
  const [discountMode, setDiscountMode] = useState<"percent" | "paise">("percent");
  const [discountPercent, setDiscountPercent] = useState<string>(() => {
    if (promo?.promo_type === "happy_hour") {
      const cfg = JSON.parse(promo.config) as HappyHourConfig;
      return cfg.discount_percent !== undefined ? String(cfg.discount_percent) : "";
    }
    return "";
  });
  const [discountPaise, setDiscountPaise] = useState<string>(() => {
    if (promo?.promo_type === "happy_hour") {
      const cfg = JSON.parse(promo.config) as HappyHourConfig;
      return cfg.discount_paise !== undefined ? (cfg.discount_paise / 100).toFixed(2) : "";
    }
    return "";
  });
  const [appliesTo, setAppliesTo] = useState<"category" | "item" | "all">(() => {
    if (promo?.promo_type === "happy_hour") {
      const cfg = JSON.parse(promo.config) as HappyHourConfig;
      return cfg.applies_to;
    }
    return "all";
  });
  const [targetIds, setTargetIds] = useState<string[]>(() => {
    if (promo?.promo_type === "happy_hour") {
      const cfg = JSON.parse(promo.config) as HappyHourConfig;
      return cfg.target_ids;
    }
    return [];
  });

  // Gaming bundle state
  const [sessionDuration, setSessionDuration] = useState<string>(() => {
    if (promo?.promo_type === "gaming_bundle") {
      const cfg = JSON.parse(promo.config) as GamingBundleConfig;
      return String(cfg.session_duration_mins);
    }
    return "";
  });
  const [bundleCafeItemIds, setBundleCafeItemIds] = useState<string[]>(() => {
    if (promo?.promo_type === "gaming_bundle") {
      const cfg = JSON.parse(promo.config) as GamingBundleConfig;
      return cfg.cafe_item_ids;
    }
    return [];
  });
  const [gamingBundlePriceRupees, setGamingBundlePriceRupees] = useState<string>(() => {
    if (promo?.promo_type === "gaming_bundle") {
      const cfg = JSON.parse(promo.config) as GamingBundleConfig;
      return (cfg.bundle_price_paise / 100).toFixed(2);
    }
    return "";
  });

  const inputClass =
    "w-full bg-[#1A1A1A] border border-[#333333] rounded-lg px-3 py-2 text-sm text-neutral-200 placeholder-[#5A5A5A] focus:outline-none focus:border-[#E10600] transition-colors";

  function toggleComboItem(itemId: string) {
    setComboItems((prev) => {
      const exists = prev.find((i) => i.id === itemId);
      if (exists) return prev.filter((i) => i.id !== itemId);
      return [...prev, { id: itemId, qty: 1 }];
    });
  }

  function setComboItemQty(itemId: string, qty: number) {
    setComboItems((prev) =>
      prev.map((i) => (i.id === itemId ? { ...i, qty } : i))
    );
  }

  function toggleTargetId(id: string) {
    setTargetIds((prev) =>
      prev.includes(id) ? prev.filter((t) => t !== id) : [...prev, id]
    );
  }

  function toggleBundleCafeItem(itemId: string) {
    setBundleCafeItemIds((prev) =>
      prev.includes(itemId) ? prev.filter((i) => i !== itemId) : [...prev, itemId]
    );
  }

  function buildConfig(): CreateCafePromoRequest["config"] {
    if (promoType === "combo") {
      const cfg: ComboConfig = {
        items: comboItems,
        bundle_price_paise: Math.round(parseFloat(comboBundlePriceRupees || "0") * 100),
      };
      return cfg;
    }
    if (promoType === "happy_hour") {
      const cfg: HappyHourConfig = {
        applies_to: appliesTo,
        target_ids: targetIds,
        ...(discountMode === "percent"
          ? { discount_percent: parseFloat(discountPercent || "0") }
          : { discount_paise: Math.round(parseFloat(discountPaise || "0") * 100) }),
      };
      return cfg;
    }
    // gaming_bundle
    const cfg: GamingBundleConfig = {
      session_duration_mins: parseInt(sessionDuration || "0", 10),
      cafe_item_ids: bundleCafeItemIds,
      bundle_price_paise: Math.round(parseFloat(gamingBundlePriceRupees || "0") * 100),
    };
    return cfg;
  }

  async function handleSubmit() {
    if (!name.trim()) return;
    await onSave({
      name: name.trim(),
      promo_type: promoType,
      config: buildConfig(),
      is_active: promo?.is_active ?? false,
      start_time: startTime || null,
      end_time: endTime || null,
      stacking_group: stackingGroup.trim() || null,
    });
  }

  const targetOptions = appliesTo === "category" ? categories : items;

  return (
    <div className="fixed inset-0 bg-black/40 z-50 flex justify-end">
      <div className="w-[480px] bg-[#1A1A1A] border-l border-[#333333] h-full overflow-y-auto p-6 flex flex-col gap-5">
        <div className="flex items-center justify-between">
          <h2 className="text-lg font-bold text-white">
            {promo ? "Edit Promo" : "New Promo"}
          </h2>
          <button onClick={onClose} className="text-[#5A5A5A] hover:text-white text-xl leading-none">
            &times;
          </button>
        </div>

        {/* Name */}
        <div>
          <label className="block text-xs text-[#5A5A5A] mb-1">
            Name <span className="text-[#E10600]">*</span>
          </label>
          <input
            type="text"
            placeholder="Promo name"
            value={name}
            onChange={(e) => setName(e.target.value)}
            className={inputClass}
          />
        </div>

        {/* Promo Type */}
        <div>
          <label className="block text-xs text-[#5A5A5A] mb-1">Type</label>
          <select
            value={promoType}
            onChange={(e) => onTypeChange(e.target.value as PromoType)}
            disabled={!!promo}
            className={inputClass + (promo ? " opacity-50 cursor-not-allowed" : "")}
          >
            <option value="combo">Combo Deal</option>
            <option value="happy_hour">Happy Hour</option>
            <option value="gaming_bundle">Gaming + Cafe Bundle</option>
          </select>
        </div>

        {/* Stacking Group */}
        <div>
          <label className="block text-xs text-[#5A5A5A] mb-1">
            Exclusivity Group
            <span className="ml-1 text-[#5A5A5A]/70 font-normal">
              (promos sharing the same group name are mutually exclusive)
            </span>
          </label>
          <input
            type="text"
            placeholder="Optional group name"
            value={stackingGroup}
            onChange={(e) => setStackingGroup(e.target.value)}
            className={inputClass}
          />
        </div>

        {/* ── Combo fields ── */}
        {promoType === "combo" && (
          <>
            <div>
              <label className="block text-xs text-[#5A5A5A] mb-2">Select Items</label>
              <div className="space-y-1 max-h-48 overflow-y-auto border border-[#333333] rounded-lg p-2">
                {items.map((item) => {
                  const selected = comboItems.find((ci) => ci.id === item.id);
                  return (
                    <div key={item.id} className="flex items-center gap-3">
                      <input
                        type="checkbox"
                        id={`combo-item-${item.id}`}
                        checked={!!selected}
                        onChange={() => toggleComboItem(item.id)}
                        className="w-4 h-4 accent-[#E10600]"
                      />
                      <label
                        htmlFor={`combo-item-${item.id}`}
                        className="flex-1 text-sm text-neutral-300 cursor-pointer"
                      >
                        {item.name}
                      </label>
                      {selected && (
                        <input
                          type="number"
                          min={1}
                          value={selected.qty}
                          onChange={(e) =>
                            setComboItemQty(item.id, parseInt(e.target.value || "1", 10))
                          }
                          className="w-16 bg-[#1A1A1A] border border-[#333333] rounded px-2 py-1 text-xs text-neutral-200 focus:outline-none focus:border-[#E10600]"
                        />
                      )}
                    </div>
                  );
                })}
              </div>
            </div>
            <div>
              <label className="block text-xs text-[#5A5A5A] mb-1">Bundle Price (rupees)</label>
              <input
                type="number"
                min={0}
                step={0.5}
                placeholder="0.00"
                value={comboBundlePriceRupees}
                onChange={(e) => setComboBundlePriceRupees(e.target.value)}
                className={inputClass}
              />
            </div>
          </>
        )}

        {/* ── Happy Hour fields ── */}
        {promoType === "happy_hour" && (
          <>
            <div>
              <label className="block text-xs text-[#5A5A5A] mb-2">Discount Type</label>
              <div className="flex gap-4">
                <label className="flex items-center gap-2 cursor-pointer text-sm text-neutral-300">
                  <input
                    type="radio"
                    checked={discountMode === "percent"}
                    onChange={() => setDiscountMode("percent")}
                    className="accent-[#E10600]"
                  />
                  Percent (%)
                </label>
                <label className="flex items-center gap-2 cursor-pointer text-sm text-neutral-300">
                  <input
                    type="radio"
                    checked={discountMode === "paise"}
                    onChange={() => setDiscountMode("paise")}
                    className="accent-[#E10600]"
                  />
                  Flat Amount (rupees)
                </label>
              </div>
            </div>
            {discountMode === "percent" ? (
              <div>
                <label className="block text-xs text-[#5A5A5A] mb-1">Discount Percent</label>
                <input
                  type="number"
                  min={0}
                  max={100}
                  step={1}
                  placeholder="e.g. 20"
                  value={discountPercent}
                  onChange={(e) => setDiscountPercent(e.target.value)}
                  className={inputClass}
                />
              </div>
            ) : (
              <div>
                <label className="block text-xs text-[#5A5A5A] mb-1">Flat Discount (rupees)</label>
                <input
                  type="number"
                  min={0}
                  step={0.5}
                  placeholder="0.00"
                  value={discountPaise}
                  onChange={(e) => setDiscountPaise(e.target.value)}
                  className={inputClass}
                />
              </div>
            )}
            <div>
              <label className="block text-xs text-[#5A5A5A] mb-1">Applies To</label>
              <select
                value={appliesTo}
                onChange={(e) => {
                  setAppliesTo(e.target.value as "category" | "item" | "all");
                  setTargetIds([]);
                }}
                className={inputClass}
              >
                <option value="all">All Items</option>
                <option value="category">Specific Category</option>
                <option value="item">Specific Items</option>
              </select>
            </div>
            {appliesTo !== "all" && (
              <div>
                <label className="block text-xs text-[#5A5A5A] mb-2">
                  Select {appliesTo === "category" ? "Categories" : "Items"}
                </label>
                <div className="space-y-1 max-h-40 overflow-y-auto border border-[#333333] rounded-lg p-2">
                  {targetOptions.map((opt) => (
                    <label
                      key={opt.id}
                      className="flex items-center gap-2 cursor-pointer text-sm text-neutral-300"
                    >
                      <input
                        type="checkbox"
                        checked={targetIds.includes(opt.id)}
                        onChange={() => toggleTargetId(opt.id)}
                        className="w-4 h-4 accent-[#E10600]"
                      />
                      {opt.name}
                    </label>
                  ))}
                </div>
              </div>
            )}
            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="block text-xs text-[#5A5A5A] mb-1">Start Time (IST)</label>
                <input
                  type="time"
                  value={startTime}
                  onChange={(e) => setStartTime(e.target.value)}
                  className={inputClass}
                />
              </div>
              <div>
                <label className="block text-xs text-[#5A5A5A] mb-1">End Time (IST)</label>
                <input
                  type="time"
                  value={endTime}
                  onChange={(e) => setEndTime(e.target.value)}
                  className={inputClass}
                />
              </div>
            </div>
          </>
        )}

        {/* ── Gaming Bundle fields ── */}
        {promoType === "gaming_bundle" && (
          <>
            <div>
              <label className="block text-xs text-[#5A5A5A] mb-1">
                Session Duration (minutes)
              </label>
              <input
                type="number"
                min={1}
                step={1}
                placeholder="e.g. 60"
                value={sessionDuration}
                onChange={(e) => setSessionDuration(e.target.value)}
                className={inputClass}
              />
            </div>
            <div>
              <label className="block text-xs text-[#5A5A5A] mb-2">Include Cafe Items</label>
              <div className="space-y-1 max-h-48 overflow-y-auto border border-[#333333] rounded-lg p-2">
                {items.map((item) => (
                  <label
                    key={item.id}
                    className="flex items-center gap-2 cursor-pointer text-sm text-neutral-300"
                  >
                    <input
                      type="checkbox"
                      checked={bundleCafeItemIds.includes(item.id)}
                      onChange={() => toggleBundleCafeItem(item.id)}
                      className="w-4 h-4 accent-[#E10600]"
                    />
                    {item.name}
                  </label>
                ))}
              </div>
            </div>
            <div>
              <label className="block text-xs text-[#5A5A5A] mb-1">Bundle Price (rupees)</label>
              <input
                type="number"
                min={0}
                step={0.5}
                placeholder="0.00"
                value={gamingBundlePriceRupees}
                onChange={(e) => setGamingBundlePriceRupees(e.target.value)}
                className={inputClass}
              />
            </div>
          </>
        )}

        {/* Save / Cancel */}
        <div className="flex gap-3 pt-2 mt-auto">
          <button
            onClick={handleSubmit}
            disabled={!name.trim() || saving}
            className="flex-1 bg-[#E10600] text-white text-sm font-semibold py-2 rounded-lg hover:bg-[#c40500] disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            {saving ? "Saving..." : "Save"}
          </button>
          <button
            onClick={onClose}
            className="flex-1 bg-[#222222] border border-[#333333] text-neutral-400 text-sm font-semibold py-2 rounded-lg hover:text-white hover:border-neutral-500 transition-colors"
          >
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
}
