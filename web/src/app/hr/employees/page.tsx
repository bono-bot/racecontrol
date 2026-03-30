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

export default function EmployeesPage() {
  const [employees, setEmployees] = useState<Employee[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    fetchApi<{ employees: Employee[] }>("/hr/employees")
      .then((res) => {
        setEmployees(res.employees || []);
        setLoading(false);
      })
      .catch(() => setLoading(false));
  }, []);

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
            onClick={() => alert("Coming soon")}
            className="px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white text-sm font-medium rounded-lg transition-colors"
          >
            Add Employee
          </button>
        </div>
      </div>

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
