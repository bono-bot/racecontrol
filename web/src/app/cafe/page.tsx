"use client";

import { useEffect, useState } from "react";
import DashboardLayout from "@/components/DashboardLayout";
import { api } from "@/lib/api";
import type { CafeItem, CafeCategory, CreateCafeItemRequest } from "@/lib/api";

const formatRupees = (paise: number) => `\u20b9${(paise / 100).toFixed(2)}`;

interface FormData {
  name: string;
  description: string;
  category_id: string;
  selling_price_rupees: string;
  cost_price_rupees: string;
}

const emptyForm: FormData = {
  name: "",
  description: "",
  category_id: "",
  selling_price_rupees: "",
  cost_price_rupees: "",
};

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

  function getCategoryName(categoryId: string): string {
    return categories.find((c) => c.id === categoryId)?.name || categoryId;
  }

  const inputClass =
    "w-full bg-[#1A1A1A] border border-[#333333] rounded-lg px-3 py-2 text-sm text-neutral-200 placeholder-[#5A5A5A] focus:outline-none focus:border-[#E10600] transition-colors";

  return (
    <DashboardLayout>
      {/* Header */}
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-2xl font-bold text-white">Cafe Menu</h1>
          <p className="text-sm text-[#5A5A5A]">Manage cafe items and categories</p>
        </div>
        <button
          onClick={openAddPanel}
          className="bg-[#E10600] text-white text-sm font-semibold px-4 py-2 rounded-lg hover:bg-[#c40500] transition-colors"
        >
          Add Item
        </button>
      </div>

      {/* Main flex container */}
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
                    <th className="text-right px-4 py-3 text-xs font-medium text-[#5A5A5A] uppercase tracking-wider">
                      Actions
                    </th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-[#333333]/50">
                  {items.map((item) => (
                    <tr key={item.id} className="hover:bg-[#1A1A1A]/50 transition-colors">
                      <td className="px-4 py-3 text-neutral-200 font-medium">{item.name}</td>
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
    </DashboardLayout>
  );
}
