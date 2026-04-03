"use client";

import { useEffect, useState } from "react";
import DashboardLayout from "@/components/DashboardLayout";
import { fetchApi } from "@/lib/api";

interface Employee {
  id: string;
  name: string;
  role: string;
  skills: string[];
  hourly_rate_paise: number;
  phone: string;
  is_active: boolean;
  face_enrollment_id: string | null;
  hired_at: string;
}

const formatINR = (paise: number) =>
  "\u20B9" +
  (paise / 100).toLocaleString("en-IN", { minimumFractionDigits: 2 });

function generatePin(): string {
  return String(Math.floor(1000 + Math.random() * 9000));
}

export default function EmployeesPage() {
  const [employees, setEmployees] = useState<Employee[]>([]);
  const [loading, setLoading] = useState(true);
  const [showAddModal, setShowAddModal] = useState(false);
  const [addName, setAddName] = useState("");
  const [addPhone, setAddPhone] = useState("");
  const [addSubmitting, setAddSubmitting] = useState(false);
  const [addError, setAddError] = useState<string | null>(null);

  function loadEmployees() {
    fetchApi<{ employees: Employee[] }>("/hr/employees")
      .then((res) => {
        setEmployees(res.employees || []);
        setLoading(false);
      })
      .catch(() => setLoading(false));
  }

  useEffect(() => { loadEmployees(); }, []);

  async function handleAddEmployee() {
    if (addName.trim().length < 2) { setAddError("Name must be at least 2 characters"); return; }
    if (addPhone.trim().length < 10) { setAddError("Enter a valid 10-digit phone number"); return; }
    setAddSubmitting(true);
    setAddError(null);
    try {
      const res = await fetchApi<{ status?: string; error?: string }>("/staff", {
        method: "POST",
        body: JSON.stringify({ name: addName.trim(), phone: addPhone.trim(), pin: generatePin() }),
      });
      if (res.error) {
        setAddError(res.error);
      } else {
        setShowAddModal(false);
        setAddName("");
        setAddPhone("");
        loadEmployees();
      }
    } catch {
      setAddError("Network error. Try again.");
    } finally {
      setAddSubmitting(false);
    }
  }

  return (
    <DashboardLayout>
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-2xl font-bold text-white">Employees</h1>
          <p className="text-sm text-rp-grey">Staff management and profiles</p>
        </div>
        <div className="flex items-center gap-3">
          <span className="text-xs text-rp-grey">{employees.length} employees</span>
          <button
            onClick={() => { setShowAddModal(true); setAddError(null); }}
            className="px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white text-sm font-medium rounded-lg transition-colors"
          >
            Add Employee
          </button>
        </div>
      </div>

      {/* Add Employee Modal */}
      {showAddModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm">
          <div className="bg-rp-card border border-rp-border rounded-xl p-6 w-full max-w-sm shadow-2xl">
            <h2 className="text-lg font-bold text-white mb-4">Add Employee</h2>

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
              className="w-full bg-rp-surface border border-rp-border rounded-lg px-3 py-2 text-sm text-white placeholder-zinc-600 focus:outline-none focus:border-rp-red transition-colors mb-4"
            />

            {addError && <p className="text-red-400 text-xs mb-3">{addError}</p>}

            <div className="flex gap-2">
              <button
                onClick={() => { setShowAddModal(false); setAddName(""); setAddPhone(""); }}
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
              A login PIN will be sent via WhatsApp daily at 10 AM
            </p>
          </div>
        </div>
      )}

      {loading ? (
        <div className="text-center py-12 text-rp-grey text-sm">Loading employees...</div>
      ) : employees.length === 0 ? (
        <div className="bg-rp-card border border-rp-border rounded-lg p-8 text-center">
          <p className="text-neutral-400 mb-2">No employees found</p>
          <p className="text-rp-grey text-sm">
            Employees can be added through the HR management system.
          </p>
        </div>
      ) : (
        <div className="bg-rp-card border border-rp-border rounded-lg overflow-hidden">
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-rp-border text-rp-grey text-left">
                  <th className="px-4 py-3 font-medium">Name</th>
                  <th className="px-4 py-3 font-medium">Role</th>
                  <th className="px-4 py-3 font-medium">Skills</th>
                  <th className="px-4 py-3 font-medium">Rate</th>
                  <th className="px-4 py-3 font-medium">Phone</th>
                  <th className="px-4 py-3 font-medium">Status</th>
                  <th className="px-4 py-3 font-medium">Hired</th>
                  <th className="px-4 py-3 font-medium">Actions</th>
                </tr>
              </thead>
              <tbody>
                {employees.map((emp) => (
                  <tr
                    key={emp.id}
                    className="border-b border-rp-border/50 hover:bg-white/5"
                  >
                    <td className="px-4 py-3 text-white font-medium">{emp.name}</td>
                    <td className="px-4 py-3 text-neutral-300 capitalize">{emp.role}</td>
                    <td className="px-4 py-3">
                      <div className="flex flex-wrap gap-1">
                        {emp.skills.map((skill) => (
                          <span
                            key={skill}
                            className="px-2 py-0.5 text-xs bg-blue-500/20 text-blue-300 rounded"
                          >
                            {skill}
                          </span>
                        ))}
                      </div>
                    </td>
                    <td className="px-4 py-3 text-neutral-300">
                      {formatINR(emp.hourly_rate_paise)}/hr
                    </td>
                    <td className="px-4 py-3 text-neutral-400">{emp.phone}</td>
                    <td className="px-4 py-3">
                      <span
                        className={`px-2 py-0.5 text-xs rounded font-medium ${
                          emp.is_active
                            ? "bg-green-500/20 text-green-400"
                            : "bg-red-500/20 text-red-400"
                        }`}
                      >
                        {emp.is_active ? "Active" : "Inactive"}
                      </span>
                    </td>
                    <td className="px-4 py-3 text-neutral-400">
                      {new Date(emp.hired_at).toLocaleDateString("en-IN")}
                    </td>
                    <td className="px-4 py-3">
                      <button
                        onClick={async () => {
                          try {
                            const res = await fetchApi<{ status?: string; new_pin?: string; error?: string }>(
                              `/staff/${emp.id}/reset-pin`,
                              { method: "POST" }
                            );
                            if (res.error) {
                              alert(`Error: ${res.error}`);
                            } else {
                              alert(`New PIN for ${emp.name}: ${res.new_pin}\nAlso sent via WhatsApp.`);
                            }
                          } catch {
                            alert("Network error");
                          }
                        }}
                        className="px-2 py-1 text-xs bg-amber-500/20 text-amber-400 rounded hover:bg-amber-500/30 transition-colors"
                      >
                        Reset PIN
                      </button>
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
