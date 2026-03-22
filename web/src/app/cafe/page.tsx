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
} from "@/lib/api";

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

type ActiveTab = "items" | "inventory";

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
