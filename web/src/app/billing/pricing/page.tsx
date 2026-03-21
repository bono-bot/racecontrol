"use client";

import { useEffect, useState } from "react";
import DashboardLayout from "@/components/DashboardLayout";
import { api } from "@/lib/api";
import type { PricingTier, BillingRate } from "@/lib/api";

const formatCredits = (paise: number) => `${Math.floor(paise / 100)} cr`;

const SIM_TYPE_LABELS: Record<string, string> = {
  assetto_corsa: "AC",
  assetto_corsa_evo: "AC EVO",
  assetto_corsa_rally: "AC Rally",
  iracing: "iRacing",
  le_mans_ultimate: "LMU",
  f1_25: "F1 25",
  forza: "Forza",
  forza_horizon_5: "Forza H5",
  ea_wrc: "EA WRC",
};

const SIM_TYPE_OPTIONS = [
  { value: "", label: "All games" },
  ...Object.entries(SIM_TYPE_LABELS).map(([value, label]) => ({ value, label })),
];

export default function PricingPage() {
  const [tiers, setTiers] = useState<PricingTier[]>([]);
  const [loading, setLoading] = useState(true);

  // New tier form
  const [newName, setNewName] = useState("");
  const [newDuration, setNewDuration] = useState(30);
  const [newPrice, setNewPrice] = useState(200);
  const [newIsTrial, setNewIsTrial] = useState(false);
  const [creating, setCreating] = useState(false);

  // Editing
  const [editId, setEditId] = useState<string | null>(null);
  const [editName, setEditName] = useState("");
  const [editDuration, setEditDuration] = useState(0);
  const [editPrice, setEditPrice] = useState(0);
  const [editIsTrial, setEditIsTrial] = useState(false);

  // Billing rates (per-minute)
  const [rates, setRates] = useState<BillingRate[]>([]);
  const [rateEditId, setRateEditId] = useState<string | null>(null);
  const [rateEditName, setRateEditName] = useState("");
  const [rateEditThreshold, setRateEditThreshold] = useState(0);
  const [rateEditRate, setRateEditRate] = useState(0);
  const [editSimType, setEditSimType] = useState<string>("");

  // New rate form
  const [newRateName, setNewRateName] = useState("");
  const [newRateThreshold, setNewRateThreshold] = useState(0);
  const [newRateRate, setNewRateRate] = useState(5);
  const [newSimType, setNewSimType] = useState<string>("");
  const [creatingRate, setCreatingRate] = useState(false);

  function fetchRates() {
    api.listBillingRates().then((res) => setRates(res.rates || [])).catch(() => {});
  }

  function fetchTiers() {
    api
      .listPricingTiers()
      .then((res) => {
        setTiers(res.tiers || []);
        setLoading(false);
      })
      .catch(() => setLoading(false));
  }

  useEffect(() => {
    fetchTiers();
    fetchRates();
  }, []);

  async function handleCreate() {
    if (!newName.trim()) return;
    setCreating(true);
    try {
      await api.createPricingTier({
        name: newName.trim(),
        duration_minutes: newDuration,
        price_paise: newIsTrial ? 0 : newPrice * 100,
        is_trial: newIsTrial,
        is_active: true,
      });
      setNewName("");
      setNewDuration(30);
      setNewPrice(200);
      setNewIsTrial(false);
      fetchTiers();
    } finally {
      setCreating(false);
    }
  }

  async function handleToggleActive(tier: PricingTier) {
    await api.updatePricingTier(tier.id, { is_active: !tier.is_active });
    fetchTiers();
  }

  async function handleDelete(id: string) {
    await api.deletePricingTier(id);
    fetchTiers();
  }

  function startEdit(tier: PricingTier) {
    setEditId(tier.id);
    setEditName(tier.name);
    setEditDuration(tier.duration_minutes);
    setEditPrice(tier.price_paise / 100);
    setEditIsTrial(tier.is_trial);
  }

  async function handleSaveEdit() {
    if (!editId || !editName.trim()) return;
    await api.updatePricingTier(editId, {
      name: editName.trim(),
      duration_minutes: editDuration,
      price_paise: editIsTrial ? 0 : editPrice * 100,
      is_trial: editIsTrial,
    });
    setEditId(null);
    fetchTiers();
  }

  function cancelEdit() {
    setEditId(null);
  }

  return (
    <DashboardLayout>
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-2xl font-bold text-white">Pricing Tiers</h1>
          <p className="text-sm text-rp-grey">Configure session pricing</p>
        </div>
        <span className="text-xs text-rp-grey">{tiers.length} tiers</span>
      </div>

      {/* Add New Tier Form */}
      <div className="bg-rp-card border border-rp-border rounded-lg p-4 mb-6">
        <h3 className="text-sm font-medium text-neutral-300 mb-3">
          Add New Tier
        </h3>
        <div className="grid grid-cols-1 sm:grid-cols-5 gap-3 items-end">
          <div>
            <label className="block text-xs text-rp-grey mb-1">Name</label>
            <input
              type="text"
              placeholder="e.g. Standard"
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              className="w-full bg-rp-card border border-rp-border rounded-lg px-3 py-2 text-sm text-neutral-200 placeholder-rp-grey focus:outline-none focus:border-rp-red transition-colors"
            />
          </div>
          <div>
            <label className="block text-xs text-rp-grey mb-1">
              Duration (min)
            </label>
            <input
              type="number"
              min={1}
              value={newDuration}
              onChange={(e) => setNewDuration(parseInt(e.target.value) || 30)}
              className="w-full bg-rp-card border border-rp-border rounded-lg px-3 py-2 text-sm text-neutral-200 focus:outline-none focus:border-rp-red transition-colors"
            />
          </div>
          <div>
            <label className="block text-xs text-rp-grey mb-1">
              Price (credits)
            </label>
            <input
              type="number"
              min={0}
              step={50}
              value={newPrice}
              onChange={(e) => setNewPrice(parseInt(e.target.value) || 0)}
              disabled={newIsTrial}
              className="w-full bg-rp-card border border-rp-border rounded-lg px-3 py-2 text-sm text-neutral-200 focus:outline-none focus:border-rp-red transition-colors disabled:opacity-50"
            />
          </div>
          <div className="flex items-center gap-2 pt-1">
            <button
              onClick={() => setNewIsTrial(!newIsTrial)}
              className={`relative inline-flex h-5 w-9 items-center rounded-full transition-colors ${
                newIsTrial ? "bg-emerald-500" : "bg-rp-card"
              }`}
            >
              <span
                className={`inline-block h-3.5 w-3.5 rounded-full bg-white transition-transform ${
                  newIsTrial ? "translate-x-4" : "translate-x-1"
                }`}
              />
            </button>
            <span className="text-xs text-neutral-400">Trial</span>
          </div>
          <button
            onClick={handleCreate}
            disabled={!newName.trim() || creating}
            className={`rounded-lg py-2 text-sm font-semibold transition-all ${
              newName.trim() && !creating
                ? "bg-rp-red text-white hover:bg-rp-red"
                : "bg-rp-card text-rp-grey cursor-not-allowed"
            }`}
          >
            {creating ? "Adding..." : "Add Tier"}
          </button>
        </div>
      </div>

      {/* Tiers Table */}
      {loading ? (
        <div className="text-center py-12 text-rp-grey text-sm">
          Loading tiers...
        </div>
      ) : tiers.length === 0 ? (
        <div className="bg-rp-card border border-rp-border rounded-lg p-8 text-center">
          <p className="text-neutral-400 mb-2">No pricing tiers</p>
          <p className="text-rp-grey text-sm">
            Add your first pricing tier above.
          </p>
        </div>
      ) : (
        <div className="bg-rp-card border border-rp-border rounded-lg overflow-hidden">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-rp-border">
                <th className="text-left px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider">
                  Name
                </th>
                <th className="text-left px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider">
                  Duration
                </th>
                <th className="text-left px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider">
                  Price
                </th>
                <th className="text-center px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider">
                  Trial
                </th>
                <th className="text-center px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider">
                  Active
                </th>
                <th className="text-right px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider">
                  Actions
                </th>
              </tr>
            </thead>
            <tbody className="divide-y divide-rp-border/50">
              {tiers.map((tier) => (
                <tr key={tier.id} className="hover:bg-rp-card/30 transition-colors">
                  {editId === tier.id ? (
                    <>
                      <td className="px-4 py-3">
                        <input
                          type="text"
                          value={editName}
                          onChange={(e) => setEditName(e.target.value)}
                          className="bg-rp-card border border-rp-border rounded px-2 py-1 text-sm text-neutral-200 focus:outline-none focus:border-rp-red w-full"
                        />
                      </td>
                      <td className="px-4 py-3">
                        <input
                          type="number"
                          min={1}
                          value={editDuration}
                          onChange={(e) =>
                            setEditDuration(parseInt(e.target.value) || 0)
                          }
                          className="bg-rp-card border border-rp-border rounded px-2 py-1 text-sm text-neutral-200 focus:outline-none focus:border-rp-red w-20"
                        />
                      </td>
                      <td className="px-4 py-3">
                        <input
                          type="number"
                          min={0}
                          step={50}
                          value={editPrice}
                          onChange={(e) =>
                            setEditPrice(parseInt(e.target.value) || 0)
                          }
                          disabled={editIsTrial}
                          className="bg-rp-card border border-rp-border rounded px-2 py-1 text-sm text-neutral-200 focus:outline-none focus:border-rp-red w-24 disabled:opacity-50"
                        />
                      </td>
                      <td className="px-4 py-3 text-center">
                        <button
                          onClick={() => setEditIsTrial(!editIsTrial)}
                          className={`relative inline-flex h-5 w-9 items-center rounded-full transition-colors ${
                            editIsTrial ? "bg-emerald-500" : "bg-rp-card"
                          }`}
                        >
                          <span
                            className={`inline-block h-3.5 w-3.5 rounded-full bg-white transition-transform ${
                              editIsTrial ? "translate-x-4" : "translate-x-1"
                            }`}
                          />
                        </button>
                      </td>
                      <td className="px-4 py-3 text-center">
                        <span className="text-xs text-rp-grey">&mdash;</span>
                      </td>
                      <td className="px-4 py-3 text-right">
                        <div className="flex items-center justify-end gap-2">
                          <button
                            onClick={handleSaveEdit}
                            className="text-xs text-emerald-400 hover:text-emerald-300 font-medium"
                          >
                            Save
                          </button>
                          <button
                            onClick={cancelEdit}
                            className="text-xs text-rp-grey hover:text-neutral-300"
                          >
                            Cancel
                          </button>
                        </div>
                      </td>
                    </>
                  ) : (
                    <>
                      <td className="px-4 py-3 text-neutral-200 font-medium">
                        {tier.name}
                      </td>
                      <td className="px-4 py-3 text-neutral-400">
                        {tier.duration_minutes} min
                      </td>
                      <td className="px-4 py-3 text-neutral-300 font-mono">
                        {tier.is_trial ? (
                          <span className="text-emerald-400">Free</span>
                        ) : (
                          formatCredits(tier.price_paise)
                        )}
                      </td>
                      <td className="px-4 py-3 text-center">
                        {tier.is_trial && (
                          <span className="bg-emerald-500/20 text-emerald-400 text-[10px] font-bold px-1.5 py-0.5 rounded">
                            FREE
                          </span>
                        )}
                      </td>
                      <td className="px-4 py-3 text-center">
                        <button
                          onClick={() => handleToggleActive(tier)}
                          className={`relative inline-flex h-5 w-9 items-center rounded-full transition-colors ${
                            tier.is_active ? "bg-rp-red" : "bg-rp-card"
                          }`}
                        >
                          <span
                            className={`inline-block h-3.5 w-3.5 rounded-full bg-white transition-transform ${
                              tier.is_active
                                ? "translate-x-4"
                                : "translate-x-1"
                            }`}
                          />
                        </button>
                      </td>
                      <td className="px-4 py-3 text-right">
                        <div className="flex items-center justify-end gap-2">
                          <button
                            onClick={() => startEdit(tier)}
                            className="text-xs text-neutral-400 hover:text-rp-red font-medium transition-colors"
                          >
                            Edit
                          </button>
                          <button
                            onClick={() => handleDelete(tier.id)}
                            className="text-xs text-rp-grey hover:text-red-400 font-medium transition-colors"
                          >
                            Delete
                          </button>
                        </div>
                      </td>
                    </>
                  )}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {/* Per-Minute Rates */}
      <div className="mt-8">
        <div className="flex items-center justify-between mb-4">
          <div>
            <h2 className="text-xl font-bold text-white">Per-Minute Rates</h2>
            <p className="text-sm text-rp-grey">Non-retroactive tiered pricing (credits/min)</p>
          </div>
          <span className="text-xs text-rp-grey">{rates.filter(r => r.is_active).length} active tiers</span>
        </div>

        {/* Add Rate Form */}
        <div className="bg-rp-card border border-rp-border rounded-lg p-4 mb-4">
          <h3 className="text-sm font-medium text-neutral-300 mb-3">Add Rate</h3>
          <div className="grid grid-cols-1 sm:grid-cols-6 gap-3 items-end">
            <div>
              <label className="block text-xs text-rp-grey mb-1">Tier Name</label>
              <input
                type="text"
                placeholder="e.g. Standard"
                value={newRateName}
                onChange={(e) => setNewRateName(e.target.value)}
                className="w-full bg-rp-card border border-rp-border rounded-lg px-3 py-2 text-sm text-neutral-200 placeholder-rp-grey focus:outline-none focus:border-rp-red transition-colors"
              />
            </div>
            <div>
              <label className="block text-xs text-rp-grey mb-1">Up To (min)</label>
              <input
                type="number"
                min={0}
                value={newRateThreshold}
                onChange={(e) => setNewRateThreshold(parseInt(e.target.value) || 0)}
                className="w-full bg-rp-card border border-rp-border rounded-lg px-3 py-2 text-sm text-neutral-200 focus:outline-none focus:border-rp-red transition-colors"
              />
            </div>
            <div>
              <label className="block text-xs text-rp-grey mb-1">Game</label>
              <select
                value={newSimType}
                onChange={(e) => setNewSimType(e.target.value)}
                className="w-full bg-rp-card border border-rp-border rounded-lg px-3 py-2 text-sm text-neutral-200 focus:outline-none focus:border-rp-red transition-colors"
              >
                {SIM_TYPE_OPTIONS.map((opt) => (
                  <option key={opt.value} value={opt.value}>{opt.label}</option>
                ))}
              </select>
            </div>
            <div>
              <label className="block text-xs text-rp-grey mb-1">Rate (cr/min)</label>
              <input
                type="number"
                min={1}
                value={newRateRate}
                onChange={(e) => setNewRateRate(parseInt(e.target.value) || 0)}
                className="w-full bg-rp-card border border-rp-border rounded-lg px-3 py-2 text-sm text-neutral-200 focus:outline-none focus:border-rp-red transition-colors"
              />
            </div>
            <div />
            <button
              onClick={async () => {
                if (!newRateName.trim()) return;
                setCreatingRate(true);
                try {
                  const simTypePayload = newSimType === "" ? null : newSimType;
                  await api.createBillingRate({
                    tier_name: newRateName.trim(),
                    threshold_minutes: newRateThreshold,
                    rate_per_min_paise: newRateRate * 100,
                    sim_type: simTypePayload,
                    is_active: true,
                  });
                  setNewRateName("");
                  setNewRateThreshold(0);
                  setNewRateRate(5);
                  setNewSimType("");
                  fetchRates();
                } finally {
                  setCreatingRate(false);
                }
              }}
              disabled={!newRateName.trim() || creatingRate}
              className={`rounded-lg py-2 text-sm font-semibold transition-all ${
                newRateName.trim() && !creatingRate
                  ? "bg-rp-red text-white hover:bg-rp-red"
                  : "bg-rp-card text-rp-grey cursor-not-allowed"
              }`}
            >
              {creatingRate ? "Adding..." : "Add Rate"}
            </button>
          </div>
        </div>

        <div className="bg-rp-card border border-rp-border rounded-lg overflow-hidden">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-rp-border">
                <th className="text-left px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider">Tier Name</th>
                <th className="text-left px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider">Up To (min)</th>
                <th className="text-left px-3 py-2 text-xs font-bold uppercase tracking-wider text-rp-grey">Game</th>
                <th className="text-left px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider">Rate (cr/min)</th>
                <th className="text-center px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider">Active</th>
                <th className="text-right px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider">Actions</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-rp-border/50">
              {rates.map((rate) => (
                <tr key={rate.id} className="hover:bg-rp-card/30 transition-colors">
                  {rateEditId === rate.id ? (
                    <>
                      <td className="px-4 py-3">
                        <input type="text" value={rateEditName} onChange={(e) => setRateEditName(e.target.value)}
                          className="bg-rp-card border border-rp-border rounded px-2 py-1 text-sm text-neutral-200 focus:outline-none focus:border-rp-red w-full" />
                      </td>
                      <td className="px-4 py-3">
                        <input type="number" min={0} value={rateEditThreshold} onChange={(e) => setRateEditThreshold(parseInt(e.target.value) || 0)}
                          className="bg-rp-card border border-rp-border rounded px-2 py-1 text-sm text-neutral-200 focus:outline-none focus:border-rp-red w-20" />
                      </td>
                      <td className="px-3 py-2">
                        <select
                          value={editSimType}
                          onChange={(e) => setEditSimType(e.target.value)}
                          className="bg-rp-card border border-rp-border rounded px-2 py-1 text-sm text-neutral-200 focus:outline-none focus:border-rp-red w-32"
                        >
                          {SIM_TYPE_OPTIONS.map((opt) => (
                            <option key={opt.value} value={opt.value}>{opt.label}</option>
                          ))}
                        </select>
                      </td>
                      <td className="px-4 py-3">
                        <input type="number" min={1} value={rateEditRate} onChange={(e) => setRateEditRate(parseInt(e.target.value) || 0)}
                          className="bg-rp-card border border-rp-border rounded px-2 py-1 text-sm text-neutral-200 focus:outline-none focus:border-rp-red w-20" />
                      </td>
                      <td className="px-4 py-3 text-center">
                        <span className="text-xs text-rp-grey">&mdash;</span>
                      </td>
                      <td className="px-4 py-3 text-right">
                        <div className="flex items-center justify-end gap-2">
                          <button onClick={async () => {
                            const simTypePayload = editSimType === "" ? null : editSimType;
                            await api.updateBillingRate(rate.id, {
                              tier_name: rateEditName, threshold_minutes: rateEditThreshold,
                              rate_per_min_paise: rateEditRate * 100,
                              sim_type: simTypePayload,
                            });
                            setRateEditId(null); fetchRates();
                          }} className="text-xs text-rp-red hover:text-rp-red font-medium">Save Rate</button>
                          <button onClick={() => setRateEditId(null)} className="text-xs text-rp-grey hover:text-neutral-300">Discard</button>
                        </div>
                      </td>
                    </>
                  ) : (
                    <>
                      <td className="px-4 py-3 text-neutral-200 font-medium">{rate.tier_name}</td>
                      <td className="px-4 py-3 text-neutral-400 font-mono">
                        {rate.threshold_minutes === 0 ? "Unlimited" : `${rate.threshold_minutes} min`}
                      </td>
                      <td className="px-3 py-2 text-sm">
                        {rate.sim_type
                          ? <span className={SIM_TYPE_LABELS[rate.sim_type] ? "text-neutral-200" : "text-amber-400"}>
                              {SIM_TYPE_LABELS[rate.sim_type] || rate.sim_type}
                            </span>
                          : <span className="text-neutral-400">All games</span>
                        }
                      </td>
                      <td className="px-4 py-3 text-neutral-300 font-mono font-bold">{rate.rate_per_min_paise / 100} cr/min</td>
                      <td className="px-4 py-3 text-center">
                        <button onClick={async () => {
                          await api.updateBillingRate(rate.id, { is_active: !rate.is_active });
                          fetchRates();
                        }} className={`relative inline-flex h-5 w-9 items-center rounded-full transition-colors ${rate.is_active ? "bg-rp-red" : "bg-rp-card"}`}>
                          <span className={`inline-block h-3.5 w-3.5 rounded-full bg-white transition-transform ${rate.is_active ? "translate-x-4" : "translate-x-1"}`} />
                        </button>
                      </td>
                      <td className="px-4 py-3 text-right">
                        <div className="flex items-center justify-end gap-2">
                          <button onClick={() => {
                            setRateEditId(rate.id); setRateEditName(rate.tier_name);
                            setRateEditThreshold(rate.threshold_minutes);
                            setRateEditRate(rate.rate_per_min_paise / 100);
                            setEditSimType(rate.sim_type || "");
                          }} className="text-xs text-neutral-400 hover:text-rp-red font-medium transition-colors">Edit</button>
                          <button onClick={async () => {
                            await api.deleteBillingRate(rate.id);
                            fetchRates();
                          }} className="text-xs text-rp-grey hover:text-red-400 font-medium transition-colors">Delete</button>
                        </div>
                      </td>
                    </>
                  )}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </DashboardLayout>
  );
}
