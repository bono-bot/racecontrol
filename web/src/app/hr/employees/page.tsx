"use client";

import { useEffect, useState } from "react";
import DashboardLayout from "@/components/DashboardLayout";
import { fetchApi } from "@/lib/api";

interface StaffMember {
  id: string;
  name: string;
  phone: string;
  is_active: boolean;
  last_login_at: string | null;
  role: string;
}

function generatePin(): string {
  return String(Math.floor(1000 + Math.random() * 9000));
}

export default function EmployeesPage() {
  const [staff, setStaff] = useState<StaffMember[]>([]);
  const [loading, setLoading] = useState(true);

  // Add modal
  const [showAddModal, setShowAddModal] = useState(false);
  const [addName, setAddName] = useState("");
  const [addPhone, setAddPhone] = useState("");
  const [addPin, setAddPin] = useState("");
  const [addUseCustomPin, setAddUseCustomPin] = useState(false);
  const [addSubmitting, setAddSubmitting] = useState(false);
  const [addError, setAddError] = useState<string | null>(null);

  // Set PIN modal
  const [showPinModal, setShowPinModal] = useState(false);
  const [pinTarget, setPinTarget] = useState<StaffMember | null>(null);
  const [newPin, setNewPin] = useState("");
  const [pinSubmitting, setPinSubmitting] = useState(false);
  const [pinError, setPinError] = useState<string | null>(null);

  function loadStaff() {
    fetchApi<{ staff: StaffMember[] }>("/staff")
      .then((res) => {
        setStaff((res.staff || []).filter((s) => s.is_active));
        setLoading(false);
      })
      .catch(() => setLoading(false));
  }

  useEffect(() => { loadStaff(); }, []);

  async function handleAddEmployee() {
    const name = addName.trim();
    const phone = addPhone.trim();
    if (name.length < 2) { setAddError("Name must be at least 2 characters"); return; }
    if (phone.length < 10) { setAddError("Enter a valid 10-digit phone number"); return; }

    const pin = addUseCustomPin ? addPin : generatePin();
    if (addUseCustomPin && (addPin.length < 4 || !/^\d+$/.test(addPin))) {
      setAddError("PIN must be at least 4 digits");
      return;
    }

    setAddSubmitting(true);
    setAddError(null);
    try {
      const res = await fetchApi<{ status?: string; error?: string }>("/staff", {
        method: "POST",
        body: JSON.stringify({ name, phone, pin }),
      });
      if (res.error) {
        setAddError(res.error);
      } else {
        setShowAddModal(false);
        setAddName("");
        setAddPhone("");
        setAddPin("");
        setAddUseCustomPin(false);
        loadStaff();
      }
    } catch {
      setAddError("Network error. Try again.");
    } finally {
      setAddSubmitting(false);
    }
  }

  async function handleSetPin() {
    if (!pinTarget) return;
    if (newPin.length < 4 || !/^\d+$/.test(newPin)) {
      setPinError("PIN must be at least 4 digits");
      return;
    }
    setPinSubmitting(true);
    setPinError(null);
    try {
      const res = await fetchApi<{ status?: string; error?: string }>(
        `/staff/${pinTarget.id}`,
        {
          method: "PUT",
          body: JSON.stringify({ pin: newPin }),
        }
      );
      if (res.error) {
        setPinError(res.error);
      } else {
        setShowPinModal(false);
        setPinTarget(null);
        setNewPin("");
        loadStaff();
      }
    } catch {
      setPinError("Network error");
    } finally {
      setPinSubmitting(false);
    }
  }

  async function handleResetPin(member: StaffMember) {
    try {
      const res = await fetchApi<{ status?: string; new_pin?: string; error?: string }>(
        `/staff/${member.id}/reset-pin`,
        { method: "POST" }
      );
      if (res.error) {
        alert(`Error: ${res.error}`);
      } else {
        alert(`New PIN for ${member.name}: ${res.new_pin}\nAlso sent via WhatsApp.`);
      }
    } catch {
      alert("Network error");
    }
  }

  function openSetPin(member: StaffMember) {
    setPinTarget(member);
    setNewPin("");
    setPinError(null);
    setShowPinModal(true);
  }

  return (
    <DashboardLayout>
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-2xl font-bold text-white">Staff</h1>
          <p className="text-sm text-rp-grey">Staff PIN management and profiles</p>
        </div>
        <div className="flex items-center gap-3">
          <span className="text-xs text-rp-grey">{staff.length} staff</span>
          <button
            onClick={() => { setShowAddModal(true); setAddError(null); setAddPin(""); setAddUseCustomPin(false); }}
            className="px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white text-sm font-medium rounded-lg transition-colors"
          >
            Add Staff
          </button>
        </div>
      </div>

      {/* Add Staff Modal */}
      {showAddModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm">
          <div className="bg-rp-card border border-rp-border rounded-xl p-6 w-full max-w-sm shadow-2xl">
            <h2 className="text-lg font-bold text-white mb-4">Add Staff Member</h2>

            <label className="block text-xs text-neutral-400 mb-1">Name</label>
            <input
              type="text"
              value={addName}
              onChange={(e) => setAddName(e.target.value)}
              placeholder="Chavan Vishal"
              className="w-full bg-rp-surface border border-rp-border rounded-lg px-3 py-2 text-sm text-white placeholder-zinc-600 focus:outline-none focus:border-rp-red transition-colors mb-3"
              autoFocus
            />

            <label className="block text-xs text-neutral-400 mb-1">Phone</label>
            <input
              type="tel"
              value={addPhone}
              onChange={(e) => setAddPhone(e.target.value.replace(/\D/g, "").slice(0, 10))}
              placeholder="9876543210"
              inputMode="numeric"
              className="w-full bg-rp-surface border border-rp-border rounded-lg px-3 py-2 text-sm text-white placeholder-zinc-600 focus:outline-none focus:border-rp-red transition-colors mb-3"
            />

            {/* PIN option */}
            <div className="flex items-center gap-2 mb-2">
              <input
                type="checkbox"
                id="custom-pin"
                checked={addUseCustomPin}
                onChange={(e) => { setAddUseCustomPin(e.target.checked); setAddPin(""); }}
                className="accent-rp-red"
              />
              <label htmlFor="custom-pin" className="text-xs text-neutral-400">Set custom PIN</label>
            </div>

            {addUseCustomPin ? (
              <>
                <label className="block text-xs text-neutral-400 mb-1">PIN</label>
                <input
                  type="text"
                  value={addPin}
                  onChange={(e) => setAddPin(e.target.value.replace(/\D/g, "").slice(0, 6))}
                  placeholder="4-6 digits"
                  inputMode="numeric"
                  maxLength={6}
                  className="w-full bg-rp-surface border border-rp-border rounded-lg px-3 py-2 text-sm text-white placeholder-zinc-600 focus:outline-none focus:border-rp-red transition-colors mb-4"
                />
              </>
            ) : (
              <p className="text-xs text-neutral-500 mb-4">A random 4-digit PIN will be generated automatically</p>
            )}

            {addError && <p className="text-red-400 text-xs mb-3">{addError}</p>}

            <div className="flex gap-2">
              <button
                onClick={() => { setShowAddModal(false); setAddName(""); setAddPhone(""); setAddPin(""); }}
                className="flex-1 rounded-lg py-2 text-sm font-medium bg-rp-surface text-neutral-400 hover:text-white border border-rp-border transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={handleAddEmployee}
                disabled={addSubmitting}
                className="flex-1 rounded-lg py-2 text-sm font-semibold bg-blue-600 hover:bg-blue-700 text-white disabled:opacity-50 transition-colors"
              >
                {addSubmitting ? "Adding..." : "Add"}
              </button>
            </div>

            <p className="text-xs text-neutral-500 mt-3 text-center">
              PIN will be sent via WhatsApp to the staff member
            </p>
          </div>
        </div>
      )}

      {/* Set PIN Modal */}
      {showPinModal && pinTarget && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm">
          <div className="bg-rp-card border border-rp-border rounded-xl p-6 w-full max-w-xs shadow-2xl">
            <h2 className="text-lg font-bold text-white mb-1">Set PIN</h2>
            <p className="text-sm text-neutral-400 mb-4">{pinTarget.name}</p>

            <label className="block text-xs text-neutral-400 mb-1">New PIN</label>
            <input
              type="text"
              value={newPin}
              onChange={(e) => setNewPin(e.target.value.replace(/\D/g, "").slice(0, 6))}
              placeholder="4-6 digits"
              inputMode="numeric"
              maxLength={6}
              className="w-full bg-rp-surface border border-rp-border rounded-lg px-3 py-2 text-sm text-white placeholder-zinc-600 focus:outline-none focus:border-rp-red transition-colors mb-4"
              autoFocus
            />

            {pinError && <p className="text-red-400 text-xs mb-3">{pinError}</p>}

            <div className="flex gap-2">
              <button
                onClick={() => { setShowPinModal(false); setPinTarget(null); setNewPin(""); }}
                className="flex-1 rounded-lg py-2 text-sm font-medium bg-rp-surface text-neutral-400 hover:text-white border border-rp-border transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={handleSetPin}
                disabled={pinSubmitting}
                className="flex-1 rounded-lg py-2 text-sm font-semibold bg-rp-red hover:bg-rp-red-hover text-white disabled:opacity-50 transition-colors"
              >
                {pinSubmitting ? "Saving..." : "Set PIN"}
              </button>
            </div>
          </div>
        </div>
      )}

      {loading ? (
        <div className="text-center py-12 text-rp-grey text-sm">Loading staff...</div>
      ) : staff.length === 0 ? (
        <div className="bg-rp-card border border-rp-border rounded-lg p-8 text-center">
          <p className="text-neutral-400 mb-2">No staff found</p>
          <p className="text-rp-grey text-sm">Add staff members using the button above.</p>
        </div>
      ) : (
        <div className="bg-rp-card border border-rp-border rounded-lg overflow-hidden">
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-rp-border text-rp-grey text-left">
                  <th className="px-4 py-3 font-medium">Name</th>
                  <th className="px-4 py-3 font-medium">Role</th>
                  <th className="px-4 py-3 font-medium">Phone</th>
                  <th className="px-4 py-3 font-medium">Last Login</th>
                  <th className="px-4 py-3 font-medium">Status</th>
                  <th className="px-4 py-3 font-medium">Actions</th>
                </tr>
              </thead>
              <tbody>
                {staff.map((s) => (
                  <tr key={s.id} className="border-b border-rp-border/50 hover:bg-white/5">
                    <td className="px-4 py-3 text-white font-medium">{s.name}</td>
                    <td className="px-4 py-3 text-neutral-300 capitalize">{s.role}</td>
                    <td className="px-4 py-3 text-neutral-400">{s.phone || "-"}</td>
                    <td className="px-4 py-3 text-neutral-400">
                      {s.last_login_at
                        ? new Date(s.last_login_at).toLocaleString("en-IN", { day: "2-digit", month: "short", hour: "2-digit", minute: "2-digit" })
                        : "Never"}
                    </td>
                    <td className="px-4 py-3">
                      <span className={`px-2 py-0.5 text-xs rounded font-medium ${
                        s.is_active ? "bg-green-500/20 text-green-400" : "bg-red-500/20 text-red-400"
                      }`}>
                        {s.is_active ? "Active" : "Inactive"}
                      </span>
                    </td>
                    <td className="px-4 py-3">
                      <div className="flex gap-2">
                        <button
                          onClick={() => openSetPin(s)}
                          className="px-2 py-1 text-xs bg-blue-500/20 text-blue-400 rounded hover:bg-blue-500/30 transition-colors"
                        >
                          Set PIN
                        </button>
                        <button
                          onClick={() => handleResetPin(s)}
                          className="px-2 py-1 text-xs bg-amber-500/20 text-amber-400 rounded hover:bg-amber-500/30 transition-colors"
                        >
                          Reset PIN
                        </button>
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}
    </DashboardLayout>
  );
}
