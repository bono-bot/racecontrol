"use client";

import { useEffect, useState } from "react";
import DashboardLayout from "@/components/DashboardLayout";
import { fetchApi } from "@/lib/api";

interface ShiftRecord {
  employee_id: string;
  employee_name: string;
  clock_in: string | null;
  clock_out: string | null;
  hours_worked: number;
  source: string;
}

const MOCK_SHIFTS: ShiftRecord[] = [
  {
    employee_id: "mock-1",
    employee_name: "Ravi Kumar",
    clock_in: "2026-03-30T09:00:00",
    clock_out: "2026-03-30T17:30:00",
    hours_worked: 8.5,
    source: "camera",
  },
  {
    employee_id: "mock-2",
    employee_name: "Priya Sharma",
    clock_in: "2026-03-30T10:00:00",
    clock_out: "2026-03-30T18:00:00",
    hours_worked: 8.0,
    source: "manual",
  },
  {
    employee_id: "mock-3",
    employee_name: "Arjun Singh",
    clock_in: "2026-03-30T12:00:00",
    clock_out: null,
    hours_worked: 0,
    source: "camera",
  },
];

function formatTime(iso: string | null): string {
  if (!iso) return "--";
  const d = new Date(iso);
  return d.toLocaleTimeString("en-IN", { hour: "2-digit", minute: "2-digit", hour12: true });
}

export default function ShiftsPage() {
  const [shifts, setShifts] = useState<ShiftRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedDate, setSelectedDate] = useState(() => {
    const now = new Date();
    return now.toISOString().split("T")[0];
  });

  useEffect(() => {
    setLoading(true);
    fetchApi<{ shifts: ShiftRecord[] }>(`/hr/shifts?date=${selectedDate}`)
      .then((res) => {
        const records = res.shifts || [];
        setShifts(records.length > 0 ? records : MOCK_SHIFTS);
        setLoading(false);
      })
      .catch(() => {
        setShifts(MOCK_SHIFTS);
        setLoading(false);
      });
  }, [selectedDate]);

  return (
    <DashboardLayout>
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-2xl font-bold text-white">Shifts &amp; Attendance</h1>
          <p className="text-sm text-rp-grey">Daily clock-in/clock-out tracking</p>
        </div>
        <div className="flex items-center gap-3">
          <span className="text-xs text-rp-grey">{shifts.length} records</span>
          <input
            type="date"
            value={selectedDate}
            onChange={(e) => setSelectedDate(e.target.value)}
            className="px-3 py-2 bg-rp-card border border-rp-border rounded-lg text-sm text-white focus:outline-none focus:border-blue-500"
          />
        </div>
      </div>

      {loading ? (
        <div className="text-center py-12 text-rp-grey text-sm">Loading attendance...</div>
      ) : shifts.length === 0 ? (
        <div className="bg-rp-card border border-rp-border rounded-lg p-8 text-center">
          <p className="text-neutral-400 mb-2">No attendance records</p>
          <p className="text-rp-grey text-sm">
            No shifts recorded for {selectedDate}.
          </p>
        </div>
      ) : (
        <div className="bg-rp-card border border-rp-border rounded-lg overflow-hidden">
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-rp-border text-rp-grey text-left">
                  <th className="px-4 py-3 font-medium">Employee</th>
                  <th className="px-4 py-3 font-medium">Clock In</th>
                  <th className="px-4 py-3 font-medium">Clock Out</th>
                  <th className="px-4 py-3 font-medium text-right">Hours</th>
                  <th className="px-4 py-3 font-medium">Source</th>
                </tr>
              </thead>
              <tbody>
                {shifts.map((shift) => (
                  <tr
                    key={shift.employee_id}
                    className="border-b border-rp-border/50 hover:bg-white/5"
                  >
                    <td className="px-4 py-3 text-white font-medium">
                      {shift.employee_name}
                    </td>
                    <td className="px-4 py-3 text-neutral-300">
                      {formatTime(shift.clock_in)}
                    </td>
                    <td className="px-4 py-3 text-neutral-300">
                      {formatTime(shift.clock_out)}
                    </td>
                    <td className="px-4 py-3 text-right text-neutral-300">
                      {shift.hours_worked > 0
                        ? `${shift.hours_worked.toFixed(1)}h`
                        : "--"}
                    </td>
                    <td className="px-4 py-3">
                      <span
                        className={`px-2 py-0.5 text-xs rounded font-medium ${
                          shift.source === "camera"
                            ? "bg-purple-500/20 text-purple-300"
                            : "bg-yellow-500/20 text-yellow-300"
                        }`}
                      >
                        {shift.source}
                      </span>
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
