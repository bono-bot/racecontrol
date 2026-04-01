"use client";

import { useCallback, useEffect, useState } from "react";
import DashboardLayout from "@/components/DashboardLayout";
import {
  policyApi,
  type PolicyRule,
  type PolicyEvalLogEntry,
  type CreatePolicyRuleRequest,
} from "@/lib/api";

// ─── Helpers ──────────────────────────────────────────────────────────────────

function formatIST(ts: string | null): string {
  if (!ts) return "Never";
  try {
    return new Date(ts).toLocaleString("en-IN", { timeZone: "Asia/Kolkata" });
  } catch {
    return ts;
  }
}

const CONDITION_LABELS: Record<string, string> = { gt: ">", lt: "<", eq: "=" };
const ACTION_LABELS: Record<string, string> = {
  alert: "Send Alert",
  config_change: "Config Change",
  flag_toggle: "Toggle Flag",
  budget_adjust: "Budget Adjust",
};

// ─── Form state ──────────────────────────────────────────────────────────────

interface RuleFormState {
  name: string;
  metric: string;
  condition: "gt" | "lt" | "eq";
  threshold: string; // string for input, parse to number on submit
  action: "alert" | "config_change" | "flag_toggle" | "budget_adjust";
  action_params: string; // raw JSON text input
  enabled: boolean;
}

const EMPTY_FORM: RuleFormState = {
  name: "",
  metric: "",
  condition: "gt",
  threshold: "",
  action: "alert",
  action_params: "{}",
  enabled: true,
};

function formFromRule(rule: PolicyRule): RuleFormState {
  return {
    name: rule.name,
    metric: rule.metric,
    condition: rule.condition,
    threshold: String(rule.threshold),
    action: rule.action,
    action_params: rule.action_params,
    enabled: rule.enabled,
  };
}

// ─── Main Page ────────────────────────────────────────────────────────────────

export default function PolicyRulesPage() {
  const [rules, setRules] = useState<PolicyRule[]>([]);
  const [evalLog, setEvalLog] = useState<PolicyEvalLogEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showForm, setShowForm] = useState(false);
  const [editingRule, setEditingRule] = useState<PolicyRule | null>(null);
  const [form, setForm] = useState<RuleFormState>(EMPTY_FORM);
  const [saving, setSaving] = useState(false);
  const [deleteConfirmId, setDeleteConfirmId] = useState<string | null>(null);

  const loadData = useCallback(async () => {
    try {
      const [rulesRes, logRes] = await Promise.all([
        policyApi.listRules(),
        policyApi.listEvalLog(),
      ]);
      setRules(rulesRes.rules);
      setEvalLog(logRes.entries.slice(0, 20));
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to load policy data");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadData();
  }, [loadData]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    const thresholdNum = parseFloat(form.threshold);
    if (isNaN(thresholdNum)) {
      setError("Threshold must be a number");
      return;
    }
    let parsedParams: Record<string, unknown> = {};
    try {
      const raw: unknown = JSON.parse(form.action_params);
      if (typeof raw === "object" && raw !== null && !Array.isArray(raw)) {
        parsedParams = raw as Record<string, unknown>;
      }
    } catch {
      setError("Action params must be valid JSON");
      return;
    }
    setSaving(true);
    try {
      if (editingRule) {
        await policyApi.updateRule(editingRule.id, {
          name: form.name,
          metric: form.metric,
          condition: form.condition,
          threshold: thresholdNum,
          action: form.action,
          action_params: parsedParams,
          enabled: form.enabled,
        });
      } else {
        const req: CreatePolicyRuleRequest = {
          name: form.name,
          metric: form.metric,
          condition: form.condition,
          threshold: thresholdNum,
          action: form.action,
          action_params: parsedParams,
          enabled: form.enabled,
        };
        await policyApi.createRule(req);
      }
      setShowForm(false);
      setEditingRule(null);
      setForm(EMPTY_FORM);
      setError(null);
      await loadData();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Save failed");
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async (id: string) => {
    try {
      await policyApi.deleteRule(id);
      setDeleteConfirmId(null);
      await loadData();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Delete failed");
    }
  };

  const openCreate = () => {
    setEditingRule(null);
    setForm(EMPTY_FORM);
    setShowForm(true);
    setError(null);
  };

  const openEdit = (rule: PolicyRule) => {
    setEditingRule(rule);
    setForm(formFromRule(rule));
    setShowForm(true);
    setError(null);
  };

  const cancelForm = () => {
    setShowForm(false);
    setEditingRule(null);
    setForm(EMPTY_FORM);
    setError(null);
  };

  return (
    <DashboardLayout>
      {/* Header */}
      <div className="mb-6 flex items-start justify-between">
        <div>
          <h1 className="text-2xl font-bold text-white">Policy Rules</h1>
          <p className="text-sm text-[#5A5A5A]">
            Automated rules that evaluate metrics and dispatch actions
          </p>
        </div>
        {!showForm && (
          <button
            onClick={openCreate}
            className="px-4 py-2 bg-[#E10600] hover:bg-red-700 text-white text-sm rounded transition-colors"
          >
            + New Rule
          </button>
        )}
      </div>

      {error && (
        <div className="mb-4 px-4 py-3 bg-red-900/30 border border-red-700 rounded text-red-400 text-sm">
          {error}
        </div>
      )}

      {/* Create / Edit Form */}
      {showForm && (
        <div className="mb-6 bg-[#222222] border border-[#333333] rounded-lg p-6">
          <h2 className="text-lg font-semibold text-white mb-4">
            {editingRule ? `Edit Rule: ${editingRule.name}` : "New Policy Rule"}
          </h2>
          <form onSubmit={handleSubmit} className="space-y-4">
            <div className="grid grid-cols-2 gap-4">
              {/* Name */}
              <div>
                <label className="block text-xs text-[#5A5A5A] mb-1">
                  Rule Name *
                </label>
                <input
                  type="text"
                  required
                  value={form.name}
                  onChange={(e) => setForm({ ...form, name: e.target.value })}
                  placeholder="e.g. GPU overheating alert"
                  className="w-full bg-[#1A1A1A] border border-[#333333] text-white rounded px-3 py-2 text-sm focus:outline-none focus:border-[#E10600]"
                />
              </div>
              {/* Metric */}
              <div>
                <label className="block text-xs text-[#5A5A5A] mb-1">
                  Metric *
                </label>
                <input
                  type="text"
                  required
                  value={form.metric}
                  onChange={(e) => setForm({ ...form, metric: e.target.value })}
                  placeholder="e.g. gpu_temp, cpu_usage"
                  className="w-full bg-[#1A1A1A] border border-[#333333] text-white rounded px-3 py-2 text-sm focus:outline-none focus:border-[#E10600]"
                />
              </div>
              {/* Condition */}
              <div>
                <label className="block text-xs text-[#5A5A5A] mb-1">
                  Condition *
                </label>
                <select
                  value={form.condition}
                  onChange={(e) =>
                    setForm({
                      ...form,
                      condition: e.target.value as "gt" | "lt" | "eq",
                    })
                  }
                  className="w-full bg-[#1A1A1A] border border-[#333333] text-white rounded px-3 py-2 text-sm focus:outline-none focus:border-[#E10600]"
                >
                  <option value="gt">&gt; Greater than</option>
                  <option value="lt">&lt; Less than</option>
                  <option value="eq">= Equal to</option>
                </select>
              </div>
              {/* Threshold */}
              <div>
                <label className="block text-xs text-[#5A5A5A] mb-1">
                  Threshold *
                </label>
                <input
                  type="number"
                  step="any"
                  required
                  value={form.threshold}
                  onChange={(e) =>
                    setForm({ ...form, threshold: e.target.value })
                  }
                  placeholder="e.g. 85"
                  className="w-full bg-[#1A1A1A] border border-[#333333] text-white rounded px-3 py-2 text-sm focus:outline-none focus:border-[#E10600]"
                />
              </div>
              {/* Action */}
              <div>
                <label className="block text-xs text-[#5A5A5A] mb-1">
                  Action *
                </label>
                <select
                  value={form.action}
                  onChange={(e) =>
                    setForm({
                      ...form,
                      action: e.target.value as
                        | "alert"
                        | "config_change"
                        | "flag_toggle"
                        | "budget_adjust",
                    })
                  }
                  className="w-full bg-[#1A1A1A] border border-[#333333] text-white rounded px-3 py-2 text-sm focus:outline-none focus:border-[#E10600]"
                >
                  <option value="alert">Send WhatsApp Alert</option>
                  <option value="config_change">Config Change</option>
                  <option value="flag_toggle">Toggle Feature Flag</option>
                  <option value="budget_adjust">Budget Adjust</option>
                </select>
              </div>
              {/* Enabled */}
              <div className="flex items-end pb-2">
                <label className="flex items-center gap-2 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={form.enabled}
                    onChange={(e) =>
                      setForm({ ...form, enabled: e.target.checked })
                    }
                    className="rounded border-[#333333]"
                  />
                  <span className="text-sm text-white">Enabled</span>
                </label>
              </div>
            </div>

            {/* Action Params */}
            <div>
              <label className="block text-xs text-[#5A5A5A] mb-1">
                Action Params (JSON)
              </label>
              <textarea
                value={form.action_params}
                onChange={(e) =>
                  setForm({ ...form, action_params: e.target.value })
                }
                rows={3}
                placeholder={
                  form.action === "alert"
                    ? '{"message": "GPU temp {value}°C exceeds {metric} threshold"}'
                    : form.action === "flag_toggle"
                    ? '{"flag_name": "some_feature", "enabled": true}'
                    : form.action === "budget_adjust"
                    ? '{"daily_budget_usd": 3.0}'
                    : '{"field": "some_field", "value": "new_value", "target_pods": ["pod_1"]}'
                }
                className="w-full bg-[#1A1A1A] border border-[#333333] text-white rounded px-3 py-2 text-sm font-mono focus:outline-none focus:border-[#E10600]"
              />
              <p className="mt-1 text-xs text-[#5A5A5A]">
                JSON object. Use{" "}
                <code className="text-neutral-400">{"{value}"}</code> and{" "}
                <code className="text-neutral-400">{"{metric}"}</code> as
                template variables in alert messages.
              </p>
            </div>

            <div className="flex items-center gap-3 pt-2">
              <button
                type="submit"
                disabled={saving}
                className="px-4 py-2 bg-[#E10600] hover:bg-red-700 text-white text-sm rounded transition-colors disabled:opacity-50"
              >
                {saving ? "Saving..." : editingRule ? "Save Changes" : "Create Rule"}
              </button>
              <button
                type="button"
                onClick={cancelForm}
                className="px-4 py-2 bg-[#333333] hover:bg-neutral-600 text-neutral-300 text-sm rounded transition-colors"
              >
                Cancel
              </button>
            </div>
          </form>
        </div>
      )}

      {/* Rules Table */}
      {loading ? (
        <div className="text-center py-12 text-[#5A5A5A] text-sm">
          Loading policy rules...
        </div>
      ) : rules.length === 0 ? (
        <div className="bg-[#222222] border border-[#333333] rounded-lg p-8 text-center mb-6">
          <p className="text-neutral-400 mb-2">No policy rules defined</p>
          <p className="text-[#5A5A5A] text-sm">
            Create a rule to automatically trigger alerts or actions when
            metrics exceed thresholds.
          </p>
        </div>
      ) : (
        <div className="bg-[#222222] border border-[#333333] rounded-lg overflow-hidden mb-6">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-[#333333]">
                <th className="text-left px-4 py-3 text-[#5A5A5A] text-xs uppercase font-medium">
                  Rule
                </th>
                <th className="text-left px-4 py-3 text-[#5A5A5A] text-xs uppercase font-medium">
                  Condition
                </th>
                <th className="text-left px-4 py-3 text-[#5A5A5A] text-xs uppercase font-medium">
                  Action
                </th>
                <th className="text-left px-4 py-3 text-[#5A5A5A] text-xs uppercase font-medium">
                  Status
                </th>
                <th className="text-left px-4 py-3 text-[#5A5A5A] text-xs uppercase font-medium">
                  Last Fired
                </th>
                <th className="text-left px-4 py-3 text-[#5A5A5A] text-xs uppercase font-medium">
                  Evals
                </th>
                <th className="text-right px-4 py-3 text-[#5A5A5A] text-xs uppercase font-medium">
                  Actions
                </th>
              </tr>
            </thead>
            <tbody>
              {rules.map((rule) => (
                <tr
                  key={rule.id}
                  className="border-b border-[#333333]/50 hover:bg-neutral-800/30"
                >
                  <td className="px-4 py-3">
                    <div className="font-medium text-white">{rule.name}</div>
                  </td>
                  <td className="px-4 py-3">
                    <code className="text-neutral-300 text-xs font-mono">
                      {rule.metric} {CONDITION_LABELS[rule.condition] ?? rule.condition}{" "}
                      {rule.threshold}
                    </code>
                  </td>
                  <td className="px-4 py-3">
                    <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-blue-900/40 text-blue-300">
                      {ACTION_LABELS[rule.action] ?? rule.action}
                    </span>
                  </td>
                  <td className="px-4 py-3">
                    <span
                      className={`inline-flex items-center px-2 py-0.5 rounded text-xs font-medium ${
                        rule.enabled
                          ? "bg-emerald-900/40 text-emerald-400"
                          : "bg-neutral-700 text-neutral-400"
                      }`}
                    >
                      {rule.enabled ? "Active" : "Disabled"}
                    </span>
                  </td>
                  <td className="px-4 py-3">
                    {rule.last_fired === null ? (
                      <span className="text-[#5A5A5A] text-xs">Never</span>
                    ) : (
                      <span className="text-amber-400 text-xs flex items-center gap-1">
                        <span className="w-1.5 h-1.5 rounded-full bg-amber-400 inline-block" />
                        {formatIST(rule.last_fired)}
                      </span>
                    )}
                  </td>
                  <td className="px-4 py-3">
                    <span className="text-neutral-400 text-xs font-mono">
                      {rule.eval_count}
                    </span>
                  </td>
                  <td className="px-4 py-3 text-right">
                    <div className="flex items-center justify-end gap-2">
                      <button
                        onClick={() => openEdit(rule)}
                        className="px-2 py-1 text-xs text-neutral-400 hover:text-white border border-[#333333] hover:border-neutral-500 rounded transition-colors"
                      >
                        Edit
                      </button>
                      {deleteConfirmId === rule.id ? (
                        <>
                          <button
                            onClick={() => handleDelete(rule.id)}
                            className="px-2 py-1 text-xs text-white bg-[#E10600] hover:bg-red-700 rounded transition-colors"
                          >
                            Confirm
                          </button>
                          <button
                            onClick={() => setDeleteConfirmId(null)}
                            className="px-2 py-1 text-xs text-neutral-400 hover:text-white border border-[#333333] rounded transition-colors"
                          >
                            Cancel
                          </button>
                        </>
                      ) : (
                        <button
                          onClick={() => setDeleteConfirmId(rule.id)}
                          className="px-2 py-1 text-xs text-red-400 hover:text-red-300 border border-[#333333] hover:border-red-700 rounded transition-colors"
                        >
                          Delete
                        </button>
                      )}
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {/* Evaluation Log Section */}
      <div>
        <h2 className="text-lg font-semibold text-white mb-3">
          Evaluation Log{" "}
          <span className="text-sm font-normal text-[#5A5A5A]">(last 20)</span>
        </h2>
        {evalLog.length === 0 ? (
          <div className="bg-[#222222] border border-[#333333] rounded-lg p-6 text-center">
            <p className="text-[#5A5A5A] text-sm">
              No evaluations yet. The engine runs every 60 seconds.
            </p>
          </div>
        ) : (
          <div className="bg-[#222222] border border-[#333333] rounded-lg overflow-hidden">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-[#333333]">
                  <th className="text-left px-4 py-3 text-[#5A5A5A] text-xs uppercase font-medium">
                    Rule
                  </th>
                  <th className="text-left px-4 py-3 text-[#5A5A5A] text-xs uppercase font-medium">
                    Metric Value
                  </th>
                  <th className="text-left px-4 py-3 text-[#5A5A5A] text-xs uppercase font-medium">
                    Fired
                  </th>
                  <th className="text-left px-4 py-3 text-[#5A5A5A] text-xs uppercase font-medium">
                    Action Taken
                  </th>
                  <th className="text-left px-4 py-3 text-[#5A5A5A] text-xs uppercase font-medium">
                    Evaluated At (IST)
                  </th>
                </tr>
              </thead>
              <tbody>
                {evalLog.map((entry) => (
                  <tr
                    key={entry.id}
                    className={`border-b border-[#333333]/50 ${
                      entry.fired ? "bg-amber-900/10" : ""
                    }`}
                  >
                    <td className="px-4 py-3 text-neutral-300 text-xs">
                      {entry.rule_name}
                    </td>
                    <td className="px-4 py-3">
                      <code className="text-neutral-400 text-xs font-mono">
                        {entry.metric_value.toFixed(2)}
                      </code>
                    </td>
                    <td className="px-4 py-3">
                      {entry.fired ? (
                        <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-amber-900/50 text-amber-400">
                          Yes
                        </span>
                      ) : (
                        <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-neutral-700 text-neutral-400">
                          No
                        </span>
                      )}
                    </td>
                    <td className="px-4 py-3">
                      <code className="text-neutral-400 text-xs font-mono">
                        {entry.action_taken}
                      </code>
                    </td>
                    <td className="px-4 py-3 text-neutral-400 text-xs">
                      {formatIST(entry.evaluated_at)}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>
    </DashboardLayout>
  );
}
